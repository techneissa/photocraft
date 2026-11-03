//! Previews of the Home screen's recent files: decoded one file at a time on a background thread
//! (newest first, only those on screen), kept as textures while the file stays on the list, and
//! made again when the file is saved.
//!
//! The window never touches the disk for a preview: the thread reads each file's size and date
//! too (a sleeping network drive can take seconds to answer). It stops decoding as soon as the
//! Home screen is no longer drawn, so opening a document doesn't compete with previews.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use photocraft_color::{ColorMode, SampleType};

use crate::PhotocraftApp;

/// Longest side of a decoded preview in pixels: sharp in a frame of about 280 points on a 2×
/// display once cropped to fill it.
#[cfg(not(target_arch = "wasm32"))]
pub(super) const PREVIEW_SIDE: u32 = 768;

/// Files larger than this show the file icon instead of a preview: decoding a multi-gigabyte PSB
/// for a thumbnail would hold that much memory.
#[cfg(not(target_arch = "wasm32"))]
pub(super) const MAX_PREVIEW_FILE_BYTES: u64 = 512 << 20;

/// How often a shown preview checks whether its file changed.
const RECHECK: std::time::Duration = std::time::Duration::from_secs(2);

/// Previews turned into textures in one frame; the rest wait for the next frames, so a burst
/// of finished previews never stalls a frame.
const UPLOADS_PER_FRAME: usize = 2;

/// Frames without the Home screen after which the thread drops its queue.
#[cfg(not(target_arch = "wasm32"))]
const HOME_GONE_PASSES: u64 = 2;

/// What a tile shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::canvas) enum Thumb {
    /// Not decoded yet.
    Loading,
    /// The preview's texture and its size in pixels.
    Ready(egui::TextureId, egui::Vec2),
    /// The file is gone, too large, or not a picture PhotoCraft can read.
    Unavailable,
}

/// Size and modification time of a file (`None`: no such file); a different stamp means it was
/// saved again.
type Stamp = Option<(u64, std::time::SystemTime)>;

struct Entry {
    /// What the last finished check found; `None` until the first one.
    stamp: Option<Stamp>,
    checked: web_time::Instant,
    texture: Option<egui::TextureHandle>,
    /// A check or decode is queued or running.
    pending: bool,
    failed: bool,
}

/// A decoded preview, before it is turned into display colours and uploaded.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct Decoded {
    buffer: photocraft_compose::Buffer,
    mode: ColorMode,
    depth: SampleType,
    icc: Option<Arc<Vec<u8>>>,
}

/// What the thread found for one queued file.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
enum Outcome {
    /// Same size and date as last time: the preview stays.
    Unchanged,
    /// A new stamp and the preview made for it (or why there is none).
    Preview(Stamp, Result<Decoded, String>),
    /// The Home screen went away before the thread got to it.
    Skipped,
}

struct Done {
    path: String,
    outcome: Outcome,
}

/// A queued file and the stamp its preview was made for (`None`: no check finished yet).
type Job = (String, Option<Stamp>);

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    running: bool,
}

/// What the window and the decoding thread share.
#[derive(Default)]
struct Shared {
    queue: Mutex<Queue>,
    done: Mutex<VecDeque<Done>>,
    /// egui pass in which the Home screen was last drawn.
    home_pass: AtomicU64,
}

/// The previews, one per recent file.
#[derive(Default)]
pub(in crate::canvas) struct ThumbCache {
    entries: HashMap<String, Entry>,
    shared: Arc<Shared>,
}

/// The window's preview cache (in egui's memory, so the official app state is untouched).
pub(super) fn cache(ctx: &egui::Context) -> Arc<Mutex<ThumbCache>> {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Arc<Mutex<ThumbCache>>>(egui::Id::unique("home-recent-thumbs")).clone())
}

impl ThumbCache {
    /// What the tile of `path` shows now; asks for a preview the first time, and every few
    /// seconds whether the file was saved again.
    pub(super) fn thumb(&mut self, ctx: &egui::Context, path: &str) -> Thumb {
        let now = web_time::Instant::now();
        let mut job = None;
        match self.entries.get_mut(path) {
            None => {
                self.entries.insert(path.to_string(), Entry { stamp: None, checked: now, texture: None, pending: true, failed: false });
                job = Some(None);
            }
            Some(e) if !e.pending && now.duration_since(e.checked) >= RECHECK => {
                e.checked = now;
                e.pending = true;
                job = Some(e.stamp);
            }
            Some(_) => {}
        }
        if let Some(known) = job {
            self.request(ctx, (path.to_string(), known));
        }
        let Some(e) = self.entries.get(path) else { return Thumb::Unavailable };
        match &e.texture {
            Some(t) => Thumb::Ready(t.id(), t.size_vec2()),
            None if e.failed => Thumb::Unavailable,
            None => Thumb::Loading,
        }
    }

