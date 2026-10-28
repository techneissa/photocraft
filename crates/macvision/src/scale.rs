//! The pure part: how big Vision's input is, shrinking the picture for it, scaling the mask back
//! up, and reading Vision's pixel buffers. Tested on every platform.

use crate::Mask;

/// Vision gets at most this many pixels (it works at a few megapixels; more only costs memory).
pub const MAX_VISION_PIXELS: u64 = 4_000_000;

/// The largest picture [`crate::subject_mask`] accepts (16384 × 16384).
pub const MAX_INPUT_PIXELS: u64 = 1 << 28;

/// Vision is given nothing thinner than this on either side.
pub(crate) const MIN_SIDE: u32 = 8;

/// True when a picture of this size (the size Vision would get) is too thin for Vision on a side:
/// a strip has no subject to cut out, and Vision must not be asked.
pub(crate) fn too_thin(width: u32, height: u32) -> bool {
    width < MIN_SIDE || height < MIN_SIDE
}

/// The size Vision is given for a `width × height` picture: the picture itself when it has at most
/// [`MAX_VISION_PIXELS`] pixels, else shrunk to fit with its aspect ratio kept (each side ≥ 1).
pub fn vision_size(width: u32, height: u32) -> (u32, u32) {
    let px = u64::from(width) * u64::from(height);
    if px <= MAX_VISION_PIXELS || width == 0 || height == 0 {
        return (width, height);
    }
    let k = (MAX_VISION_PIXELS as f64 / px as f64).sqrt();
    let fit = |v: u32| ((f64::from(v) * k).floor() as u32).clamp(1, v);
    (fit(width), fit(height))
}

/// Area-average `src` (straight RGBA8, `from`) down to `to`. `None` when a size is zero, `to` is
/// larger than `from` on an axis, or the buffer does not match `from`.
pub(crate) fn box_downscale_rgba(src: &[u8], from: (u32, u32), to: (u32, u32)) -> Option<Vec<u8>> {
    let ((sw, sh), (dw, dh)) = ((from.0 as usize, from.1 as usize), (to.0 as usize, to.1 as usize));
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || dw > sw || dh > sh || src.len() != sw.checked_mul(sh)?.checked_mul(4)? {
        return None;
    }
    let mut out = vec![0u8; dw.checked_mul(dh)?.checked_mul(4)?];
    // The source span `start..end` of output index `i` along an axis: `start < end <= n_src`.
    let span = |i: usize, n_src: usize, n_dst: usize| -> (usize, usize) {
        let start = i.saturating_mul(n_src) / n_dst;
        let end = (i + 1).saturating_mul(n_src) / n_dst;
        (start.min(n_src - 1), end.max(start + 1).min(n_src))
    };
    // Source column span of each output column, computed once.
    let cols: Vec<(usize, usize)> = (0..dw).map(|x| span(x, sw, dw)).collect();
    for (y, out_row) in out.chunks_exact_mut(dw * 4).enumerate() {
        let (y0, y1) = span(y, sh, dh);
        for (x, px) in out_row.chunks_exact_mut(4).enumerate() {
            let (x0, x1) = cols.get(x).copied()?;
            let mut sum = [0u32; 4];
            for sy in y0..y1 {
                let row_start = sy.checked_mul(sw)?;
                let row = src.get(row_start.checked_add(x0)?.checked_mul(4)?..row_start.checked_add(x1)?.checked_mul(4)?)?;
                for p in row.chunks_exact(4) {
                    for (s, v) in sum.iter_mut().zip(p) {
                        *s += u32::from(*v);
                    }
                }
            }
            let n = u32::try_from((y1 - y0).checked_mul(x1 - x0)?).ok()?;
            for (o, s) in px.iter_mut().zip(sum) {
                *o = u8::try_from((s + n / 2) / n).unwrap_or(u8::MAX);
            }
        }
    }
    Some(out)
}

