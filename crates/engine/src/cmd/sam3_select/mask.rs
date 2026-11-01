//! SAM 3's mask on the document's canvas: the encoded image is the canvas shrunk by a whole step
//! (so the model never sees 36 MP), clicks are normalised to it, and its soft mask is stretched
//! back with bilinear taps so the outline has no stairs and the edge stays partial selection.

use photocraft_algo::segment::trim_region;
use photocraft_algo::selection::Region;
use photocraft_geom::Rect;
use rayon::prelude::*;

use super::backend::Mask;
use crate::{EngineError, Result};

/// Longest side (pixels) the encoded image gets; the model takes a 1008 px square anyway.
pub(crate) const ENCODE_SIDE: usize = 2016;

/// The mask's most pixels (the canvas box it is expanded into), as for the neural subject mask.
const MAX_MASK_PX: usize = 64_000_000;

/// The model's coverage is a sigmoid, soft wherever it is unsure (white cloth against a white
/// wall reads 0.6 inside the object): below `LO` is not selected, above `HI` is selected fully, and
/// the part between stretches over the whole range, so the inside is solid and the edge keeps a
/// narrow soft ramp.
const LO: f32 = 0.2;
const HI: f32 = 0.8;

/// The selection coverage (`0..=255`) of the model's coverage `v`.
fn shape(v: f32) -> u8 {
    if v <= LO { 0 } else { (((v - LO) / (HI - LO)).clamp(0.0, 1.0) * 255.0).round() as u8 }
}

/// The step (canvas pixels per encoded pixel) that brings `canvas` within [`ENCODE_SIDE`].
pub(crate) fn step_for(canvas: Rect) -> usize {
    (canvas.width().max(canvas.height()) as usize).div_ceil(ENCODE_SIDE).max(1)
}

/// The click at document point `(x, y)` on the encoded image of `canvas` shrunk by `step`
/// (`enc_w × enc_h`), normalised to it. `None` outside the canvas.
pub(crate) fn normalise(canvas: Rect, step: usize, enc: (usize, usize), (x, y): (f32, f32)) -> Option<(f32, f32)> {
    if !x.is_finite() || !y.is_finite() || !canvas.contains(x.floor() as i32, y.floor() as i32) || enc.0 == 0 || enc.1 == 0 {
        return None;
    }
    let s = step.max(1) as f32;
    let nx = (x - canvas.x0 as f32) / s / enc.0 as f32;
    let ny = (y - canvas.y0 as f32) / s / enc.1 as f32;
    Some((nx.clamp(0.0, 1.0), ny.clamp(0.0, 1.0)))
}

/// At most `max` points of a stroke, evenly spread along it, the first and last always kept.
pub(crate) fn spread(points: &[(f32, f32)], max: usize) -> Vec<(f32, f32)> {
    if points.len() <= max {
        return points.to_vec();
    }
    match max {
        0 => Vec::new(),
        1 => points.first().copied().into_iter().collect(),
        _ => (0..max).filter_map(|i| points.get(i * (points.len() - 1) / (max - 1)).copied()).collect(),
    }
}

/// The mask as a [`Region`] of `canvas` (`step` canvas pixels per mask pixel), partial coverage
/// kept; `None` when it selects nothing. Only the box around what the mask covers is expanded.
pub(crate) fn to_region(mask: &Mask, canvas: Rect, step: usize) -> Result<Option<Region>> {
    let (cw, ch) = (canvas.width() as usize, canvas.height() as usize);
    let step = step.max(1);
    if mask.w == 0 || mask.h == 0 || cw == 0 || ch == 0 || mask.alpha.len() != mask.w.saturating_mul(mask.h) {
        return Err(EngineError::Other("SAM 3 returned a mask of the wrong size".into()));
    }
    // The mask pixels that count, then the canvas box they reach (a bilinear tap reaches one more).
    let (mut c0, mut r0, mut c1, mut r1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (r, row) in mask.alpha.chunks_exact(mask.w).enumerate() {
        if let (Some(a), Some(b)) = (row.iter().position(|v| *v > LO), row.iter().rposition(|v| *v > LO)) {
            c0 = c0.min(a);
            c1 = c1.max(b + 1);
            r0 = r0.min(r);
            r1 = r1.max(r + 1);
        }
    }
    if c0 == usize::MAX {
        return Ok(None);
    }
    let reach = |lo: usize, hi: usize, len: usize| (lo.saturating_sub(1).saturating_mul(step), hi.saturating_add(1).saturating_mul(step).min(len));
    let ((bx0, bx1), (by0, by1)) = (reach(c0, c1, cw), reach(r0, r1, ch));
    let (bw, bh) = (bx1.saturating_sub(bx0), by1.saturating_sub(by0));
    let count = bw
        .checked_mul(bh)
        .filter(|n| *n <= MAX_MASK_PX)
        .ok_or_else(|| EngineError::Other(format!("the SAM 3 mask exceeds the {MAX_MASK_PX}-pixel budget; choose the Classic engine or crop the document")))?;
    if count == 0 {
        return Ok(None);
    }
    let xs = super::super::smartselect_cmds::stretch_taps(cw, mask.w, step);
    let ys = super::super::smartselect_cmds::stretch_taps(ch, mask.h, step);
    let mut out = crate::allocation::filled(count, 0u8, "SAM 3 mask")?;
    let at = |row: &[f32], x: usize| row.get(x).copied().unwrap_or(0.0);
    out.par_chunks_mut(bw).enumerate().for_each(|(i, line)| {
        let Some(&(y0, y1, ty)) = ys.get(by0 + i) else { return };
        let (Some(row0), Some(row1)) = (mask.alpha.get(y0 * mask.w..(y0 + 1) * mask.w), mask.alpha.get(y1 * mask.w..(y1 + 1) * mask.w)) else { return };
        for (o, &(x0, x1, tx)) in line.iter_mut().zip(xs.iter().skip(bx0)) {
            let top = at(row0, x0) + (at(row0, x1) - at(row0, x0)) * tx;
            let bottom = at(row1, x0) + (at(row1, x1) - at(row1, x0)) * tx;
            let v = top + (bottom - top) * ty;
            *o = shape(v);
        }
    });
    let bbox = Rect::new(canvas.x0 + bx0 as i32, canvas.y0 + by0 as i32, canvas.x0 + bx1 as i32, canvas.y0 + by1 as i32);
    Ok(trim_region(Region { bbox, mask: out }))
}
