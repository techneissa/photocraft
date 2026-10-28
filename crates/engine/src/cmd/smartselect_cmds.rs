//! Smart selection commands: Quick Selection, Object Selection, Select Subject, Select and Mask
//! (Refine Edge) and Focus Area. The algorithms live in `photocraft_algo::segment` and
//! `photocraft_algo::matting`; these commands pick the pixels to analyse (active layer or
//! composite), run them, and store the result as the selection (or a mask / new layer).

use photocraft_algo::matting::{self, RefineParams};
use photocraft_algo::segment::{Sampler, SurfaceSampler, focus, grabcut, quick, subject};
use photocraft_algo::selection::{Region, SelectionMode, combine_region};
use photocraft_doc::{Document, Layer, LayerContent, LayerId, LayerMask};
use photocraft_geom::Rect;
use serde_json::{Value, json};

use super::smartselect_macos::{self, Step};
use crate::commands::CommandSpec;
use crate::prefs::SubjectEngine;
use crate::select_mask_edits;
use crate::{EngineError, Result, Session};

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}
fn has_selection_or_mask(s: &Session) -> std::result::Result<(), String> {
    let d = s.active().ok_or_else(|| "no document open".to_string())?;
    if d.doc.selection.is_some() {
        return Ok(());
    }
    if let Some(l) = d.active_layer.and_then(|id| d.doc.layer(id))
        && l.mask.is_some()
    {
        return Ok(());
    }
    Err("no selection or layer mask".into())
}

fn f(p: &Value, k: &str, d: f32) -> f32 {
    p.get(k).and_then(Value::as_f64).map_or(d, |v| v as f32)
}
fn b(p: &Value, k: &str, d: bool) -> bool {
    p.get(k).and_then(Value::as_bool).unwrap_or(d)
}
fn mode(p: &Value, default: &str) -> SelectionMode {
    let m = p.get("mode").and_then(Value::as_str).unwrap_or(default);
    SelectionMode::parse(if m == "new" { "replace" } else { m })
}
fn bad(cmd: &str, msg: &str) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

/// Composite of all visible layers.
struct CompositeSampler<'a>(&'a Document);

impl Sampler for CompositeSampler<'_> {
    fn rgba(&self, r: Rect) -> Vec<[f32; 4]> {
        photocraft_compose::render(self.0, r).px
    }
}

/// Runs `f` with a sampler over the active layer (or the composite with `all_layers`, or when the
/// active layer has no pixels).
fn with_sampler<R>(s: &Session, all_layers: bool, f: impl FnOnce(&dyn Sampler, &Document) -> R) -> Result<R> {
    let d = s.active().ok_or(EngineError::NoDocument)?;
    Ok(with_doc_sampler(&d.doc, d.active_layer, all_layers, f))
}

/// Runs `f` with a sampler over `layer`'s pixels in `doc` (or the composite with `all_layers`, or
/// when the layer has no pixels).
pub(crate) fn with_doc_sampler<R>(doc: &Document, layer: Option<LayerId>, all_layers: bool, f: impl FnOnce(&dyn Sampler, &Document) -> R) -> R {
    match (all_layers, layer.and_then(|id| doc.layer(id)).and_then(Layer::surface)) {
        (false, Some(surf)) => f(&SurfaceSampler(surf), doc),
        _ => f(&CompositeSampler(doc), doc),
    }
}

/// Stores `region` combined with the current selection by `m` as one history step.
/// Finding nothing never destroys work: the selection (and history) is left alone and the
/// caller gets `changed: false`, whatever the mode. (Select Subject and Object Selection run
/// heuristics that often find nothing on busy photos; clearing the selection then looks like
/// a dead button.)
fn apply(s: &mut Session, label: &str, region: Option<&Region>, m: SelectionMode) -> Result<Value> {
    if region.is_none() {
        // Nothing found: leave the selection (and history) alone.
        let selected = s.active().ok_or(EngineError::NoDocument)?.doc.selection.is_some();
        return Ok(json!({ "selected": selected, "changed": false }));
    }
    let bbox = region.map(|r| r.bbox);
    let selected = s.edit(label, |doc, _| {
        doc.selection = combine_region(doc.selection.as_ref(), region, m);
        Ok(doc.selection.is_some())
    })?;
    Ok(json!({ "selected": selected, "changed": true, "bounds": bbox.map(|r| [r.x0, r.y0, r.width() as i32, r.height() as i32]) }))
}

fn points(p: &Value) -> Vec<(f32, f32)> {
    p.get("points")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|q| Some((q.get(0)?.as_f64()? as f32, q.get(1)?.as_f64()? as f32))).collect())
        .unwrap_or_default()
}

/// Auto-Enhance for Quick Selection: a light edge refinement that flows the edge towards image
/// edges and removes blockiness.
const ENHANCE: RefineParams = RefineParams { radius: 4.0, smart_radius: true, smooth: 15.0, feather: 0.0, contrast: 25.0, shift_edge: 0.0 };