/// Bilinear resample of `m` to `width × height`, pixel centres aligned (so a constant mask stays
/// constant and the picture does not shift).
pub(crate) fn resize_bilinear(m: &Mask, width: u32, height: u32) -> Option<Mask> {
    let ((sw, sh), (dw, dh)) = ((m.width as usize, m.height as usize), (width as usize, height as usize));
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || m.data.len() != sw.checked_mul(sh)? {
        return None;
    }
    // An allocation sized by the caller is capped (a mask is never bigger than a picture we accept).
    if (dw as u64).checked_mul(dh as u64)? > MAX_INPUT_PIXELS {
        return None;
    }
    let mut data = vec![0u8; dw.checked_mul(dh)?];
    if (sw, sh) == (dw, dh) {
        data.copy_from_slice(&m.data);
        return Some(Mask { width, height, data });
    }
    // For each output index: the two source indices and the weight of the second.
    let taps = |dn: usize, sn: usize| -> Vec<(usize, usize, f32)> {
        (0..dn)
            .map(|i| {
                let c = ((i as f32 + 0.5) * sn as f32 / dn as f32 - 0.5).clamp(0.0, (sn - 1) as f32);
                let a = c.floor() as usize;
                (a, (a + 1).min(sn - 1), c - a as f32)
            })
            .collect()
    };
    let (xt, yt) = (taps(dw, sw), taps(dh, sh));
    for (y, row) in data.chunks_exact_mut(dw).enumerate() {
        let (y0, y1, fy) = yt.get(y).copied()?;
        let (r0, r1) = (m.data.get(y0 * sw..(y0 + 1) * sw)?, m.data.get(y1 * sw..(y1 + 1) * sw)?);
        for (o, &(x0, x1, fx)) in row.iter_mut().zip(&xt) {
            let at = |r: &[u8], x: usize| r.get(x).copied().map(f32::from);
            let top = at(r0, x0)? * (1.0 - fx) + at(r0, x1)? * fx;
            let bottom = at(r1, x0)? * (1.0 - fx) + at(r1, x1)? * fx;
            *o = (top * (1.0 - fy) + bottom * fy + 0.5) as u8;
        }
    }
    Some(Mask { width, height, data })
}

/// One coverage value from a float in 0..=1 (NaN and out-of-range values are clamped).
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(crate) fn coverage(v: f32) -> u8 {
    if v.is_nan() { 0 } else { (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8 }
}

/// The pixel layouts of Vision's mask buffers this crate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(crate) enum Layout {
    /// `kCVPixelFormatType_OneComponent32Float`: one native-endian `f32` per pixel.
    Float32,
    /// `kCVPixelFormatType_OneComponent8`: one byte per pixel.
    Byte,
}

/// Read a `width × height` mask out of a pixel buffer's bytes, rows `bytes_per_row` apart (rows may
/// be padded). `None` when the bytes are too short for the size.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(crate) fn read_mask(bytes: &[u8], bytes_per_row: usize, width: u32, height: u32, layout: Layout) -> Option<Mask> {
    let (w, h) = (width as usize, height as usize);
    let item = match layout {
        Layout::Float32 => 4,
        Layout::Byte => 1,
    };
    if w == 0 || h == 0 || bytes_per_row < w.checked_mul(item)? || bytes.len() < bytes_per_row.checked_mul(h - 1)?.checked_add(w * item)? {
        return None;
    }
    let mut data = Vec::with_capacity(w.checked_mul(h)?);
    for y in 0..h {
        let row = bytes.get(y * bytes_per_row..y * bytes_per_row + w * item)?;
        match layout {
            Layout::Float32 => data.extend(row.chunks_exact(4).map(|c| coverage(f32::from_ne_bytes([c[0], c[1], c[2], c[3]])))),
            Layout::Byte => data.extend_from_slice(row),
        }
    }
    Some(Mask { width, height, data })
}
