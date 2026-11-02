//! Large-hole inpainting with LaMa (Suvorov et al., "Resolution-robust Large Mask Inpainting with
//! Fourier Convolutions", WACV 2022; big-lama weights, Apache-2.0).
//!
//! The network is fixed at 512 × 512. A tight context window preserves as much scale as possible;
//! native-resolution high-pass texture from known surroundings restores detail lost on enlargement.
//! Only the masked pixels (plus a hair of feather) are composited back.

use crate::inpaint_detail::restore_detail;
use crate::resize::bilinear;
use crate::store::{Model, expected_path, model_path};
use crate::{Cancel, MlError, Rgb, check_cancel};

/// The network's fixed input side.
pub const INPAINT_SIZE: usize = 512;

/// `img` with the pixels where `mask` (`w × h`, `>= 0.5` = hole) is set replaced by what fits their
/// surroundings. An empty mask returns `img` unchanged.
pub fn inpaint(img: &Rgb, mask: &[f32]) -> Result<Rgb, MlError> {
    inpaint_with(img, mask, &|| false)
}

/// [`inpaint`] that stops with [`MlError::Cancelled`] between stages once `cancel` says so.
pub fn inpaint_with(img: &Rgb, mask: &[f32], cancel: Cancel<'_>) -> Result<Rgb, MlError> {
    inpaint_via(img, mask, cancel, &predict)
}

/// [`inpaint_with`] with an already loaded model, for callers that keep one (and for tests with a made-up
/// one, `photocraft_lama::synthetic::tiny_model`): the file is not looked for and nothing is loaded or freed.
#[cfg(not(target_arch = "wasm32"))]
pub fn inpaint_with_lama(img: &Rgb, mask: &[f32], lama: &photocraft_lama::Lama, cancel: Cancel<'_>) -> Result<Rgb, MlError> {
    inpaint_via(img, mask, cancel, &|rgb, hole, n, cancel| lama_result(lama.predict(rgb, hole, n, n, cancel), n))
}

