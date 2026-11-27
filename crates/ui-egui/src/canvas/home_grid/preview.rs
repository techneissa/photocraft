//! The pictures and sizes behind Home's recent files: a PSD or PSB shows the picture inside it (its thumbnail, else
//! its merged image) without importing its layers, and each decoded file's size in pixels is kept for the dim line.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

/// The sizes in pixels of the recent files the previews found, by path; shared with the decoding jobs.
#[derive(Clone, Default)]
pub(crate) struct Sizes(Arc<Mutex<HashMap<String, (u32, u32)>>>);

impl Sizes {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, (u32, u32)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The size of `path` once its preview was decoded.
    pub(crate) fn get(&self, path: &str) -> Option<(u32, u32)> {
        self.map().get(path).copied()
    }

    /// Keeps or forgets the size of `path`: a file saved again, or taken off the list, has a new one or none.
    pub(crate) fn set(&self, path: &str, size: Option<(u32, u32)>) {
        match size {
            Some(size) => self.map().insert(path.to_string(), size),
            None => self.map().remove(path),
        };
    }
}

/// Longest side of a PSD's merged-image preview, as a card needs: sharp on a 2× display.
#[cfg(not(target_arch = "wasm32"))]
const PREVIEW_SIDE: u32 = 768;

/// How much of a file is read to find its size: a header sits at the start of every format.
#[cfg(not(target_arch = "wasm32"))]
const HEADER_BYTES: u64 = 4 << 20;

/// The picture of the file at `path` (`len` bytes) for its card, and its size noted in `sizes`. A PSD or PSB gives
/// the picture it carries; anything else, or a PSD with none, goes through `official`, the framework app's bounded
/// loader.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load(path: &str, len: u64, sizes: &Sizes, official: fn(&str, u64) -> Option<egui::ColorImage>) -> Option<egui::ColorImage> {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    if (ext.eq_ignore_ascii_case("psd") || ext.eq_ignore_ascii_case("psb"))
        && let Some(img) = photocraft_io::fork_psd_preview::preview(p, PREVIEW_SIDE)
        && img.rgba.len() == img.width as usize * img.height as usize * 4
    {
        sizes.set(path, Some(img.full));
        return Some(egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba));
    }
    let image = official(path, len);
    sizes.set(path, image.as_ref().and_then(|_| file_size(p)));
    image
}

/// The size in pixels of the picture file at `p`, read from its start; `None` for a format whose header is not
/// there (a `.pcraft` shows its stored preview, not its size).
#[cfg(not(target_arch = "wasm32"))]
fn file_size(p: &std::path::Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = Vec::new();
    std::fs::File::open(p).ok()?.take(HEADER_BYTES).read_to_end(&mut head).ok()?;
    photocraft_codecs::probe_size(&head).ok().filter(|&(w, h)| w > 0 && h > 0)
}
