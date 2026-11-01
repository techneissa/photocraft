//! The API the editor uses: [`Sam3Service`] finds and loads the model, encodes an image once,
//! answers clicks and text prompts from that encoding, and frees the model after use or when it
//! has been idle.
//!
//! Images come in as `photocraft_ml::Rgb` and masks go out as [`Matte`]s (`w × h` coverage,
//! `0..=1`, like the matte `photocraft-ml` hands back for BiRefNet); failures are
//! `photocraft_ml::MlError`s, so the caller reports them like any other model failure.
//!
//! The model is several GB. One service keeps one model and one encoded image at a time, and runs
//! one request at a time (a second caller waits for the first). Every request runs under
//! `catch_unwind`: a panic inside candle becomes an error and drops the possibly inconsistent
//! model, which loads again on the next request. Where the model runs on the GPU (Metal on macOS)
//! and fails there, it is freed and the request runs once more on the CPU; when that works, the
//! service stays on the CPU. `PHOTOCRAFT_SAM3_CPU=1` keeps it on the CPU from the start.

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError, Weak};
use std::time::{Duration, Instant};

use photocraft_ml::{Cancel, MAX_SIDE, MlError, Model, Rgb};

use crate::{Click, Encoded, Error, MASK_SIDE, Prediction, Sam3, mask};

/// Free the model after this long without a request (the idle reaper, [`Sam3Service::spawn_idle_reaper`]).
pub const IDLE_UNLOAD: Duration = Duration::from_secs(10 * 60);

/// Environment variable: when set (and not `0`), the model runs on the CPU only.
pub const CPU_VAR: &str = "PHOTOCRAFT_SAM3_CPU";

/// An encoded image, as the service remembers it. Cheap to copy; valid until the service encodes
/// another image, [`Sam3Service::release`]s it or unloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageHandle {
    id: u64,
    /// Width of the encoded image, in pixels.
    pub w: usize,
    /// Height of the encoded image, in pixels.
    pub h: usize,
}

/// A predicted mask over the encoded image.
#[derive(Clone, Debug, PartialEq)]
pub struct Matte {
    pub w: usize,
    pub h: usize,
    /// `w × h` coverage, `0..=1` (row-major): 1 on the selected object(s).
    pub alpha: Vec<f32>,
    /// The model's confidence, `0..=1`.
    pub score: f32,
}

impl Matte {
    /// The matte of `p` resampled to `w × h`.
    fn from_prediction(p: &Prediction, w: usize, h: usize) -> Self {
        Self { w, h, alpha: mask::alpha(&p.logits, MASK_SIDE, w, h), score: p.score }
    }
}