/// The network's answer for an `n × n` window: `rgb` interleaved `0..1`, `hole` 1 where to fill; returns
/// the network's picture of the whole window, interleaved `0..1`.
type Predict<'a> = &'a dyn Fn(&[f32], &[f32], usize, Cancel<'_>) -> Result<Vec<f32>, MlError>;

fn inpaint_via(img: &Rgb, mask: &[f32], cancel: Cancel<'_>, predict: Predict<'_>) -> Result<Rgb, MlError> {
    let (w, h) = (img.w, img.h);
    Rgb::check_dims(w, h)?;
    if img.px.len() != w * h * 3 || mask.len() != w * h {
        return Err(MlError::BadInput("the pixel and mask buffers do not match the size".into()));
    }
    check_cancel(cancel)?;
    let Some(hole) = bounds(mask, w, h) else { return Ok(img.clone()) };
    let crop = context_window(hole, w, h);
    let (cw, ch) = (crop.2 - crop.0, crop.3 - crop.1);

    // Crop, scale to the network's size.
    let mut cimg = Vec::with_capacity(cw * ch * 3);
    let mut cmask = Vec::with_capacity(cw * ch);
    for y in crop.1..crop.3 {
        check_cancel(cancel)?;
        let row = y * w;
        cimg.extend_from_slice(img.px.get((row + crop.0) * 3..(row + crop.2) * 3).ok_or_else(|| MlError::BadInput("crop outside the image".into()))?);
        cmask.extend(
            mask.get(row + crop.0..row + crop.2)
                .ok_or_else(|| MlError::BadInput("crop outside the mask".into()))?
                .iter()
                .map(|m| if *m >= 0.5 { 1.0f32 } else { 0.0 }),
        );
    }
    check_cancel(cancel)?;
    let n = INPAINT_SIZE;
    let small = bilinear(&cimg, cw, ch, 3, n, n);
    // Nearest-ish for the mask: bilinear then threshold low, so a thin hole never vanishes.
    let small_mask: Vec<f32> = bilinear(&cmask, cw, ch, 1, n, n).iter().map(|m| if *m > 0.1 { 1.0 } else { 0.0 }).collect();
    check_cancel(cancel)?;
    // The network's own picture of the whole window, 0..1; only the hole (and its feather) is taken from it below.
    let filled = predict(&small, &small_mask, n, cancel)?;
    if filled.len() != 3 * n * n {
        return Err(MlError::Backend(format!("unexpected output of {} values", filled.len())));
    }
    check_cancel(cancel)?;
    let mut big = bilinear(&filled, n, n, 3, cw, ch);
    check_cancel(cancel)?;
    restore_detail(&cimg, &cmask, &mut big, cw, ch, cw.max(ch) as f32 / n as f32, cancel)?;
    check_cancel(cancel)?;

    // Composite: the hole plus a 2 px feather takes the fill; everything else is untouched.
    let alpha = feathered(&cmask, cw, ch, 2);
    check_cancel(cancel)?;
    let mut res = img.clone();
    for y in 0..ch {
        check_cancel(cancel)?;
        for x in 0..cw {
            let a = alpha.get(y * cw + x).copied().unwrap_or(0.0);
            if a <= 0.0 {
                continue;
            }
            let di = ((y + crop.1) * w + x + crop.0) * 3;
            for c in 0..3 {
                if let (Some(d), Some(f)) = (res.px.get_mut(di + c), big.get((y * cw + x) * 3 + c)) {
                    *d += (f - *d) * a;
                }
            }
        }
    }
    Ok(res)
}

/// LaMa's prediction for an `n × n` window: `rgb` interleaved `0..1`, `hole` 1 where to fill. Loads
/// `big-lama.pt`, runs it (on the GPU when there is one) and lets the model go again, one run at a time
/// (a cancelled job's worker may still be inside the network, so a retry waits for it instead of
/// stacking a second run in memory).
#[cfg(not(target_arch = "wasm32"))]
fn predict(rgb: &[f32], hole: &[f32], n: usize, cancel: Cancel<'_>) -> Result<Vec<f32>, MlError> {
    use std::sync::{Mutex, PoisonError};
    static RUNNING: Mutex<()> = Mutex::new(());
    let _one = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    check_cancel(cancel)?;
    let path = model_path(Model::Lama).ok_or_else(|| MlError::ModelMissing { model: Model::Lama, path: expected_path(Model::Lama) })?;
    lama_result(photocraft_lama::predict_from_file(&path, rgb, hole, n, n, cancel), n)
}

/// A LaMa result as this crate's: errors mapped, and values that are not numbers refused.
#[cfg(not(target_arch = "wasm32"))]
fn lama_result(result: photocraft_lama::Result<Vec<f32>>, n: usize) -> Result<Vec<f32>, MlError> {
    let out = result.map_err(|e| match e {
        photocraft_lama::Error::Cancelled => MlError::Cancelled,
        photocraft_lama::Error::Input(m) => MlError::BadInput(m),
        other => MlError::Backend(other.to_string()),
    })?;
    if out.len() != 3 * n * n || out.iter().any(|v| !v.is_finite()) {
        return Err(MlError::Backend(format!("unexpected output of {} values", out.len())));
    }
    Ok(out.iter().map(|v| v.clamp(0.0, 1.0)).collect())
}

/// The browser build has no models to run.
#[cfg(target_arch = "wasm32")]
fn predict(_: &[f32], _: &[f32], _: usize, _: Cancel<'_>) -> Result<Vec<f32>, MlError> {
    Err(MlError::Unsupported)
}

/// Bounding box `(x0, y0, x1, y1)` (exclusive end) of the set pixels.
fn bounds(mask: &[f32], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if mask.get(y * w + x).is_some_and(|m| *m >= 0.5) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    (x1 > x0).then_some((x0, y0, x1, y1))
}

/// Small holes retain up to their own width of context; larger ones get 64px per side, rather
/// than spending half the fixed network resolution on context. Keep the complete hole, slide
/// the window inside the image, and retain at least 256px for enough surrounding structure.
fn context_window(hole: (usize, usize, usize, usize), w: usize, h: usize) -> (usize, usize, usize, usize) {
    let side = (hole.2 - hole.0).max(hole.3 - hole.1);
    let want = side.saturating_add(side.min(128)).max(256);
    let place = |lo: usize, hi: usize, limit: usize| {
        let len = want.min(limit);
        let mid = (lo + hi) / 2;
        let start = mid.saturating_sub(len / 2).min(limit - len);
        (start, start + len)
    };
    let (x0, x1) = place(hole.0, hole.2, w);
    let (y0, y1) = place(hole.1, hole.3, h);
    (x0, y0, x1, y1)
}

/// `mask` (0/1) grown outward by up to `r` pixels with a linear falloff; the hole itself stays 1.
fn feathered(mask: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut out = mask.to_vec();
    if r == 0 {
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            if mask.get(y * w + x).is_some_and(|m| *m >= 0.5) {
                continue;
            }
            let mut best = 0.0f32;
            for dy in -(r as isize)..=r as isize {
                for dx in -(r as isize)..=r as isize {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    if mask.get(ny as usize * w + nx as usize).is_some_and(|m| *m >= 0.5) {
                        let d = ((dx * dx + dy * dy) as f32).sqrt();
                        best = best.max((1.0 - d / (r as f32 + 1.0)).max(0.0));
                    }
                }
            }
            if let Some(o) = out.get_mut(y * w + x) {
                *o = best;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_and_window() {
        let mut m = vec![0.0f32; 100 * 80];
        assert_eq!(bounds(&m, 100, 80), None);
        m[10 * 100 + 20] = 1.0;
        m[30 * 100 + 60] = 1.0;
        assert_eq!(bounds(&m, 100, 80), Some((20, 10, 61, 31)));
        // Window: square of 256 clipped to the image (80 high), slid inside.
        let (x0, y0, x1, y1) = context_window((20, 10, 61, 31), 100, 80);
        assert_eq!((y0, y1), (0, 80));
        assert!(x1 <= 100 && x0 < x1);
        let (x0, y0, x1, y1) = context_window((990, 990, 1000, 1000), 1000, 1000);
        assert_eq!((x1 - x0, y1 - y0), (256, 256));
        assert_eq!((x1, y1), (1000, 1000));
    }

    #[test]
    fn large_holes_keep_context_without_wasting_half_the_network() {
        let hole = (1000, 900, 1490, 1850);
        let crop = context_window(hole, 3000, 3000);
        assert_eq!((crop.2 - crop.0, crop.3 - crop.1), (1078, 1078));
        assert!(crop.0 < hole.0 && crop.1 < hole.1 && crop.2 > hole.2 && crop.3 > hole.3);
        let edge = context_window((0, 0, 950, 950), 3000, 3000);
        assert_eq!(edge, (0, 0, 1078, 1078));
    }

    #[test]
    fn pre_cancelled_inpainting_never_needs_a_model() {
        let img = Rgb::new(6, 6, vec![0.5; 108]).unwrap();
        assert_eq!(inpaint_with(&img, &[1.0; 36], &|| true), Err(MlError::Cancelled));
        assert_eq!(inpaint_with(&img, &[0.0; 36], &|| true), Err(MlError::Cancelled));
    }

    /// A picture with a flat hole, the made-up tiny LaMa: only the hole (and its feather) changes, the rest is exact.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_tiny_lama_fills_only_the_hole() {
        let (w, h) = (96, 80);
        let px: Vec<f32> = (0..w * h * 3).map(|i| ((i * 7) % 255) as f32 / 255.0).collect();
        let img = Rgb::new(w, h, px).unwrap();
        let mut mask = vec![0.0f32; w * h];
        for y in 30..50 {
            for x in 40..70 {
                mask[y * w + x] = 1.0;
            }
        }
        let lama = photocraft_lama::synthetic::tiny_model();
        let out = inpaint_with_lama(&img, &mask, &lama, &|| false).unwrap();
        assert_eq!((out.w, out.h, out.px.len()), (w, h, w * h * 3));
        assert!(out.px.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        let mut changed = 0;
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (&img.px[(y * w + x) * 3..(y * w + x) * 3 + 3], &out.px[(y * w + x) * 3..(y * w + x) * 3 + 3]);
                let inside_feather = (28..52).contains(&y) && (38..72).contains(&x);
                if !inside_feather {
                    assert_eq!(a, b, "pixel ({x},{y}) outside the hole changed");
                } else if a != b {
                    changed += 1;
                }
            }
        }
        assert!(changed > 300, "the hole was not filled ({changed} pixels changed)");
        assert_eq!(inpaint_with_lama(&img, &mask, &lama, &|| true), Err(MlError::Cancelled));
    }

    #[test]
    fn a_network_answer_of_the_wrong_size_is_an_error() {
        let img = Rgb::new(6, 6, vec![0.5; 108]).unwrap();
        let r = inpaint_via(&img, &[1.0; 36], &|| false, &|_, _, _, _| Ok(vec![0.0; 3]));
        assert!(matches!(r, Err(MlError::Backend(_))), "{r:?}");
    }

    #[test]
    fn empty_mask_is_identity_without_a_model() {
        let img = Rgb::new(6, 6, (0..108).map(|i| i as f32 / 108.0).collect()).unwrap();
        assert_eq!(inpaint(&img, &[0.0; 36]).unwrap(), img);
    }

    #[test]
    fn bad_input_is_an_error() {
        let img = Rgb::new(6, 6, vec![0.5; 108]).unwrap();
        assert!(matches!(inpaint(&img, &[1.0; 5]), Err(MlError::BadInput(_))));
    }

    #[test]
    fn feather_keeps_the_hole_and_fades_outward() {
        let mut m = vec![0.0f32; 11 * 11];
        m[5 * 11 + 5] = 1.0;
        let f = feathered(&m, 11, 11, 2);
        assert_eq!(f[5 * 11 + 5], 1.0);
        assert!(f[5 * 11 + 6] > f[5 * 11 + 7] && f[5 * 11 + 7] > 0.0);
        assert_eq!(f[5 * 11 + 9], 0.0);
    }
}
