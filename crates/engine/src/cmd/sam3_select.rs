//! Object Selection and Quick Selection with SAM 3 (Segment Anything, `photocraft_sam3`).
//!
//! **Engine choice.** The preference `tools.objectSelectionEngine` ([`ObjectEngine`]): `auto` uses
//! SAM 3 when its model is installed and the computer has the memory for it (else Classic, with no
//! note), `sam3` the same but says why in a status note when it falls back, `classic` never uses it.
//!
//! **Encoding.** SAM 3 reads the whole picture once (about 5 s, a peak of 19 GB) and then answers
//! each click in about 40 ms. The engine keeps one encoded image, for the active document and the
//! layer (or composite) the tool samples, valid while the pixels are the same
//! ([`same_pixels`](crate::layer_multi_cmds::same_pixels), so selecting never drops it). It is made
//! by a background thread ([`Session::sam3_prepare`]) that the UI starts when the tool is picked;
//! nothing here ever waits for it. Until it is ready the Classic finders answer (clicks carry a note).
//! The encoding is dropped when a pixel edit lands, the document closes, another document or layer
//! is sampled, or the service has been idle for ten minutes. The service holds one image, so
//! switching documents encodes again.
//!
//! **Clicks.** A click on the tool's picture selects the object under it: SAM 3's soft mask
//! becomes the selection's alpha. ⇧-click (`"refine": "add"`) adds a positive point and ⌥-click
//! (`"refine": "subtract"`) a negative one to the same mask, as long as nothing else changed the
//! document since: the selection is rebuilt from all the points on the selection the sequence
//! started from. Without such a sequence they select (⇧) or deselect (⌥) the object under the
//! click, with the `mode` given. Each click is one History step. Quick Selection strokes are
//! positive clicks along the stroke (⌥ strokes negative) and continue the same sequence.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use photocraft_algo::selection::{Region, SelectionMode, combine_region};
use photocraft_doc::{DocId, Document, LayerId};
use photocraft_geom::Rect;
use photocraft_ml::{MlError, Rgb};
use photocraft_raster::Surface;
use serde_json::{Value, json};

use super::smartselect_cmds::{sampled_target, with_doc_sampler};
use crate::prefs::ObjectEngine;
use crate::{EngineError, Result, Session};

mod backend;
mod boxed;
mod mask;
mod subject;

pub(crate) use boxed::{BoxRequest, select_box};
pub(crate) use subject::{SubjectSam3, failure_note};
pub use backend::{EncodedImage, Mask, PointBox, PointClick, Sam3Backend};

/// The note for an explicit `sam3` choice with no model installed.
pub const NOT_INSTALLED: &str = "SAM 3 not installed, used Classic (Preferences › AI Models)";
/// The note for an explicit `sam3` choice on a computer without the memory.
pub const NO_MEMORY: &str = "Not enough memory for SAM 3, used Classic";
/// The note for SAM 3 failing.
pub const FAILED: &str = "SAM 3 failed, used Classic";
/// The note for a click made before the encoding is ready.
pub const NOT_READY: &str = "SAM 3 is still preparing, used Classic";

/// Most points one Quick Selection stroke gives SAM 3.
const STROKE_CLICKS: usize = 6;
/// Most points one sequence keeps (each is one more prompt for the decoder).
const SEQUENCE_CLICKS: usize = 24;
/// How long the answer to "is SAM 3 installed, is there memory" is reused.
const CAPABLE_FOR: Duration = Duration::from_secs(5);

/// Where SAM 3 stands for the active document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sam3Status {
    /// Not used: the preference says Classic, or SAM 3 cannot run here (with the note, when the
    /// choice was explicit).
    Unavailable(Option<&'static str>),
    /// Usable, nothing prepared for these pixels yet ([`Session::sam3_prepare`] starts it).
    Idle,
    /// Encoding in the background.
    Preparing,
    /// Clicks and hovers use SAM 3.
    Ready,
    /// Encoding failed for these pixels (with the note); an edit or [`Prepare::Force`] tries again.
    Failed(&'static str),
    /// The user stopped the encoding (Esc); [`Prepare::Force`] starts it again.
    Cancelled,
    /// The service freed the model after being idle; [`Prepare::Force`] encodes again.
    Unloaded,
}

impl Sam3Status {
    /// The word the control channel reports.
    pub fn word(self) -> &'static str {
        match self {
            Self::Unavailable(_) => "unavailable",
            Self::Idle => "idle",
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
            Self::Unloaded => "unloaded",
        }
    }
}