/// What a request asks of the model.
#[derive(Clone, Copy)]
enum Prompt<'a> {
    Clicks(&'a [Click]),
    Text(&'a str, f32),
}

/// What a request returns before it becomes a [`Matte`].
type Answer = Option<Prediction>;

/// Where the model files come from.
#[derive(Clone, Debug)]
enum Source {
    /// The folder `photocraft-ml` finds for [`Model::Sam3`].
    Models,
    /// A fixed folder (tests, tools).
    Dir(PathBuf),
}

/// Why a request failed inside.
#[derive(Debug)]
enum Failure {
    /// The model failed (a candle error or a panic): on the GPU, the CPU may run it.
    Model(String),
    /// Anything else (no files, bad input, low memory): another device won't help.
    Other(MlError),
}

impl From<MlError> for Failure {
    fn from(e: MlError) -> Self {
        Self::Other(e)
    }
}

impl Failure {
    fn into_ml(self) -> MlError {
        match self {
            Self::Model(m) => MlError::Backend(m),
            Self::Other(e) => e,
        }
    }
}

/// A candle failure may be the GPU's (the CPU may run it); the rest will fail on any device.
fn from_sam3(e: Error) -> Failure {
    match e {
        Error::Missing(path) => Failure::Other(MlError::ModelMissing { model: Model::Sam3, path: path.display().to_string() }),
        Error::Model(m) => Failure::Other(MlError::Backend(m)),
        Error::Candle(e) => Failure::Model(e.to_string()),
    }
}

/// An image the service holds: its 8-bit pixels (to encode again on another device) and, once
/// computed, the model's features.
struct Cached {
    id: u64,
    w: usize,
    h: usize,
    rgb: Vec<u8>,
    enc: Option<Encoded>,
}

#[derive(Default)]
struct State {
    model: Option<(PathBuf, Sam3)>,
    image: Option<Cached>,
    /// The model failed on the GPU and works on the CPU: it stays there.
    cpu_only: bool,
    last_used: Option<Instant>,
}

/// SAM 3 for the editor. Create one (they are cheap: nothing loads until a request), share it in
/// an `Arc`, and call it from worker threads, never from the UI thread.
pub struct Sam3Service {
    source: Source,
    state: Mutex<State>,
    loaded: AtomicBool,
    busy: AtomicBool,
    next_id: AtomicU64,
}

impl Default for Sam3Service {
    fn default() -> Self {
        Self::new()
    }
}

/// Raises a flag while alive (also when unwinding).
struct Flag<'a>(&'a AtomicBool);

impl<'a> Flag<'a> {
    fn raise(f: &'a AtomicBool) -> Self {
        f.store(true, Ordering::SeqCst);
        Self(f)
    }
}

impl Drop for Flag<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Sam3Service {
    /// A service that loads the model from where `photocraft-ml` finds it ([`Model::Sam3`]).
    pub fn new() -> Self {
        Self::with_source(Source::Models)
    }