    /// Forgets previews of files no longer on the list (their textures are freed).
    pub(super) fn keep_only(&mut self, recent: &[String]) {
        self.entries.retain(|p, _| recent.contains(p));
        lock(&self.shared.queue).jobs.retain(|(p, _)| recent.contains(p));
    }

    /// Notes that the Home screen is drawn in this frame and takes what the thread finished,
    /// uploading at most [`UPLOADS_PER_FRAME`] previews.
    pub(super) fn receive(&mut self, app: &PhotocraftApp, ctx: &egui::Context) {
        self.shared.home_pass.store(ctx.cumulative_pass_nr(), Ordering::Relaxed);
        let mut uploads = 0;
        loop {
            let d = {
                let mut done = lock(&self.shared.done);
                let next_is_upload = done.front().is_some_and(|d| matches!(d.outcome, Outcome::Preview(_, Ok(_))));
                if next_is_upload && uploads >= UPLOADS_PER_FRAME {
                    ctx.request_repaint();
                    return;
                }
                match done.pop_front() {
                    Some(d) => d,
                    None => return,
                }
            };
            match d.outcome {
                Outcome::Skipped => {
                    // Asked for again the next time the Home screen shows it; a preview already
                    // made stays until then.
                    if self.entries.get(&d.path).is_some_and(|e| e.stamp.is_none()) {
                        self.entries.remove(&d.path);
                    } else if let Some(e) = self.entries.get_mut(&d.path) {
                        e.pending = false;
                    }
                }
                Outcome::Unchanged => {
                    if let Some(e) = self.entries.get_mut(&d.path) {
                        e.pending = false;
                    }
                }
                Outcome::Preview(stamp, result) => {
                    let Some(e) = self.entries.get_mut(&d.path) else { continue };
                    e.pending = false;
                    e.stamp = Some(stamp);
                    let uploaded = result.and_then(|decoded| {
                        uploads += 1;
                        upload(app, ctx, &d.path, decoded)
                    });
                    match uploaded {
                        Ok(t) => {
                            e.texture = Some(t);
                            e.failed = false;
                        }
                        Err(err) => {
                            log::debug!("no Home preview for {}: {err}", d.path);
                            e.texture = None;
                            e.failed = true;
                        }
                    }
                }
            }
        }
    }

    /// Queues a job and starts the thread if it isn't running.
    fn request(&mut self, ctx: &egui::Context, job: Job) {
        let mut q = lock(&self.shared.queue);
        q.jobs.push_back(job);
        if q.running {
            return;
        }
        q.running = true;
        drop(q);
        if let Err(e) = spawn(self.shared.clone(), ctx.clone()) {
            log::warn!("Home previews: {e}");
            let mut q = lock(&self.shared.queue);
            q.running = false;
            let failed: Vec<Done> = q.jobs.drain(..).map(|(path, _)| Done { path, outcome: Outcome::Preview(None, Err(e.to_string())) }).collect();
            drop(q);
            lock(&self.shared.done).extend(failed);
        }
    }