fn quick_selection(s: &mut Session, p: &Value) -> Result<Value> {
    let pts = points(p);
    if pts.is_empty() {
        return Err(bad("select.quick", "needs at least one point"));
    }
    let size = f(p, "size", 30.0).clamp(1.0, 5000.0);
    let m = match p.get("mode").and_then(Value::as_str).unwrap_or("add") {
        "subtract" => SelectionMode::Subtract,
        "replace" | "new" => SelectionMode::Replace,
        _ => SelectionMode::Add,
    };
    let enhance = b(p, "enhanceEdge", false);
    let region = with_sampler(s, b(p, "sampleAllLayers", false), |smp, doc| {
        let canvas = doc.bounds();
        let r = quick::quick_select(smp, canvas, &pts, size)?;
        if enhance { matting::refine_mask(smp, &matting::region_reader(&r), r.bbox, canvas, &ENHANCE) } else { Some(r) }
    })?;
    apply(s, "Quick Selection", region.as_ref(), m)
}

fn rect_param(p: &Value) -> Option<Rect> {
    let a = p.get("rect")?.as_array()?;
    let v: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
    if v.len() < 4 {
        return None;
    }
    // Negative sizes (dragging up/left) are normalised.
    let (x0, x1) = (v[0].min(v[0] + v[2]), v[0].max(v[0] + v[2]));
    let (y0, y1) = (v[1].min(v[1] + v[3]), v[1].max(v[1] + v[3]));
    Some(Rect::new(x0.floor() as i32, y0.floor() as i32, x1.ceil() as i32, y1.ceil() as i32))
}

fn point_param(p: &Value) -> Option<(f32, f32)> {
    let a = p.get("point")?.as_array()?;
    let (x, y) = (a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32);
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

/// The layer [`with_doc_sampler`] samples for `layer` / `all_layers`: `None` = the composite.
pub fn sampled_target(doc: &Document, layer: Option<LayerId>, all_layers: bool) -> Option<LayerId> {
    layer.filter(|id| !all_layers && doc.layer(*id).and_then(Layer::surface).is_some())
}

/// Object Finder (#4087): the object Object Selection would select at document point `(x, y)`,
/// sampling `layer` (or the composite). Pure, so the UI can run it on a worker thread while the
/// pointer hovers; `select.object` with `"point"` commits the same result.
pub fn find_object(doc: &Document, layer: Option<LayerId>, all_layers: bool, x: f32, y: f32) -> Option<Region> {
    with_doc_sampler(doc, layer, all_layers, |smp, doc| grabcut::object_at(smp, doc.bounds(), x, y, grabcut::OBJECT_AT_WORK_PX))
}

/// Most objects remembered by [`ObjectFinderCache`].
pub const OBJECT_FINDER_ENTRIES: usize = 12;

/// Object Finder results for one document's pixels (#4087): hovering over an object already
/// found, or clicking it, answers at once. Keyed like [`SubjectCache`] (valid while the pixels
/// are the same, the document held weakly) plus the sampled layer; a lookup hits any cached
/// object that covers the point, so one computation serves the whole object.
#[derive(Debug, Default)]
pub struct ObjectFinderCache {
    doc: std::sync::Weak<Document>,
    doc_id: Option<photocraft_doc::DocId>,
    target: Option<LayerId>,
    regions: Vec<std::sync::Arc<Region>>,
}

impl ObjectFinderCache {
    fn valid(&self, doc: &Document, target: Option<LayerId>) -> bool {
        self.doc_id == Some(doc.id) && self.target == target && self.doc.upgrade().is_some_and(|old| crate::layer_multi_cmds::same_pixels(&old, doc))
    }

    /// A cached object of `doc` (sampling `target`) covering `(x, y)`.
    pub fn lookup(&self, doc: &Document, target: Option<LayerId>, x: f32, y: f32) -> Option<std::sync::Arc<Region>> {
        if !x.is_finite() || !y.is_finite() || !self.valid(doc, target) {
            return None;
        }
        let (px, py) = (x.floor() as i32, y.floor() as i32);
        self.regions.iter().find(|r| r.at(px, py) >= 0.5).cloned()
    }

    /// Remembers `region`, found in `doc` sampling `target`. A different document or pixels
    /// start over; the oldest object is dropped beyond [`OBJECT_FINDER_ENTRIES`].
    pub fn insert(&mut self, doc: &std::sync::Arc<Document>, target: Option<LayerId>, region: std::sync::Arc<Region>) {
        if !self.valid(doc, target) {
            self.regions.clear();
        }
        self.doc = std::sync::Arc::downgrade(doc);
        self.doc_id = Some(doc.id);
        self.target = target;
        if self.regions.len() >= OBJECT_FINDER_ENTRIES {
            self.regions.remove(0);
        }
        self.regions.push(region);
    }

    /// Number of remembered objects.
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Whether nothing is remembered.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Forget the results if they were computed from the document `id` (it is closing).
    pub(crate) fn forget_doc(&mut self, id: photocraft_doc::DocId) {
        if self.doc_id == Some(id) {
            *self = Self::default();
        }
    }

    /// Follows `doc` past a selection-only edit (same pixels), so the next lookup still hits.
    fn follow(&mut self, doc: &std::sync::Arc<Document>) {
        if self.doc_id == Some(doc.id) && self.doc.upgrade().is_some_and(|old| crate::layer_multi_cmds::same_pixels(&old, doc)) {
            self.doc = std::sync::Arc::downgrade(doc);
        }
    }
}

/// Object Selection samples only the active layer by default: when that layer has no pixels in
/// `area`, say so instead of quietly finding nothing (#5246; scripts often add an empty layer
/// above the photo first).
fn check_sampled_pixels(doc: &Document, layer: Option<LayerId>, all_layers: bool, area: Rect) -> Result<()> {
    match empty_sampled_area(doc, layer, all_layers, area) {
        Some(msg) => Err(bad("select.object", &format!("{msg}; set \"sampleAllLayers\": true or select a layer with pixels"))),
        None => Ok(()),
    }
}

/// Why sampling `layer` (unless `all_layers`) can find nothing in `area`: the layer is empty, or has no pixels there.
/// `None` = there is something to sample. The click path reports it as an error, the Object Finder hover as a hint.
pub fn empty_sampled_area(doc: &Document, layer: Option<LayerId>, all_layers: bool, area: Rect) -> Option<&'static str> {
    let surf = sampled_target(doc, layer, all_layers).and_then(|id| doc.layer(id)).and_then(Layer::surface)?;
    if !surf.content_bounds().intersect(&area).is_empty() {
        return None;
    }
    Some(if surf.content_bounds().is_empty() { "the active layer is empty" } else { "the active layer has no pixels in that area" })
}