/// How hard [`Session::sam3_prepare`] tries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prepare {
    /// Start only when nothing was tried for these pixels (a failure, a cancel or an unload stands).
    Auto,
    /// Start again after a failure, a cancel or an unload (the user picked the tool, or asks).
    Force,
}

/// What a background encoding came to.
struct Encoded {
    image: EncodedImage,
    canvas: Rect,
    step: usize,
}

type Outcome = std::result::Result<Encoded, MlError>;

/// Which pixels an entry is for: the document, the sampled layer (`None` = composite), and the
/// document it was made from, held weakly (as the other smart-selection caches do) so it never
/// keeps an edited document's tiles alive.
struct Key {
    doc: std::sync::Weak<Document>,
    doc_id: DocId,
    target: Option<LayerId>,
}

impl Key {
    fn new(doc: &Arc<Document>, target: Option<LayerId>) -> Self {
        Self { doc: Arc::downgrade(doc), doc_id: doc.id, target }
    }

    /// Whether this is for `doc` sampling `target` and the pixels are the same; follows `doc`
    /// past a selection-only edit, so the next check still passes.
    fn holds(&mut self, doc: &Arc<Document>, target: Option<LayerId>) -> bool {
        let same = self.doc_id == doc.id && self.target == target && self.doc.upgrade().is_some_and(|old| crate::layer_multi_cmds::same_pixels(&old, doc));
        if same {
            self.doc = Arc::downgrade(doc);
        }
        same
    }
}

struct Preparing {
    key: Key,
    rx: std::sync::mpsc::Receiver<Outcome>,
    cancel: Arc<AtomicBool>,
}

struct Ready {
    key: Key,
    done: Encoded,
}

#[derive(Default)]
enum Entry {
    #[default]
    None,
    Preparing(Preparing),
    Ready(Ready),
    Failed(Key, &'static str),
    Cancelled(Key),
    Unloaded(Key),
}

impl Entry {
    fn key_mut(&mut self) -> Option<&mut Key> {
        match self {
            Self::None => None,
            Self::Preparing(p) => Some(&mut p.key),
            Self::Ready(r) => Some(&mut r.key),
            Self::Failed(k, _) | Self::Cancelled(k) | Self::Unloaded(k) => Some(k),
        }
    }
}

/// The points of the object being built by clicks, to refine it with the next one. It stands
/// while the document is as the sequence left it (the revision), so any other edit, an undo or a
/// layer change starts afresh.
struct Sequence {
    doc_id: DocId,
    target: Option<LayerId>,
    revision: u64,
    /// The selection the sequence started from, which the mask is combined with.
    base: Option<Surface>,
    mode: SelectionMode,
    /// Document points, each positive or negative.
    points: Vec<((f32, f32), bool)>,
}

/// The session's SAM 3 state (`Session::sam3`).
#[derive(Default)]
pub struct Sam3Select {
    backend: Option<Arc<dyn Sam3Backend>>,
    entry: Entry,
    sequence: Option<Sequence>,
    capable: Option<(Instant, bool, bool)>,
}

impl std::fmt::Debug for Sam3Select {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sam3Select").finish_non_exhaustive()
    }
}