    #[cfg(test)]
    pub(super) fn is_known(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The preview in the canvas's display colours, as a texture.
fn upload(app: &PhotocraftApp, ctx: &egui::Context, path: &str, d: Decoded) -> Result<egui::TextureHandle, String> {
    let (w, h) = (d.buffer.rect.width(), d.buffer.rect.height());
    let mut stand_in = photocraft_doc::Document::new("", photocraft_geom::Size::new(w.max(1), h.max(1)), d.mode, d.depth);
    stand_in.icc_profile = d.icc;
    let display = app.session.color.canvas_display(&stand_in).map_err(|e| e.to_string())?;
    let img = display.to_rgba8(&d.buffer);
    let size = [img.width as usize, img.height as usize];
    if size[0] == 0 || size[1] == 0 || img.pixels.len() != size[0].saturating_mul(size[1]).saturating_mul(4) {
        return Err(format!("the preview is {}×{} with {} bytes", size[0], size[1], img.pixels.len()));
    }
    Ok(ctx.load_texture(format!("home-recent:{path}"), egui::ColorImage::from_rgba_unmultiplied(size, &img.pixels), egui::TextureOptions::LINEAR))
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(shared: Arc<Shared>, ctx: egui::Context) -> std::io::Result<()> {
    std::thread::Builder::new().name("home-previews".into()).spawn(move || work(&shared, &ctx)).map(drop)
}

#[cfg(target_arch = "wasm32")]
fn spawn(_shared: Arc<Shared>, _ctx: egui::Context) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no previews on the web"))
}

/// The thread: checks and decodes queued files until the queue is empty, then ends.
#[cfg(not(target_arch = "wasm32"))]
fn work(shared: &Shared, ctx: &egui::Context) {
    loop {
        let (path, known) = {
            let mut q = lock(&shared.queue);
            let Some(job) = q.jobs.pop_front() else {
                q.running = false;
                return;
            };
            job
        };
        // Passes advance only while the window draws; an idle Home screen keeps its queue.
        let home_gone = ctx.cumulative_pass_nr() > shared.home_pass.load(Ordering::Relaxed).saturating_add(HOME_GONE_PASSES);
        let outcome = if home_gone {
            Outcome::Skipped
        } else {
            let now = stamp(&path);
            if known == Some(now) {
                Outcome::Unchanged
            } else if now.is_none() {
                Outcome::Preview(None, Err("the file is gone".into()))
            } else {
                // A file that trips a decoder bug loses its preview, never the app.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decode(&path))).unwrap_or_else(|_| Err("the decoder failed".into()));
                Outcome::Preview(now, result)
            }
        };
        // Only a new preview (or a lost one) changes what the window shows.
        let changed = !matches!(outcome, Outcome::Unchanged);
        lock(&shared.done).push_back(Done { path, outcome });
        if changed {
            ctx.request_repaint();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn stamp(path: &str) -> Stamp {
    let m = std::fs::metadata(path).ok()?;
    m.is_file().then_some(())?;
    Some((m.len(), m.modified().ok()?))
}

/// Reads `path` as File › Open would and renders its composite at [`PREVIEW_SIDE`].
#[cfg(not(target_arch = "wasm32"))]
fn decode(path: &str) -> Result<Decoded, String> {
    let p = std::path::Path::new(path);
    let len = std::fs::metadata(p).map_err(|e| e.to_string())?.len();
    // Fork: a PSD or PSB shows the picture it carries (thumbnail, else merged image) before any layer import.
    if let Some(d) = psd_preview(p) {
        return Ok(d);
    }
    if len > MAX_PREVIEW_FILE_BYTES {
        return Err(format!("{len} bytes is too large for a preview"));
    }
    let native = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pcraft"));
    let doc = if native {
        photocraft_format::load_path(p).map_err(|e| e.to_string())?
    } else {
        let bytes = photocraft_format::read_file(p).map_err(|e| e.to_string())?;
        photocraft_io::import(&crate::file_open::display_name(path), &bytes).map_err(|e| e.to_string())?.document
    };
    let buffer = photocraft_compose::thumbnail_buffer(&doc, PREVIEW_SIDE);
    Ok(Decoded { buffer, mode: doc.mode, depth: doc.depth, icc: doc.icc_profile.clone() })
}

/// Fork: the preview a PSD or PSB carries (see `photocraft_io::fork_psd_preview`), as a [`Decoded`].
#[cfg(not(target_arch = "wasm32"))]
fn psd_preview(p: &std::path::Path) -> Option<Decoded> {
    let ext = p.extension()?.to_str()?;
    if !(ext.eq_ignore_ascii_case("psd") || ext.eq_ignore_ascii_case("psb")) {
        return None;
    }
    let img = photocraft_io::fork_psd_preview::preview(p, PREVIEW_SIDE)?;
    let rect = photocraft_geom::Rect::from_xywh(0, 0, img.width, img.height);
    let px = img.rgba.chunks_exact(4).map(|c| [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, f32::from(c[3]) / 255.0]).collect();
    Some(Decoded { buffer: photocraft_compose::Buffer { rect, px }, mode: ColorMode::Rgb, depth: SampleType::U8, icc: img.icc.map(Arc::new) })
}

/// The first pixel of the preview [`decode`] makes for `path` (for tests).
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(super) fn first_pixel(path: &str) -> Result<[f32; 4], String> {
    decode(path).map(|d| d.buffer.px[0])
}