/// `select.object` with `"point"`: the Object Finder's object under the point (cached).
fn object_at_point(s: &mut Session, p: &Value, (x, y): (f32, f32)) -> Result<Value> {
    let all_layers = b(p, "sampleAllLayers", false);
    let d = s.active().ok_or(EngineError::NoDocument)?;
    check_sampled_pixels(&d.doc, d.active_layer, all_layers, Rect::from_xywh(x.floor() as i32, y.floor() as i32, 1, 1))?;
    let target = sampled_target(&d.doc, d.active_layer, all_layers);
    let region = match s.object_finder.lookup(&d.doc, target, x, y) {
        Some(r) => Some(r),
        None => {
            let doc = std::sync::Arc::clone(&d.doc);
            let found = find_object(&doc, d.active_layer, all_layers, x, y).map(std::sync::Arc::new);
            if let Some(r) = &found {
                s.object_finder.insert(&doc, target, std::sync::Arc::clone(r));
            }
            found
        }
    };
    let out = apply(s, "Object Selection", region.as_deref(), mode(p, "replace"))?;
    if let Some(d) = s.active() {
        let doc = std::sync::Arc::clone(&d.doc);
        s.object_finder.follow(&doc);
    }
    Ok(out)
}

fn object_selection(s: &mut Session, p: &Value) -> Result<Value> {
    if p.get("rect").is_none()
        && let Some(pt) = point_param(p)
    {
        return object_at_point(s, p, pt);
    }
    let rect = rect_param(p).ok_or_else(|| bad("select.object", "needs \"rect\": [x, y, w, h] or \"point\": [x, y]"))?;
    let all_layers = b(p, "sampleAllLayers", false);
    let d = s.active().ok_or(EngineError::NoDocument)?;
    check_sampled_pixels(&d.doc, d.active_layer, all_layers, rect)?;
    let region = with_sampler(s, all_layers, |smp, doc| grabcut::object_select(smp, doc.bounds(), rect, 160_000))?;
    apply(s, "Object Selection", region.as_ref(), mode(p, "replace"))
}

/// Select Subject's last result (#2865), so running it again on unchanged pixels answers at
/// once instead of recomputing superpixels, saliency and GrabCut (P29). Keyed by the pixels it
/// was computed from: valid while [`same_pixels`](crate::layer_multi_cmds::same_pixels) (a
/// tile-pointer comparison) holds against that document, so selection-only edits such as
/// Deselect keep it. One entry per session; the document is held weakly, so the cache never
/// keeps a closed or edited document's tiles alive (the session also drops it on pixel edits
/// and when its document closes). Its memory is the mask: one byte per pixel of its bounds.
#[derive(Debug, Default)]
pub struct SubjectCache {
    doc: std::sync::Weak<Document>,
    doc_id: Option<photocraft_doc::DocId>,
    /// The sampled layer; `None` = the composite.
    target: Option<LayerId>,
    region: Option<std::sync::Arc<Region>>,
    /// The engine choice it was computed with ([`SubjectEngine`]): a changed choice is a miss.
    engine: SubjectEngine,
    /// Installing, removing or disabling the AI model invalidates an automatic fallback.
    model_available: bool,
    /// The note that came with it (a fallback from the macOS cutout), shown again on a repeat.
    note: Option<&'static str>,
}