/// The backend a session starts with: the model, except in unit tests and the browser, which have none.
fn default_backend() -> Arc<dyn Sam3Backend> {
    #[cfg(not(any(test, target_arch = "wasm32")))]
    {
        Arc::new(backend::Real::new())
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    {
        Arc::new(backend::Unavailable)
    }
}

/// Gives `image` back to the backend off the calling thread: releasing waits for a request that
/// is running, and the UI thread must not.
fn release_later(backend: &Arc<dyn Sam3Backend>, image: EncodedImage) {
    let backend = Arc::clone(backend);
    #[cfg(not(target_arch = "wasm32"))]
    {
        if std::thread::Builder::new().name("sam3-release".into()).spawn(move || backend.release(image)).is_err() {
            // no thread: the image stays held until the next encoding replaces it
        }
    }
    #[cfg(target_arch = "wasm32")]
    backend.release(image);
}

impl Sam3Select {
    fn backend(&mut self) -> Arc<dyn Sam3Backend> {
        Arc::clone(self.backend.get_or_insert_with(default_backend))
    }

    /// Drops the entry, freeing what it holds.
    fn clear(&mut self) {
        match std::mem::take(&mut self.entry) {
            Entry::Preparing(p) => p.cancel.store(true, Ordering::SeqCst),
            Entry::Ready(r) => {
                let backend = self.backend();
                release_later(&backend, r.done.image);
            }
            _ => {}
        }
    }

    /// Brings the entry up to date for `doc` sampling `target`: an entry for other pixels is
    /// dropped, a finished encoding becomes ready, a model the service freed becomes unloaded.
    fn settle(&mut self, doc: &Arc<Document>, target: Option<LayerId>) {
        if let Some(key) = self.entry.key_mut()
            && !key.holds(doc, target)
        {
            self.clear();
        }
        if let Entry::Preparing(p) = &mut self.entry {
            let outcome = match p.rx.try_recv() {
                Ok(o) => Some(o),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(MlError::Backend("the encoding thread ended".into()))),
            };
            if let Some(outcome) = outcome
                && let Entry::Preparing(p) = std::mem::take(&mut self.entry)
            {
                self.entry = match outcome {
                    Ok(done) => Entry::Ready(Ready { key: p.key, done }),
                    Err(MlError::Cancelled) => Entry::Cancelled(p.key),
                    Err(MlError::LowMemory { .. }) => Entry::Failed(p.key, NO_MEMORY),
                    Err(MlError::ModelMissing { .. }) => Entry::Failed(p.key, NOT_INSTALLED),
                    Err(_) => Entry::Failed(p.key, FAILED),
                };
            }
        }
        let held = if let Entry::Ready(r) = &self.entry { Some(r.done.image) } else { None };
        if let Some(image) = held {
            let backend = self.backend();
            if !backend.is_current(image)
                && let Entry::Ready(r) = std::mem::take(&mut self.entry)
            {
                self.entry = Entry::Unloaded(r.key);
            }
        }
    }

    fn status(&self) -> Option<Sam3Status> {
        Some(match &self.entry {
            Entry::None => return None,
            Entry::Preparing(_) => Sam3Status::Preparing,
            Entry::Ready(_) => Sam3Status::Ready,
            Entry::Failed(_, note) => Sam3Status::Failed(note),
            Entry::Cancelled(_) => Sam3Status::Cancelled,
            Entry::Unloaded(_) => Sam3Status::Unloaded,
        })
    }
}

/// Whether the model is installed and this computer has the memory for it (the answer is reused
/// for [`CAPABLE_FOR`]): `Ok`, or the note that says which is missing.
fn can_run(s: &mut Session) -> std::result::Result<(), &'static str> {
    let backend = s.sam3.backend();
    let (present, memory) = match s.sam3.capable {
        Some((at, present, memory)) if at.elapsed() < CAPABLE_FOR => (present, memory),
        _ => {
            let (present, memory) = (backend.present(), backend.has_memory());
            s.sam3.capable = Some((Instant::now(), present, memory));
            (present, memory)
        }
    };
    if !present {
        return Err(NOT_INSTALLED);
    }
    if !memory {
        return Err(NO_MEMORY);
    }
    Ok(())
}

/// Whether the preference and this computer let SAM 3 run: `Ok` or the note for the status bar
/// (`None` for the quiet cases: `classic`, or `auto` where it cannot run).
fn capable(s: &mut Session) -> std::result::Result<(), Option<&'static str>> {
    let engine = s.prefs().tools.object_selection_engine;
    if engine == ObjectEngine::Classic {
        return Err(None);
    }
    can_run(s).map_err(|note| (engine == ObjectEngine::Sam3).then_some(note))
}

