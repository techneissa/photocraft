//! What the engine needs from SAM 3, as a trait: the real model (`photocraft_sam3`) behind
//! [`Real`], nothing behind [`Unavailable`] (unit tests, the browser), and whatever a test builds.
//! The engine's tests run the click, refine, cache and fallback logic against a fake of this
//! trait, with no model.

use photocraft_ml::{Cancel, MlError, Rgb};

/// A click on the encoded image, normalised to it (`0..=1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointClick {
    pub x: f32,
    pub y: f32,
    /// Includes the object under it (false: excludes).
    pub positive: bool,
}

/// A predicted mask over the encoded image: `w × h` coverage, `0..=1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub alpha: Vec<f32>,
}

/// An image the backend holds encoded. Valid until the backend encodes another image, releases
/// it or unloads (see [`Sam3Backend::is_current`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodedImage {
    pub token: u64,
    pub w: usize,
    pub h: usize,
}

/// SAM 3 as the engine uses it. Calls may take seconds ([`Sam3Backend::encode`]) or wait for a
/// running request, so the UI thread calls only the quick ones: [`Sam3Backend::present`],
/// [`Sam3Backend::has_memory`], [`Sam3Backend::is_current`] and (about 40 ms)
/// [`Sam3Backend::clicks`].
pub trait Sam3Backend: Send + Sync + 'static {
    /// Whether the model seems to be installed. Looks at file sizes only: the SHA-256 check
    /// happens in [`Sam3Backend::encode`], off the UI thread.
    fn present(&self) -> bool;
    /// Whether this computer has the memory the model needs.
    fn has_memory(&self) -> bool;
    /// Load the model if needed and encode `rgb` (seconds; 19 GB at the peak). Stops with
    /// [`MlError::Cancelled`] between stages once `cancel` says so.
    fn encode(&self, rgb: &Rgb, cancel: Cancel<'_>) -> Result<EncodedImage, MlError>;
    /// Whether `image` is still held (not after an unload, also the idle one).
    fn is_current(&self, image: EncodedImage) -> bool;
    /// The object the `clicks` select (at least one).
    fn clicks(&self, image: EncodedImage, clicks: &[PointClick]) -> Result<Mask, MlError>;
    /// Forget `image` (the model stays loaded until it has been idle). May wait for a running request.
    fn release(&self, image: EncodedImage);
}

/// No SAM 3: nothing is installed, so the Classic finders are used.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) struct Unavailable;

#[cfg(any(test, target_arch = "wasm32"))]
impl Sam3Backend for Unavailable {
    fn present(&self) -> bool {
        false
    }
    fn has_memory(&self) -> bool {
        false
    }
    fn encode(&self, _: &Rgb, _: Cancel<'_>) -> Result<EncodedImage, MlError> {
        Err(MlError::Unsupported)
    }
    fn is_current(&self, _: EncodedImage) -> bool {
        false
    }
    fn clicks(&self, _: EncodedImage, _: &[PointClick]) -> Result<Mask, MlError> {
        Err(MlError::Unsupported)
    }
    fn release(&self, _: EncodedImage) {}
}

/// The model through [`photocraft_sam3::Sam3Service`], which holds one encoded image at a time and
/// frees the model after ten idle minutes (its reaper thread starts with the backend).
#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) struct Real {
    service: std::sync::Arc<photocraft_sam3::Sam3Service>,
    current: std::sync::Mutex<Option<(u64, photocraft_sam3::ImageHandle)>>,
    next: std::sync::atomic::AtomicU64,
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(test, allow(dead_code))]
impl Real {
    pub(crate) fn new() -> Self {
        let service = std::sync::Arc::new(photocraft_sam3::Sam3Service::new());
        // Without the reaper the model stays in memory until the app quits: still correct.
        let _ = photocraft_sam3::Sam3Service::spawn_idle_reaper(&service);
        Self { service, current: std::sync::Mutex::new(None), next: std::sync::atomic::AtomicU64::new(1) }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Option<(u64, photocraft_sam3::ImageHandle)>> {
        self.current.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn handle(&self, image: EncodedImage) -> Result<photocraft_sam3::ImageHandle, MlError> {
        match *self.held() {
            Some((token, handle)) if token == image.token => Ok(handle),
            _ => Err(MlError::BadInput("the image is no longer encoded".into())),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Sam3Backend for Real {
    fn present(&self) -> bool {
        photocraft_ml::model_present(photocraft_ml::Model::Sam3)
    }

    fn has_memory(&self) -> bool {
        photocraft_ml::check_memory(photocraft_ml::Model::Sam3).is_ok()
    }

    fn encode(&self, rgb: &Rgb, cancel: Cancel<'_>) -> Result<EncodedImage, MlError> {
        let handle = self.service.encode_with(rgb, cancel)?;
        let token = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        *self.held() = Some((token, handle));
        Ok(EncodedImage { token, w: handle.w, h: handle.h })
    }

    fn is_current(&self, image: EncodedImage) -> bool {
        self.handle(image).is_ok_and(|h| self.service.is_current(h))
    }

    fn clicks(&self, image: EncodedImage, clicks: &[PointClick]) -> Result<Mask, MlError> {
        let handle = self.handle(image)?;
        let clicks: Vec<photocraft_sam3::Click> = clicks.iter().map(|c| photocraft_sam3::Click { x: c.x, y: c.y, positive: c.positive }).collect();
        let m = self.service.clicks(handle, &clicks)?;
        Ok(Mask { w: m.w, h: m.h, alpha: m.alpha })
    }

    fn release(&self, image: EncodedImage) {
        let mut held = self.held();
        if let Some((token, handle)) = *held
            && token == image.token
        {
            *held = None;
            drop(held);
            self.service.release(handle);
        }
    }
}
