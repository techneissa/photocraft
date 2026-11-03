//! Subject matte from the full BiRefNet (Swin-L, `photocraft-birefnet` on candle: Metal on the Mac),
//! the `model.safetensors` LightCraft shares. Same contract as [`crate::segment_subject_with`]
//! (BiRefNet-lite): a soft `w × h` matte, the edges pulled onto the photo's own by the same guided filter.

use crate::resize::bilinear;
use crate::segment::refine_edges;
use crate::{Cancel, MlError, Rgb};

/// The network's fixed input side (the same as BiRefNet-lite's).
pub const SEGMENT_FULL_SIZE: usize = photocraft_birefnet::INPUT_SIDE;

/// The subject's soft matte (`w × h`, `0..=1`) of `img` from the full BiRefNet: 1 on the subject, 0 on the
/// background. [`MlError::ModelMissing`] when the file is not installed (or fails its size and SHA-256 check),
/// [`MlError::LowMemory`] on a computer without the memory, [`MlError::Backend`] when loading or running fails
/// (a panic inside the GPU code included); callers then fall back to BiRefNet-lite or Classic. The model is
/// loaded for this run and freed after it, one run at a time.
pub fn segment_subject_full_with(img: &Rgb, cancel: Cancel<'_>) -> Result<Vec<f32>, MlError> {
    Rgb::check_dims(img.w, img.h)?;
    if img.px.len() != img.w * img.h * 3 {
        return Err(MlError::BadInput("the pixel buffer does not match the size".into()));
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = cancel;
        Err(MlError::Unsupported)
    }
    #[cfg(not(target_arch = "wasm32"))]
    native::run(img, cancel)
}

/// The 8-bit RGB the network reads, from the normalised photo.
fn to_u8(px: &[f32]) -> Vec<u8> {
    px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// A sigmoid matte at the network's size, stretched back to `w × h` and refined along the photo's edges.
fn finish(logits: &[f32], img: &Rgb) -> Result<Vec<f32>, MlError> {
    let n = SEGMENT_FULL_SIZE;
    if logits.len() != n * n {
        return Err(MlError::Backend(format!("unexpected output of {} values", logits.len())));
    }
    let matte: Vec<f32> = logits.iter().map(|x| 1.0 / (1.0 + (-x).exp())).collect();
    let mut full = bilinear(&matte, n, n, 1, img.w, img.h);
    refine_edges(img, &mut full);
    Ok(full)
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Mutex, PoisonError};

    use super::{finish, to_u8};
    use crate::store::{Model, expected_path, model_location};
    use crate::{Cancel, MlError, Rgb, check_cancel};

    /// One inference at a time: a cancelled job's worker may still be inside the network, so a retry
    /// waits for it instead of stacking a second model in memory.
    static RUNNING: Mutex<()> = Mutex::new(());

    pub(super) fn run(img: &Rgb, cancel: Cancel<'_>) -> Result<Vec<f32>, MlError> {
        let _one = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        check_cancel(cancel)?;
        crate::ram::check_memory(Model::Birefnet)?;
        let at = model_location(Model::Birefnet).ok_or_else(|| MlError::ModelMissing { model: Model::Birefnet, path: expected_path(Model::Birefnet) })?;
        let rgb = to_u8(&img.px);
        // A panic in the GPU code must end as an error the caller can fall back from, never as a crash.
        let logits = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<f32>, MlError> {
            let net = photocraft_birefnet::BiRefNet::load(&at.dir).map_err(backend)?;
            check_cancel(cancel)?;
            let matte = net.matte_with(&rgb, img.w, img.h, cancel).map_err(backend)?;
            Ok(matte.logits)
        }))
        .unwrap_or_else(|_| Err(MlError::Backend("the BiRefNet model stopped unexpectedly".into())))?;
        check_cancel(cancel)?;
        finish(&logits, img)
    }

    fn backend(e: photocraft_birefnet::Error) -> MlError {
        match e {
            photocraft_birefnet::Error::Cancelled => MlError::Cancelled,
            other => MlError::Backend(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Model;

    #[test]
    fn bad_input_is_an_error() {
        assert!(matches!(segment_subject_full_with(&Rgb { w: 0, h: 0, px: vec![] }, &|| false), Err(MlError::BadInput(_))));
        assert!(matches!(segment_subject_full_with(&Rgb { w: 4, h: 4, px: vec![0.0; 5] }, &|| false), Err(MlError::BadInput(_))));
    }

    #[test]
    fn cancelled_before_anything_loads() {
        let img = Rgb { w: 8, h: 8, px: vec![0.5; 8 * 8 * 3] };
        assert_eq!(segment_subject_full_with(&img, &|| true), Err(MlError::Cancelled));
    }

    #[test]
    fn a_missing_model_is_reported_not_loaded() {
        crate::disable_models();
        let img = Rgb { w: 8, h: 8, px: vec![0.5; 8 * 8 * 3] };
        // Models are switched off here (as in every test): the same answer as a folder without the file.
        let r = segment_subject_full_with(&img, &|| false);
        assert!(matches!(r, Err(MlError::ModelMissing { model: Model::Birefnet, .. }) | Err(MlError::LowMemory { .. }) | Err(MlError::Unsupported)), "{r:?}");
    }

    #[test]
    fn photos_become_bytes_and_logits_become_a_matte_of_the_photos_size() {
        assert_eq!(to_u8(&[0.0, 1.0, 0.5, -1.0, 2.0]), vec![0, 255, 128, 0, 255]);
        let n = SEGMENT_FULL_SIZE;
        let img = Rgb { w: 6, h: 4, px: vec![0.5; 6 * 4 * 3] };
        // Left half subject (high logits), right half background.
        let logits: Vec<f32> = (0..n * n).map(|i| if i % n < n / 2 { 12.0 } else { -12.0 }).collect();
        let m = finish(&logits, &img).unwrap();
        assert_eq!(m.len(), 6 * 4);
        assert!(m[0] > 0.99 && m[5] < 0.01, "{m:?}");
        assert!(matches!(finish(&logits[..10], &img), Err(MlError::Backend(_))));
    }
}