/// The document's pixels for the model, shrunk by `step`, and the encoding of them.
fn encode_work(backend: &dyn Sam3Backend, doc: Arc<Document>, layer: Option<LayerId>, all_layers: bool, cancel: &AtomicBool) -> Outcome {
    let stopped = || cancel.load(Ordering::SeqCst);
    let canvas = doc.bounds();
    if canvas.is_empty() {
        return Err(MlError::BadInput("the document has no pixels".into()));
    }
    let step = mask::step_for(canvas);
    let small = with_doc_sampler(&doc, layer, all_layers, |smp, _| smp.rgb_scaled(canvas, step));
    // The snapshot is not needed to encode: let an edited document's tiles go.
    drop(doc);
    if stopped() {
        return Err(MlError::Cancelled);
    }
    let rgb = Rgb::new(small.w, small.h, small.px.iter().flatten().copied().collect())?;
    drop(small);
    let image = backend.encode(&rgb, &stopped)?;
    Ok(Encoded { image, canvas, step })
}

impl Session {
    /// Installs the SAM 3 backend this session uses (tests build a fake; the default is the model).
    pub fn set_sam3_backend(&mut self, backend: Arc<dyn Sam3Backend>) {
        self.sam3.clear();
        self.sam3.sequence = None;
        self.sam3.capable = None;
        self.sam3.backend = Some(backend);
    }

    /// Where SAM 3 stands for the active document sampling the selected layer (or, with
    /// `all_layers`, the composite). Never blocks.
    pub fn sam3_status(&mut self, all_layers: bool) -> Sam3Status {
        if let Err(note) = capable(self) {
            return Sam3Status::Unavailable(note);
        }
        let Some(st) = self.active() else { return Sam3Status::Idle };
        let (doc, target) = (Arc::clone(&st.doc), sampled_target(&st.doc, st.active_layer, all_layers));
        self.sam3.settle(&doc, target);
        self.sam3.status().unwrap_or(Sam3Status::Idle)
    }