    /// A service that loads the model from `dir` (`model.safetensors`, `vocab.json`, `merges.txt`)
    /// whatever the models folders hold: tests and tools.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self::with_source(Source::Dir(dir.into()))
    }

    fn with_source(source: Source) -> Self {
        Self { source, state: Mutex::new(State::default()), loaded: AtomicBool::new(false), busy: AtomicBool::new(false), next_id: AtomicU64::new(1) }
    }

    /// Whether the model files are installed (does not load them).
    pub fn is_installed(&self) -> bool {
        match &self.source {
            Source::Models => photocraft_ml::model_location(Model::Sam3).is_some(),
            Source::Dir(dir) => crate::is_model_dir(dir),
        }
    }

    /// Whether the model is in memory now.
    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Whether a request is loading, encoding or answering right now (the slow parts).
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    fn wants_cpu(&self, st: &State) -> bool {
        st.cpu_only || !crate::GPU_BUILD || std::env::var_os(CPU_VAR).is_some_and(|v| !v.is_empty() && v != "0")
    }

    /// Load the model now (so the first image encodes without it), unless it is loaded.
    pub fn preload(&self) -> Result<(), MlError> {
        let mut st = lock(&self.state);
        let _busy = Flag::raise(&self.busy);
        let cpu = self.wants_cpu(&st);
        self.guarded(&mut st, cpu, |this, st, cpu| this.ensure_model(st, cpu).map(|_| ()))
    }

    /// Encode `img` for clicks and text (loading the model first when needed). The encoding stays
    /// in the service until another image is encoded, [`Sam3Service::release`] or an unload; a
    /// request for the handle after that is `MlError::BadInput`.
    pub fn encode(&self, img: &Rgb) -> Result<ImageHandle, MlError> {
        self.encode_with(img, &|| false)
    }

    /// [`Sam3Service::encode`] that stops with [`MlError::Cancelled`] between stages once `cancel`
    /// says so (a stage itself, the model's own work, cannot be interrupted).
    pub fn encode_with(&self, img: &Rgb, cancel: Cancel<'_>) -> Result<ImageHandle, MlError> {
        check_image(img)?;
        let rgb = to_u8(img);
        let mut st = lock(&self.state);
        let _busy = Flag::raise(&self.busy);
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        st.image = Some(Cached { id, w: img.w, h: img.h, rgb, enc: None });
        let handle = ImageHandle { id, w: img.w, h: img.h };
        let cpu = self.wants_cpu(&st);
        let result = self.guarded(&mut st, cpu, |this, st, cpu| {
            this.ensure_model(st, cpu)?;
            if cancel() {
                return Err(MlError::Cancelled.into());
            }
            Self::ensure_encoded(st, id, true)
        });
        if let Err(e) = result {
            st.image = None;
            return Err(e);
        }
        if cancel() {
            st.image = None;
            return Err(MlError::Cancelled);
        }
        Ok(handle)
    }

    /// The object the `clicks` select in the encoded image (positions normalised to the image,
    /// `0..=1`; at least one click).
    pub fn clicks(&self, image: ImageHandle, clicks: &[Click]) -> Result<Matte, MlError> {
        if clicks.is_empty() {
            return Err(MlError::BadInput("no clicks".into()));
        }
        if clicks.iter().any(|c| !c.x.is_finite() || !c.y.is_finite()) {
            return Err(MlError::BadInput("a click is not a position".into()));
        }
        let clicks: Vec<Click> = clicks.iter().map(|c| Click { x: c.x.clamp(0.0, 1.0), y: c.y.clamp(0.0, 1.0), positive: c.positive }).collect();
        self.answer(image, Prompt::Clicks(&clicks))?.ok_or_else(|| MlError::Backend("the model returned no mask".into()))
    }

    /// Every instance of the concept `text` names ("the red car", "sky"), merged into one matte;
    /// `None` when nothing scores above `threshold` (`0..=1`; 0.5 is the reference default).
    pub fn text(&self, image: ImageHandle, text: &str, threshold: f32) -> Result<Option<Matte>, MlError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(MlError::BadInput("no text".into()));
        }
        if !threshold.is_finite() {
            return Err(MlError::BadInput("the threshold is not a number".into()));
        }
        self.answer(image, Prompt::Text(text, threshold.clamp(0.0, 1.0)))
    }

    /// Whether `image` is still the encoded image: `false` after another image, a
    /// [`Sam3Service::release`], an unload (also the idle one) or a failed model. Answers at once;
    /// while a request is running it says `true` (that request decides).
    pub fn is_current(&self, image: ImageHandle) -> bool {
        match self.state.try_lock() {
            Ok(st) => st.image.as_ref().is_some_and(|c| c.id == image.id),
            Err(TryLockError::Poisoned(p)) => p.into_inner().image.as_ref().is_some_and(|c| c.id == image.id),
            Err(TryLockError::WouldBlock) => true,
        }
    }

    /// Forget the encoded image (the model stays loaded).
    pub fn release(&self, image: ImageHandle) {
        let mut st = lock(&self.state);
        if st.image.as_ref().is_some_and(|c| c.id == image.id) {
            st.image = None;
        }
    }

    /// Free the model and the encoded image now (several GB). The next request loads it again.
    pub fn unload(&self) {
        let mut st = lock(&self.state);
        self.drop_all(&mut st);
    }

    /// [`Sam3Service::unload`] when nothing has used the model for `idle`. A request in progress
    /// counts as use. Returns whether it freed the model.
    pub fn unload_if_idle(&self, idle: Duration) -> bool {
        let mut st = match self.state.try_lock() {
            Ok(st) => st,
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
            Err(TryLockError::WouldBlock) => return false,
        };
        if st.model.is_some() && st.last_used.is_none_or(|t| t.elapsed() >= idle) {
            self.drop_all(&mut st);
            log::info!("SAM 3 unloaded after {} minutes without use", idle.as_secs() / 60);
            return true;
        }
        false
    }

    /// Start a thread that calls [`Sam3Service::unload_if_idle`] with [`IDLE_UNLOAD`] every 30
    /// seconds while `service` lives (it ends when the last `Arc` is dropped).
    pub fn spawn_idle_reaper(service: &Arc<Self>) -> std::io::Result<()> {
        let weak: Weak<Self> = Arc::downgrade(service);
        std::thread::Builder::new().name("sam3-idle".into()).spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                let Some(service) = weak.upgrade() else { return };
                service.unload_if_idle(IDLE_UNLOAD);
            }
        })?;
        Ok(())
    }

    fn drop_all(&self, st: &mut State) {
        st.model = None;
        st.image = None;
        self.loaded.store(false, Ordering::SeqCst);
    }

    fn answer(&self, image: ImageHandle, prompt: Prompt<'_>) -> Result<Option<Matte>, MlError> {
        let mut st = lock(&self.state);
        if st.image.as_ref().is_none_or(|c| c.id != image.id) {
            return Err(MlError::BadInput("the image is no longer encoded; encode it again".into()));
        }
        let _busy = Flag::raise(&self.busy);
        let cpu = self.wants_cpu(&st);
        let (w, h) = (image.w, image.h);
        let p = self.guarded(&mut st, cpu, |this, st, cpu| {
            this.ensure_model(st, cpu)?;
            Self::ensure_encoded(st, image.id, false)?;
            Self::predict(st, prompt)
        })?;
        Ok(p.map(|p| Matte::from_prediction(&p, w, h)))
    }

    /// Run `op`; a model failure on the GPU frees the model and runs `op` once more on the CPU,
    /// and the service stays there if that works.
    fn guarded<T>(&self, st: &mut State, cpu: bool, mut op: impl FnMut(&Self, &mut State, bool) -> Result<T, Failure>) -> Result<T, MlError> {
        st.last_used = Some(Instant::now());
        let first = self.attempt(st, cpu, &mut op);
        let result = match first {
            Err(Failure::Model(why)) if !cpu && crate::GPU_BUILD => {
                log::warn!("SAM 3 failed on the GPU ({why}); trying the CPU");
                self.drop_model(st);
                match self.attempt(st, true, &mut op) {
                    Ok(v) => {
                        log::warn!("SAM 3 runs on the CPU from now on: it failed on the GPU and works on the CPU");
                        st.cpu_only = true;
                        Ok(v)
                    }
                    Err(e) => {
                        // the CPU fails too, so the GPU is not the cause: the next request tries it again
                        self.drop_model(st);
                        Err(e)
                    }
                }
            }
            other => other,
        };
        st.last_used = Some(Instant::now());
        result.map_err(|f| {
            if matches!(f, Failure::Model(_)) {
                self.drop_model(st);
            }
            f.into_ml()
        })
    }

    /// Drop the model but keep the image's pixels, so it can be encoded again on another device.
    fn drop_model(&self, st: &mut State) {
        st.model = None;
        if let Some(c) = st.image.as_mut() {
            c.enc = None;
        }
        self.loaded.store(false, Ordering::SeqCst);
    }

    fn attempt<T>(&self, st: &mut State, cpu: bool, op: &mut impl FnMut(&Self, &mut State, bool) -> Result<T, Failure>) -> Result<T, Failure> {
        match std::panic::catch_unwind(AssertUnwindSafe(|| op(self, st, cpu))) {
            Ok(r) => r,
            Err(p) => {
                let why = p.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
                log::error!("SAM 3 panicked: {why}");
                // don't trust what the model kept: it loads again on the next request
                self.drop_model(st);
                Err(Failure::Model("the AI model failed unexpectedly (it will be reloaded on the next try)".into()))
            }
        }
    }

    /// The folder to load from, after the checks that don't need the files open.
    fn model_dir(&self) -> Result<PathBuf, MlError> {
        match &self.source {
            Source::Dir(dir) => Ok(dir.clone()),
            Source::Models => {
                let Some(found) = photocraft_ml::model_location(Model::Sam3) else {
                    let path = photocraft_ml::install_dir(Model::Sam3).map_or_else(|| Model::Sam3.id().to_string(), |d| d.display().to_string());
                    return Err(MlError::ModelMissing { model: Model::Sam3, path });
                };
                photocraft_ml::check_memory(Model::Sam3)?;
                Ok(found.dir)
            }
        }
    }

    fn ensure_model(&self, st: &mut State, cpu: bool) -> Result<(), Failure> {
        let dir = self.model_dir()?;
        if st.model.as_ref().is_some_and(|(d, m)| *d == dir && (!cpu || m.device().is_cpu())) {
            return Ok(());
        }
        self.drop_model(st);
        let t = Instant::now();
        let model = if cpu { Sam3::load_cpu(&dir) } else { Sam3::load(&dir) }.map_err(from_sam3)?;
        log::info!("SAM 3 loaded in {:?} ({})", t.elapsed(), if model.device().is_cpu() { "CPU" } else { "GPU" });
        st.model = Some((dir, model));
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Encode the image `id` (and, with `clicks`, prepare the click decoder's features so the first
    /// click is fast) unless that is done.
    fn ensure_encoded(st: &mut State, id: u64, clicks: bool) -> Result<(), Failure> {
        let State { model, image, .. } = st;
        let (Some((_, model)), Some(c)) = (model.as_mut(), image.as_mut()) else {
            return Err(MlError::BadInput("the image is no longer encoded; encode it again".into()).into());
        };
        if c.id != id {
            return Err(MlError::BadInput("the image is no longer encoded; encode it again".into()).into());
        }
        if c.enc.is_none() {
            let t = Instant::now();
            let enc = model.encode(&c.rgb, c.w, c.h).map_err(from_sam3)?;
            log::info!("SAM 3 encoded {}×{} in {:?}", c.w, c.h, t.elapsed());
            c.enc = Some(enc);
        }
        if clicks && let Some(enc) = c.enc.as_mut() {
            model.prepare_clicks(enc).map_err(from_sam3)?;
            // The GPU queues its work: wait for it, so the encoding is done when this returns (its
            // time is the encoding's, a GPU failure surfaces here where the CPU fallback can catch
            // it, and the queued intermediates of one image do not pile up behind the next).
            crate::sync(model).map_err(from_sam3)?;
        }
        Ok(())
    }

    fn predict(st: &mut State, prompt: Prompt<'_>) -> Result<Answer, Failure> {
        let State { model, image, .. } = st;
        let (Some((_, model)), Some(enc)) = (model.as_mut(), image.as_mut().and_then(|c| c.enc.as_mut())) else {
            return Err(Failure::Model("the model is not ready".into()));
        };
        match prompt {
            Prompt::Clicks(clicks) => model.segment_clicks(enc, clicks).map(Some).map_err(from_sam3),
            Prompt::Text(text, threshold) => model.segment_text(enc, text, threshold).map_err(from_sam3),
        }
    }
}

/// An image the model can take: a sane size and a buffer of `w × h × 3` samples.
fn check_image(img: &Rgb) -> Result<(), MlError> {
    if img.w == 0 || img.h == 0 || img.w > MAX_SIDE || img.h > MAX_SIDE {
        return Err(MlError::BadInput(format!("a {}×{} image", img.w, img.h)));
    }
    let want = img.w.checked_mul(img.h).and_then(|n| n.checked_mul(3));
    if want != Some(img.px.len()) {
        return Err(MlError::BadInput(format!("{} samples for a {}×{} RGB image", img.px.len(), img.w, img.h)));
    }
    Ok(())
}

/// 8-bit RGB of `img` (the model takes 8-bit input; not-a-number samples become 0).
fn to_u8(img: &Rgb) -> Vec<u8> {
    img.px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

#[cfg(test)]
mod tests;