impl SubjectCache {
    fn valid(&self, doc: &Document, target: Option<LayerId>, engine: SubjectEngine, model_available: bool) -> bool {
        self.target == target
            && self.engine == engine
            && self.model_available == model_available
            && self.doc.upgrade().is_some_and(|old| crate::layer_multi_cmds::same_pixels(&old, doc))
    }

    /// Forget the result if it was computed from the document `id` (it is closing).
    pub(crate) fn forget_doc(&mut self, id: photocraft_doc::DocId) {
        if self.doc_id == Some(id) {
            *self = Self::default();
        }
    }
}

/// Largest area (pixels) Select Subject's network reads at full resolution; bigger canvases are
/// box-downsampled first and the matte is stretched back (the network sees 1024 px anyway).
const MODEL_MAX_PX: usize = 16_000_000;

/// The expanded neural mask must also fit a bounded allocation, even for sparse PSB canvases.
const MODEL_MASK_MAX_PX: usize = 64_000_000;

fn subject_model_available() -> bool {
    // Unit tests run without the model, even where a developer has it installed, so results match
    // CI (which has none) and the suite stays fast.
    !cfg!(any(test, target_arch = "wasm32")) && photocraft_ml::model_path(photocraft_ml::Model::BirefnetLite).is_some()
}

/// For each of `len` canvas pixels, the two pixels of the `small_len`-pixel matte it lies between
/// (the matte is the canvas shrunk by `step`) and the weight of the second: bilinear taps at the
/// pixel centres, clamped at the edges.
fn stretch_taps(len: usize, small_len: usize, step: usize) -> Vec<(usize, usize, f32)> {
    let last = small_len.saturating_sub(1);
    (0..len)
        .map(|i| {
            let f = ((i as f32 + 0.5) / step.max(1) as f32 - 0.5).clamp(0.0, last as f32);
            let i0 = (f as usize).min(last);
            (i0, (i0 + 1).min(last), f - i0 as f32)
        })
        .collect()
}