    /// Starts encoding the active document's pixels in the background, unless that is done,
    /// running, or (with [`Prepare::Auto`]) stopped or failed. Returns at once with the status.
    pub fn sam3_prepare(&mut self, all_layers: bool, how: Prepare) -> Result<Sam3Status> {
        let status = self.sam3_status(all_layers);
        let startable = match status {
            Sam3Status::Idle => true,
            Sam3Status::Failed(_) | Sam3Status::Cancelled | Sam3Status::Unloaded => how == Prepare::Force,
            _ => false,
        };
        if !startable {
            return Ok(status);
        }
        let st = self.active().ok_or(EngineError::NoDocument)?;
        let (doc, layer) = (Arc::clone(&st.doc), st.active_layer);
        let target = sampled_target(&doc, layer, all_layers);
        let backend = self.sam3.backend();
        self.sam3.clear();
        let key = Key::new(&doc, target);
        let (tx, rx) = std::sync::mpsc::channel::<Outcome>();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let work = move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| encode_work(backend.as_ref(), doc, layer, all_layers, &stop)))
                .unwrap_or_else(|_| Err(MlError::Backend("SAM 3 failed unexpectedly".into())));
            // Nobody is waiting any more (cancelled, edited, closed): don't keep the image.
            if let Err(std::sync::mpsc::SendError(Ok(done))) = tx.send(outcome) {
                release_later(&backend, done.image);
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::thread::Builder::new().name("sam3-encode".into()).spawn(work).is_ok();
        #[cfg(target_arch = "wasm32")]
        let started = {
            drop(work);
            false
        };
        self.sam3.entry = if started { Entry::Preparing(Preparing { key, rx, cancel }) } else { Entry::Failed(key, FAILED) };
        Ok(self.sam3_status(all_layers))
    }

    /// Stops the encoding that is running (it ends within the stage it is in). Returns whether
    /// there was one.
    pub fn sam3_cancel(&mut self) -> bool {
        match std::mem::take(&mut self.sam3.entry) {
            Entry::Preparing(p) => {
                p.cancel.store(true, Ordering::SeqCst);
                self.sam3.entry = Entry::Cancelled(p.key);
                true
            }
            other => {
                self.sam3.entry = other;
                false
            }
        }
    }

    /// Waits up to `timeout` for a running encoding (scripts and tests; the UI polls instead).
    pub fn sam3_wait(&mut self, all_layers: bool, timeout: Duration) -> Sam3Status {
        let end = Instant::now() + timeout;
        loop {
            let status = self.sam3_status(all_layers);
            if status != Sam3Status::Preparing || Instant::now() >= end {
                return status;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Drops what SAM 3 holds for document `id`: a pixel edit landed on it, or it is closing.
    pub(crate) fn sam3_forget_doc(&mut self, id: DocId) {
        if self.sam3.entry.key_mut().is_some_and(|k| k.doc_id == id) {
            self.sam3.clear();
        }
        if self.sam3.sequence.as_ref().is_some_and(|q| q.doc_id == id) {
            self.sam3.sequence = None;
        }
    }

    /// What the hover preview needs to ask SAM 3 for the object under the pointer, when the
    /// encoding is ready for the active document (else `None`: the Classic finder answers).
    pub fn sam3_hover_source(&mut self, all_layers: bool) -> Option<HoverSource> {
        if self.sam3_status(all_layers) != Sam3Status::Ready {
            return None;
        }
        let Entry::Ready(r) = &self.sam3.entry else { return None };
        let (image, canvas, step) = (r.done.image, r.done.canvas, r.done.step);
        Some(HoverSource { backend: self.sam3.backend(), image, canvas, step })
    }
}

/// A ready encoding, for asking about points off the session (the hover preview's worker thread).
#[derive(Clone)]
pub struct HoverSource {
    backend: Arc<dyn Sam3Backend>,
    image: EncodedImage,
    canvas: Rect,
    step: usize,
}

impl HoverSource {
    /// The object SAM 3 finds at document point `(x, y)`: one positive click (about 40 ms).
    pub fn find(&self, x: f32, y: f32) -> std::result::Result<Option<Region>, MlError> {
        let Some((nx, ny)) = mask::normalise(self.canvas, self.step, (self.image.w, self.image.h), (x, y)) else { return Ok(None) };
        let m = self.backend.clicks(self.image, &[PointClick { x: nx, y: ny, positive: true }])?;
        mask::to_region(&m, self.canvas, self.step).map_err(|e| MlError::Backend(e.to_string()))
    }
}

/// The points of a Quick Selection stroke SAM 3 gets as clicks.
pub(crate) fn stroke(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    mask::spread(points, STROKE_CLICKS)
}

/// One click, or the points of one Quick Selection stroke.
pub(crate) struct Request<'a> {
    /// Document points.
    pub points: Vec<(f32, f32)>,
    /// A ⌥ click or stroke: excludes what is under it.
    pub negative: bool,
    /// ⇧ / ⌥: continue the sequence standing, if any.
    pub refine: bool,
    /// How a click that starts a sequence combines with the selection.
    pub mode: SelectionMode,
    pub label: &'a str,
    pub all_layers: bool,
}

/// What [`click`] came to.
pub(crate) enum Attempt {
    /// SAM 3 answered; the selection is set (or left alone: `changed: false`).
    Done(Value),
    /// Use the Classic finder, and tell the user why (when there is a note).
    Classic(Option<&'static str>),
}

/// The encoding SAM 3 answers from (image, canvas, step), or the reason Classic answers instead.
fn ready_encoding(s: &mut Session, all_layers: bool) -> std::result::Result<(EncodedImage, Rect, usize), Attempt> {
    if let Err(note) = capable(s) {
        return Err(Attempt::Classic(note));
    }
    let status = s.sam3_status(all_layers);
    match (&s.sam3.entry, status) {
        (Entry::Ready(r), Sam3Status::Ready) => Ok((r.done.image, r.done.canvas, r.done.step)),
        (_, Sam3Status::Preparing) => Err(Attempt::Classic(Some(NOT_READY))),
        (_, Sam3Status::Failed(note)) => Err(Attempt::Classic(Some(note))),
        // Not prepared (the tool was not picked through the UI, an edit dropped it, the model was
        // unloaded): Classic, quietly. The shell starts the encoding for the next click.
        _ => Err(Attempt::Classic(None)),
    }
}

/// Selects with SAM 3 when it can: ready encoding for the active document, usable preference.
pub(crate) fn click(s: &mut Session, req: &Request) -> Result<Attempt> {
    let (image, canvas, step) = match ready_encoding(s, req.all_layers) {
        Ok(ready) => ready,
        Err(classic) => return Ok(classic),
    };
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let (doc_id, revision, target, selection) = (d.doc.id, d.revision, sampled_target(&d.doc, d.active_layer, req.all_layers), d.doc.selection.clone());
    // The sequence this continues, or a new one from the selection as it is.
    let live = s.sam3.sequence.as_ref().filter(|q| req.refine && q.doc_id == doc_id && q.target == target && q.revision == revision);
    let (base, mode, mut points) = match live {
        Some(q) => (q.base.clone(), q.mode, q.points.clone()),
        None => (selection, req.mode, Vec::new()),
    };
    // A negative click with nothing to exclude from takes the object under it off the selection.
    let negative = req.negative && points.iter().any(|(_, positive)| *positive);
    let mode = if req.negative && !negative && mode == SelectionMode::Replace { SelectionMode::Subtract } else { mode };
    points.extend(req.points.iter().map(|p| (*p, !negative)));
    if points.len() > SEQUENCE_CLICKS {
        points.drain(..points.len() - SEQUENCE_CLICKS);
    }
    let clicks: Vec<PointClick> = points
        .iter()
        .filter_map(|((x, y), positive)| mask::normalise(canvas, step, (image.w, image.h), (*x, *y)).map(|(x, y)| PointClick { x, y, positive: *positive }))
        .collect();
    if clicks.is_empty() || !clicks.iter().any(|c| c.positive) {
        return Ok(Attempt::Classic(None));
    }
    let backend = s.sam3.backend();
    let region = match backend.clicks(image, &clicks).map_err(|e| e.to_string()).and_then(|m| mask::to_region(&m, canvas, step).map_err(|e| e.to_string())) {
        Ok(region) => region,
        Err(_) => {
            // The image was freed or the model failed: drop it, the Classic finder answers.
            s.sam3.clear();
            return Ok(Attempt::Classic(Some(FAILED)));
        }
    };
    let Some(region) = region else {
        // Nothing under the click: the selection (and History) stay as they are.
        let selected = s.active().ok_or(EngineError::NoDocument)?.doc.selection.is_some();
        return Ok(Attempt::Done(json!({ "selected": selected, "changed": false, "engine": "sam3" })));
    };
    let bbox = region.bbox;
    let selected = s.edit(req.label, |doc, _| {
        doc.selection = combine_region(base.as_ref(), Some(&region), mode);
        Ok(doc.selection.is_some())
    })?;
    if let Some((doc, revision)) = s.active().map(|d| (Arc::clone(&d.doc), d.revision)) {
        if let Some(key) = s.sam3.entry.key_mut() {
            key.holds(&doc, target);
        }
        s.sam3.sequence = Some(Sequence { doc_id, target, revision, base, mode, points });
    }
    Ok(Attempt::Done(
        json!({ "selected": selected, "changed": true, "bounds": [bbox.x0, bbox.y0, bbox.width() as i32, bbox.height() as i32], "engine": "sam3" }),
    ))
}

/// `select.objectPrepare`: start (or, with `cancel`, stop) the background encoding for the active
/// document; with `wait`, wait for it (up to 120 s). Scripts and agents use it before clicking;
/// the shell calls the same functions when the tool is picked.
fn object_prepare(s: &mut Session, p: &Value) -> Result<Value> {
    let flag = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
    let all_layers = flag("sampleAllLayers");
    if flag("cancel") {
        let was_running = s.sam3_cancel();
        return Ok(json!({ "status": s.sam3_status(all_layers).word(), "cancelled": was_running }));
    }
    let mut status = s.sam3_prepare(all_layers, Prepare::Force)?;
    if flag("wait") {
        status = s.sam3_wait(all_layers, Duration::from_secs(120));
    }
    let note = match status {
        Sam3Status::Unavailable(note) => note,
        Sam3Status::Failed(note) => Some(note),
        _ => None,
    };
    Ok(json!({ "status": status.word(), "note": note }))
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

/// The command specs of this module.
pub fn specs() -> Vec<crate::commands::CommandSpec> {
    vec![crate::commands::CommandSpec {
        id: "select.objectPrepare",
        label: "Prepare Object Selection",
        menu: &[],
        shortcut: None,
        params: r##"{"sampleAllLayers":bool=false,"cancel":bool=false (stop the encoding),"wait":bool=false (wait for it, up to 120 s)} → {status:"unavailable|idle|preparing|ready|failed|cancelled|unloaded",note} (encodes the document in the background for SAM 3 clicks; until it is ready Object Selection and Quick Selection use Classic)"##,
        enabled: has_doc,
        run: object_prepare,
        journal: false,
    }]
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_box;