/// The subject's coverage from the neural matte (BiRefNet), or why the model could not run.
fn model_subject(smp: &dyn Sampler, canvas: Rect, ctl: &photocraft_raster::Interrupt) -> std::result::Result<Option<Region>, photocraft_ml::MlError> {
    ctl.check().map_err(|_| photocraft_ml::MlError::Cancelled)?;
    let (w, h) = (canvas.width() as usize, canvas.height() as usize);
    let count = w.checked_mul(h).filter(|n| *n <= MODEL_MASK_MAX_PX).ok_or_else(|| {
        photocraft_ml::MlError::BadInput(format!(
            "the neural subject mask exceeds the {MODEL_MASK_MAX_PX}-pixel budget; choose the Classic engine or crop the document"
        ))
    })?;
    let step = photocraft_algo::segment::scale_for(canvas, MODEL_MAX_PX);
    let small = smp.rgb_scaled(canvas, step);
    ctl.check().map_err(|_| photocraft_ml::MlError::Cancelled)?;
    let rgb = photocraft_ml::Rgb::new(small.w, small.h, small.px.iter().flatten().copied().collect())?;
    let matte = photocraft_ml::segment_subject_with(&rgb, &|| ctl.cancelled())?;
    ctl.check().map_err(|_| photocraft_ml::MlError::Cancelled)?;
    let mut mask = crate::allocation::filled(count, 0u8, "neural subject mask").map_err(|e| photocraft_ml::MlError::BadInput(e.to_string()))?;
    // The matte is a picture of the canvas shrunk by `step`: stretch it back with bilinear taps at
    // the pixel centres, so the outline has no `step`-wide stairs (a 24 to 36 MP photo has step 2).
    let (xs, ys) = (stretch_taps(w, small.w, step), stretch_taps(h, small.h, step));
    for (y, &(y0, y1, ty)) in ys.iter().enumerate() {
        ctl.check().map_err(|_| photocraft_ml::MlError::Cancelled)?;
        let (row0, row1) = (matte.get(y0 * small.w..(y0 + 1) * small.w), matte.get(y1 * small.w..(y1 + 1) * small.w));
        let (Some(row0), Some(row1), Some(out)) = (row0, row1, mask.get_mut(y * w..(y + 1) * w)) else { continue };
        for (o, &(x0, x1, tx)) in out.iter_mut().zip(&xs) {
            let at = |row: &[f32], x: usize| row.get(x).copied().unwrap_or(0.0);
            let top = at(row0, x0) + (at(row0, x1) - at(row0, x0)) * tx;
            let bottom = at(row1, x0) + (at(row1, x1) - at(row1, x0)) * tx;
            *o = ((top + (bottom - top) * ty).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    Ok(photocraft_algo::segment::trim_region(Region { bbox: canvas, mask }))
}

/// The note for a `model` choice (or a model-only job) with no model installed.
pub const MODEL_MISSING: &str = "AI model not installed, used Classic (Preferences › AI Models)";
/// The note for a model that could not run.
pub const MODEL_FAILED: &str = "AI model failed, used Classic";

/// What [`find_subject_region`] found, which engine found it and what to tell the user about it.
pub(crate) struct Subject {
    pub region: Option<Region>,
    /// `"macos"`, `"model"` or `"classic"`.
    pub used: &'static str,
    pub note: Option<&'static str>,
}

/// The engine a command runs with: its `engine` param (`auto`, `macos`, `model`, `classic`;
/// `classical` is the upstream spelling of `classic`) or, without one, the preference
/// `tools.selectSubjectEngine`.
pub(crate) fn engine_param(s: &Session, p: &Value) -> Result<SubjectEngine> {
    let Some(name) = p.get("engine") else { return Ok(s.prefs().tools.select_subject_engine) };
    let name = name.as_str().ok_or_else(|| bad("select.subject", "\"engine\" is a string"))?;
    SubjectEngine::parse(if name == "classical" { "classic" } else { name }).ok_or_else(|| bad("select.subject", "\"engine\" is auto, macos, model or classic"))
}

/// Finds the subject in `area` of `doc` with the engine chosen. `auto` tries the Mac's cutout, then
/// the AI model when it is installed, then Classic; `macos` the cutout, then Classic; `model` the
/// model, then Classic (with a note naming the missing model); `classic` Classic only. A finder
/// that finds nothing lets the next one try under `auto`; an explicit choice takes "no subject" as
/// the answer. Stops with `Cancelled` through `ctl`.
pub(crate) fn find_subject_region(
    doc: &Document,
    layer: Option<LayerId>,
    all_layers: bool,
    area: Rect,
    engine: SubjectEngine,
    ctl: &photocraft_raster::Interrupt,
) -> Result<Subject> {
    let cancelled = |_| EngineError::Cancelled;
    ctl.check().map_err(cancelled)?;
    let mut note = match with_doc_sampler(doc, layer, all_layers, |smp, _| smartselect_macos::current_step(engine, smp, area)) {
        Step::Found(region) => return Ok(Subject { region: Some(region), used: "macos", note: None }),
        Step::NoSubject => return Ok(Subject { region: None, used: "macos", note: None }),
        Step::Fallback(note) => note,
    };
    if matches!(engine, SubjectEngine::Auto | SubjectEngine::Model) {
        if subject_model_available() {
            match with_doc_sampler(doc, layer, all_layers, |smp, _| model_subject(smp, area, ctl)) {
                Ok(Some(region)) => return Ok(Subject { region: Some(region), used: "model", note: None }),
                Ok(None) if engine == SubjectEngine::Model => return Ok(Subject { region: None, used: "model", note: None }),
                Ok(None) => {}
                Err(photocraft_ml::MlError::Cancelled) => return Err(EngineError::Cancelled),
                // A mask beyond the output budget (or a failed allocation) must not fall through to
                // another finder that may allocate the same full-resolution area.
                Err(photocraft_ml::MlError::BadInput(message)) => return Err(EngineError::Other(message)),
                Err(_) => note = Some(MODEL_FAILED),
            }
        } else if engine == SubjectEngine::Model {
            note = Some(MODEL_MISSING);
        }
    }
    let region = with_doc_sampler(doc, layer, all_layers, |smp, _| subject::select_subject_with(smp, area, ctl)).map_err(cancelled)?;
    Ok(Subject { region, used: "classic", note })
}

fn select_subject(s: &mut Session, p: &Value) -> Result<Value> {
    let all_layers = b(p, "sampleAllLayers", false);
    let m = mode(p, "replace");
    let engine = engine_param(s, p)?;
    let d = s.active().ok_or(EngineError::NoDocument)?;
    // What `with_doc_sampler` samples: the active layer's pixels, else the composite.
    let target = d.active_layer.filter(|id| !all_layers && d.doc.layer(*id).and_then(Layer::surface).is_some());
    let available = subject_model_available();
    let c = &s.subject_cache;
    if c.valid(&d.doc, target, engine, available) {
        let (region, note) = (c.region.clone(), c.note);
        return apply_subject(s, region.as_deref(), m, note);
    }
    let doc = std::sync::Arc::clone(&d.doc);
    let layer = d.active_layer;
    // A background job when started with `Session::start`: the neural matte takes seconds, so the
    // UI shows progress and Esc cancels it instead of freezing.
    crate::jobs::run(
        s,
        "Select Subject",
        true,
        move |ctx| {
            let area = doc.bounds();
            let found = ctx.stage(0.0, 1.0, "Finding subject", |ctl| find_subject_region(&doc, layer, all_layers, area, engine, ctl))?;
            Ok((doc, found.region.map(std::sync::Arc::new), found.note))
        },
        move |s, (doc, region, note)| {
            s.subject_cache = SubjectCache {
                doc: std::sync::Arc::downgrade(&doc),
                doc_id: Some(doc.id),
                target,
                engine,
                model_available: available,
                region: region.clone(),
                note,
            };
            apply_subject(s, region.as_deref(), m, note)
        },
    )
}

fn apply_subject(s: &mut Session, region: Option<&Region>, m: SelectionMode, note: Option<&'static str>) -> Result<Value> {
    let mut out = apply(s, "Select Subject", region, m)?;
    if let (Some(note), Some(o)) = (note, out.as_object_mut()) {
        o.insert("note".into(), note.into());
    }
    // Follow the document past the selection edit just made (same pixels), so a short history
    // dropping the older state doesn't turn the next repeat into a miss.
    if let Some(d) = s.active()
        && s.subject_cache.doc_id == Some(d.doc.id)
        && s.subject_cache.doc.upgrade().is_some_and(|old| crate::layer_multi_cmds::same_pixels(&old, &d.doc))
    {
        s.subject_cache.doc = std::sync::Arc::downgrade(&d.doc);
    }
    Ok(out)
}

fn focus_area(s: &mut Session, p: &Value) -> Result<Value> {
    let (range, noise) = (f(p, "range", 0.5).clamp(0.0, 1.0), f(p, "noise", 0.0).clamp(0.0, 1.0));
    let region = with_sampler(s, b(p, "sampleAllLayers", true), |smp, doc| focus::focus_area(smp, doc.bounds(), range, noise))?;
    apply(s, "Focus Area", region.as_ref(), mode(p, "replace"))
}

fn refine_params(p: &Value) -> RefineParams {
    RefineParams {
        radius: f(p, "radius", 0.0).clamp(0.0, 250.0),
        smart_radius: b(p, "smartRadius", false),
        smooth: f(p, "smooth", 0.0).clamp(0.0, 100.0),
        feather: f(p, "feather", 0.0).clamp(0.0, 1000.0),
        contrast: f(p, "contrast", 0.0).clamp(0.0, 100.0),
        shift_edge: f(p, "shiftEdge", 0.0).clamp(-100.0, 100.0),
    }
}

/// Most Refine Edge Brush strokes, and points across them, one Select and Mask takes.
pub const MAX_STROKES: usize = 4096;
pub const MAX_STROKE_POINTS: usize = 100_000;

/// The Refine Edge Brush strokes of `refineStrokes`:
/// `[{"points":[[x,y],…],"size":px,"mode":"add|subtract"}]`, in painting order.
fn refine_strokes(p: &Value) -> Result<Vec<matting::EdgeStroke>> {
    const CMD: &str = "select.refineEdge";
    let Some(v) = p.get("refineStrokes").filter(|v| !v.is_null()) else { return Ok(Vec::new()) };
    let list = v.as_array().ok_or_else(|| bad(CMD, "refineStrokes must be an array of strokes"))?;
    if list.len() > MAX_STROKES {
        return Err(bad(CMD, &format!("refineStrokes holds more than {MAX_STROKES} strokes")));
    }
    let mut total = 0usize;
    let mut strokes = Vec::with_capacity(list.len());
    for stroke in list {
        let o = stroke.as_object().ok_or_else(|| bad(CMD, "each refineStrokes entry must be {\"points\":[[x,y],…],\"size\":px,\"mode\":\"add|subtract\"}"))?;
        let pts = o.get("points").and_then(Value::as_array).filter(|a| !a.is_empty()).ok_or_else(|| bad(CMD, "each stroke needs a non-empty points array"))?;
        total = total.saturating_add(pts.len());
        if total > MAX_STROKE_POINTS {
            return Err(bad(CMD, &format!("refineStrokes holds more than {MAX_STROKE_POINTS} points")));
        }
        let points = pts
            .iter()
            .map(|pt| {
                let xy = pt.as_array().filter(|a| a.len() == 2)?;
                let (x, y) = (xy.first()?.as_f64()?, xy.get(1)?.as_f64()?);
                let ok = |v: f64| v.is_finite() && v.abs() <= 1e7;
                (ok(x) && ok(y)).then_some((x as f32, y as f32))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| bad(CMD, "stroke points must be [x,y] pairs of finite numbers"))?;
        let range = matting::BRUSH_SIZE_RANGE;
        let size = match o.get("size") {
            None => 30.0,
            Some(v) => v
                .as_f64()
                .filter(|s| s.is_finite() && (f64::from(*range.start())..=f64::from(*range.end())).contains(s))
                .ok_or_else(|| bad(CMD, &format!("stroke size must be {}..{} px", range.start(), range.end())))? as f32,
        };
        let add = match o.get("mode").map(|m| m.as_str()) {
            None | Some(Some("add")) => true,
            Some(Some("subtract")) => false,
            Some(_) => return Err(bad(CMD, "stroke mode must be add|subtract")),
        };
        strokes.push(matting::EdgeStroke { points, size, add });
    }
    if matting::swept_area(&strokes) > matting::MAX_SWEPT_AREA {
        return Err(bad(CMD, "refineStrokes paint too large an area"));
    }
    Ok(strokes)
}

/// Select and Mask: applies the workspace's tool `edits` to the selection (or layer mask), refines
/// its edge, then outputs to the selection, a layer mask on the active layer, a new layer or a new
/// document (each with or without a mask).
/// Decontaminating colours needs new pixels, so it upgrades `selection` / `layerMask` output to
/// `newLayerWithMask` (as Photoshop does). New-layer outputs hide the source layer; new-document
/// outputs leave the source document untouched.
fn refine_edge(s: &mut Session, p: &Value) -> Result<Value> {
    let params = refine_params(p);
    let strokes = refine_strokes(p)?;
    let decontaminate = b(p, "decontaminate", false);
    let amount = f(p, "amount", 100.0).clamp(0.0, 100.0);
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let active = d.active_layer.filter(|id| d.doc.layer(*id).is_some());
    let default_output =
        if d.doc.selection.is_none() && active.and_then(|id| d.doc.layer(id)).and_then(|l| l.mask.as_ref()).is_some() { "layerMask" } else { "selection" };
    let mut output = p.get("output").and_then(Value::as_str).unwrap_or(default_output).to_string();
    if !matches!(output.as_str(), "selection" | "layerMask" | "newLayer" | "newLayerWithMask" | "newDocument" | "newDocumentWithMask") {
        return Err(bad("select.refineEdge", "output must be selection|layerMask|newLayer|newLayerWithMask|newDocument|newDocumentWithMask"));
    }
    if decontaminate && matches!(output.as_str(), "selection" | "layerMask") {
        output = "newLayerWithMask".into();
    }
    let edits = select_mask_edits::parse(p)?;
    // Before anything is painted: a thin stroke across a huge canvas paints few pixels but many tiles.
    select_mask_edits::check_size(&edits, d.doc.bounds())?;
    select_mask_edits::check_strokes(&strokes, d.doc.bounds())?;
    let region = with_sampler(s, b(p, "sampleAllLayers", false), |smp, doc| {
        let mask_surf = active.and_then(|id| doc.layer(id)).and_then(|l| l.mask.as_ref()).map(|m| &m.surface);
        let target_mask = p.get("target").and_then(Value::as_str) == Some("mask");
        let start = if target_mask { mask_surf.or(doc.selection.as_ref()) } else { doc.selection.as_ref().or(mask_surf) };
        // The workspace's tool edits change the selection before its edge is refined.
        let edited = if edits.is_empty() { None } else { select_mask_edits::apply(doc, active, start.cloned(), &edits) };
        let Some(sel) = (if edits.is_empty() { start } else { edited.as_ref() }) else { return Ok(None) };
        let brush = matting::EdgeBrush::new(&strokes, doc.bounds());
        // The refined result is dense over the selection's bounds: refuse what would not fit
        // rather than allocate it (a thin edit across a huge canvas; the #8783 review).
        let content = sel.content_bounds();
        let area = matting::refine_area(content, doc.bounds(), &params, brush.as_ref());
        if u64::from(area.width()) * u64::from(area.height()) > matting::MAX_REFINE_AREA {
            return Err(bad(
                "select.refineEdge",
                &format!("the selection is too large to refine ({} × {} px; at most {} px in all)", area.width(), area.height(), matting::MAX_REFINE_AREA),
            ));
        }
        Ok(matting::refine_mask_brushed(smp, &matting::surface_reader(sel), content, doc.bounds(), &params, brush.as_ref()))
    })??;
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let active = d.active_layer.filter(|id| d.doc.layer(*id).is_some());
    match output.as_str() {
        "selection" => {
            let selected = s.edit("Select and Mask", |doc, _| {
                doc.selection = region.as_ref().map(matting::region_surface);
                Ok(doc.selection.is_some())
            })?;
            Ok(json!({ "output": output, "selected": selected }))
        }
        "layerMask" => {
            let id = active.ok_or_else(|| EngineError::Other("no active layer".into()))?;
            s.edit("Select and Mask", |doc, _| {
                let surface =
                    region.as_ref().map(matting::region_surface).unwrap_or_else(|| photocraft_raster::Surface::new(photocraft_color::PixelFormat::GRAY8));
                crate::extra_cmds::background_to_layer_for_mask(doc, id);
                let existing = doc.layer(id).and_then(|l| l.mask.as_ref()).cloned().unwrap_or_else(LayerMask::reveal_all);
                doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?.mask = Some(LayerMask { surface, ..existing });
                doc.selection = None;
                Ok(())
            })?;
            Ok(json!({ "output": output, "layer": id.0, "selected": false }))
        }
        _ => {
            let id = active.ok_or_else(|| EngineError::Other("no active layer".into()))?;
            let src_layer = d.doc.layer(id).ok_or(EngineError::NoLayer(id))?;
            let src = src_layer.surface().ok_or_else(|| {
                EngineError::Other(format!("the active layer is {} {} layer without pixels", src_layer.content.article(), src_layer.content.kind_name()))
            })?;
            let name = d.doc.copy_name(&src_layer.name);
            let empty = Region { bbox: Rect::EMPTY, mask: Vec::new() };
            let reg = region.as_ref().unwrap_or(&empty);
            let pixels = if decontaminate { matting::decontaminate(src, reg, params.radius, amount) } else { src.clone() };
            let with_mask = matches!(output.as_str(), "newLayerWithMask" | "newDocumentWithMask");
            let (content, mask) = if with_mask {
                (pixels, Some(LayerMask { surface: matting::region_surface(reg), ..LayerMask::reveal_all() }))
            } else {
                (matting::masked_copy(&pixels, reg), None)
            };
            if output.starts_with("newDocument") {
                // Same canvas, colour mode, depth and profile, so the pixels need no conversion.
                let mut doc = Document::new("Untitled", d.doc.size, d.doc.mode, d.doc.depth);
                doc.resolution_dpi = d.doc.resolution_dpi;
                doc.icc_profile = d.doc.icc_profile.clone();
                doc.color_table = d.doc.color_table.clone();
                doc.duotone = d.doc.duotone.clone();
                let mut layer = Layer::new(src_layer.name.clone(), LayerContent::Raster(content));
                layer.mask = mask;
                let layer_id = layer.id;
                doc.layers.push(layer);
                let i = s.add_document(doc, None);
                return Ok(json!({ "output": output, "document": i, "layer": layer_id.0, "selected": false }));
            }
            let new_id = s.edit("Select and Mask", |doc, act| {
                let mut layer = Layer::new(name, LayerContent::Raster(content));
                layer.mask = mask;
                let nid = doc.insert_above(Some(id), layer);
                if let Some(l) = doc.layer_mut(id) {
                    l.visible = false;
                }
                doc.selection = None;
                *act = Some(nid);
                Ok(nid)
            })?;
            Ok(json!({ "output": output, "layer": new_id.0, "selected": false }))
        }
    }
}

macro_rules! spec {
    ($id:literal, $label:literal, [$($m:literal),*], $params:literal, $en:expr, $run:expr) => {
        CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: None, params: $params, enabled: $en, run: $run, journal: true }
    };
}

/// Smart selection command specs.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "select.quick",
            "Quick Selection",
            [],
            r##"{"points":[[x,y],…],"size":px=30,"mode":"add|subtract|replace"="add","sampleAllLayers":bool=false,"enhanceEdge":bool=false}"##,
            has_doc,
            quick_selection
        ),
        spec!(
            "select.object",
            "Object Selection",
            [],
            r##"{"rect":[x,y,w,h] | "point":[x,y],"mode":"replace|add|subtract|intersect"="replace","sampleAllLayers":bool=false} (point: the Object Finder's object under the point, as hovering previews it)"##,
            has_doc,
            object_selection
        ),
        spec!(
            "select.subject",
            "Subject",
            ["Select"],
            r##"{"sampleAllLayers":bool=false (false = the selected layer, true = all visible layers),"mode":"replace|add|subtract|intersect"="replace","engine":"auto|macos|model|classic"=the preference tools.selectSubjectEngine (auto = the Mac's cutout, else the AI model when installed, else Classic; model without the model installed = Classic with a note)}"##,
            has_doc,
            select_subject
        ),
        spec!(
            "select.refineEdge",
            "Refine Edge",
            [],
            r##"{"radius":px=0,"smartRadius":bool=false,"smooth":0..100=0,"feather":px=0,"contrast":0..100=0,"shiftEdge":-100..100=0,"decontaminate":bool=false,"amount":0..100=100,"output":"selection|layerMask|newLayer|newLayerWithMask|newDocument|newDocumentWithMask"="selection","sampleAllLayers":bool=false,"refineStrokes":[{"points":[[x,y],…],"size":px=30,"mode":"add|subtract"="add"},…]=[],"edits":[{"tool":"quick","points":[[x,y],…],"size":px=30,"mode":"add|subtract"="add","sampleAllLayers":bool=false} | {"tool":"brush","points":[[x,y],…],"size":px=30,"hardness":0..100=100,"mode":"add|subtract"="add"} | {"tool":"lasso","points":[[x,y],…≥3],"mode":"add|subtract"="add"} | {"tool":"object","rect":[x,y,w,h] | "point":[x,y],"mode":"add|subtract"="add","sampleAllLayers":bool=false} | {"tool":"subject","mode":"replace|add|subtract"="replace","sampleAllLayers":bool=true} | {"tool":"clear"} | {"tool":"invert"},…]=[]} (refineStrokes: the Refine Edge Brush, in painting order: add refines the painted area from the image, subtract keeps the original edge there; edits: the workspace's Quick Selection, Brush, Lasso, Object Selection, Select Subject, Clear Selection and Invert, applied in order to the selection before its edge is refined)"##,
            has_selection_or_mask,
            refine_edge
        ),
        spec!(
            "select.focusArea",
            "Focus Area…",
            ["Select"],
            r##"{"range":0..1=0.5,"noise":0..1=0,"sampleAllLayers":bool=true,"mode":"replace|add|subtract|intersect"="replace"}"##,
            has_doc,
            focus_area
        ),
    ]
}

#[cfg(test)]
mod tests;
