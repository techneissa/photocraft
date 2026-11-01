//! Refine Hair: alpha for thin, semi-transparent structure (hair, fur, wisps) around a selection.
//!
//! The selection becomes a trimap: where it is selected and at least `radius` pixels from its
//! edge is **sure foreground**, where it is not and at least `radius` pixels from its edge is
//! **sure background**, and the band between (`radius` px on each side of the edge) is
//! **unknown**. Alpha in the unknown band is then solved with **closed-form matting** (A. Levin,
//! D. Lischinski and Y. Weiss, "A Closed-Form Solution to Natural Image Matting", IEEE PAMI 2008):
//! in every 3×3 window the image is assumed to be a blend of two colours, so alpha is an affine
//! function of the pixel colour there. That makes the cost
//! `αᵀLα`, with the matting Laplacian
//!
//! ```text
//! L(i, j) = Σ over windows k ∋ i, j of  δij − 1/9 · (1 + (Iᵢ − μk)ᵀ (Σk + ε/9 · I₃)⁻¹ (Iⱼ − μk))
//! ```
//!
//! quadratic in alpha, and with the trimap's pixels held fixed its minimum is the solution of one
//! sparse symmetric positive-definite system over the unknown pixels. Solving it (matrix-free
//! preconditioned conjugate gradients: `Lx` is computed window by window from each window's mean,
//! covariance inverse and alpha sums, as the guided filter does) lets alpha follow a hair strand
//! that is thinner than a pixel, which a local linear filter such as the guided filter of
//! [`crate::matting`] smears or loses.
//!
//! Work is tiled like [`crate::matting::refine_mask`]: every 256 × 256 tile solves its own band
//! with a margin of context (in parallel on native targets), so memory is bounded by the tile and
//! not by the image, and tiles with no edge cost one read of the mask. The result is dense over
//! the selection's bounds grown by the radius ([`MAX_HAIR_AREA`] bounds that).

use photocraft_geom::Rect;

use crate::matting::box_mean;
use crate::segment::{RgbImage, Sampler, trim_region};
use crate::selection::{Region, edt};

/// The band radius used when none is given (px each side of the selection's edge).
pub const DEFAULT_RADIUS: f32 = 10.0;
/// Widest band radius (px each side): a larger request is cut to it, which bounds a tile's memory.
pub const MAX_RADIUS: f32 = 64.0;
/// Most pixels the result may cover (the selection's bounds grown by the radius, within the
/// canvas): 2²⁷, a 16k × 8k image. The result is dense at a byte a pixel.
pub const MAX_HAIR_AREA: u64 = 1 << 27;

const TILE: i32 = 256;
/// The matting Laplacian's regulariser (colours in 0–1): the paper uses 10⁻⁷; 10⁻⁴ (the guided
/// filter's too) keeps photographs' noise, JPEG blocks and the texture of a flat region from being
/// taken for a foreground–background blend, and conjugate gradients converge in about half the
/// iterations.
pub const EPSILON: f64 = 1.0e-4;
/// Pulls pixels no known pixel reaches towards their starting value, and keeps the system definite.
const ANCHOR: f32 = 1.0e-5;
/// Conjugate gradients stop at this root-mean-square residual, or after [`MAX_ITERATIONS`].
const TOLERANCE: f64 = 2.0e-5;
/// Weight of the colour prior and the colour-line distance it tolerates (see [`Tuning`]).
const PRIOR: f32 = 0.2;
const TOLERANCE_COLOUR: f32 = 0.06;
const DEVIATION: f32 = 0.12;
/// How far beyond the selection the result reaches, in radii: hair that hangs off the band is
/// followed that far.
pub const REACH_FACTOR: f32 = 3.0;
const MAX_ITERATIONS: usize = 400;

/// The band's width inside the selection, as a fraction of its width outside.
pub const INNER_FRACTION: f32 = 0.4;

/// Trimap labels of [`solve`].
pub const UNKNOWN: u8 = 0;
pub const BACKGROUND: u8 = 1;
pub const FOREGROUND: u8 = 2;

/// Why Refine Hair refused.
#[derive(Clone, Debug, PartialEq)]
pub enum HairError {
    /// The radius is negative, not finite or absurdly large.
    BadRadius,
    /// The result would cover more than [`MAX_HAIR_AREA`] pixels.
    TooLarge { width: u32, height: u32 },
}

impl std::fmt::Display for HairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HairError::BadRadius => write!(f, "the Refine Hair radius must be a number from 0 to 1000 px"),
            HairError::TooLarge { width, height } => {
                write!(f, "the selection is too large for Refine Hair ({width} × {height} px; at most {MAX_HAIR_AREA} px in all)")
            }
        }
    }
}

impl std::error::Error for HairError {}

/// The band radius to use for a requested `radius`: [`DEFAULT_RADIUS`] for 0, else the request
/// within 1 to [`MAX_RADIUS`].
pub fn effective_radius(radius: f32) -> Result<f32, HairError> {
    if !radius.is_finite() || !(0.0..=1000.0).contains(&radius) {
        return Err(HairError::BadRadius);
    }
    Ok(if radius == 0.0 { DEFAULT_RADIUS } else { radius.clamp(1.0, MAX_RADIUS) })
}

/// The area [`refine_hair`] computes over: `content` (clipped to `canvas`) grown by the radius,
/// within `canvas`. Empty when `content` misses the canvas.
pub fn hair_area(content: Rect, canvas: Rect, radius: f32) -> Rect {
    let content = content.intersect(&canvas);
    if content.is_empty() {
        return Rect::EMPTY;
    }
    let r = (effective_radius(radius).unwrap_or(DEFAULT_RADIUS) * REACH_FACTOR).ceil() as i32;
    content.inflate(r + 1).intersect(&canvas)
}

fn area_px(r: Rect) -> u64 {
    u64::from(r.width()) * u64::from(r.height())
}

/// Refuses an area past [`MAX_HAIR_AREA`], and a bad `radius`.
pub fn check_size(content: Rect, canvas: Rect, radius: f32) -> Result<(), HairError> {
    effective_radius(radius)?;
    let area = hair_area(content, canvas, radius);
    if area_px(area) > MAX_HAIR_AREA {
        return Err(HairError::TooLarge { width: area.width(), height: area.height() });
    }
    Ok(())
}

/// Refine Hair over a selection. `mask(r)` returns its coverage over `r` (row-major); `content`
/// bounds its non-zero pixels; `sampler` supplies the image; the result is clipped to `canvas`.
/// `radius` is the band's width each side of the selection's edge (see [`effective_radius`]).
/// `None` when nothing remains selected.
pub fn refine_hair(
    sampler: &dyn Sampler,
    mask: &(dyn Fn(Rect) -> Vec<f32> + Sync),
    content: Rect,
    canvas: Rect,
    radius: f32,
) -> Result<Option<Region>, HairError> {
    refine_hair_protected(sampler, mask, content, canvas, radius, None)
}

/// How far (px) a protected subject's outline is pulled in before it counts as sure foreground
/// (see [`refine_hair_protected`]): a fraction of the band radius, within 1 to 4 px.
pub fn protect_erosion(radius: f32) -> f32 {
    (effective_radius(radius).unwrap_or(DEFAULT_RADIUS) * PROTECT_ERODE_FRACTION).clamp(1.0, 4.0)
}

/// [`protect_erosion`]'s fraction of the radius.
const PROTECT_ERODE_FRACTION: f32 = 0.15;

/// [`refine_hair`] with a `protect` region (a subject mask from a model, in canvas coordinates):
/// its pixels, pulled in by [`protect_erosion`], are sure foreground wherever the trimap would
/// leave them to the matting, and stay fully selected in the result. Deep shadows at the
/// selection's edge, which the colours read as background, are no longer cut away; strands outside
/// the region are solved as before. `None` is exactly [`refine_hair`].
pub fn refine_hair_protected(
    sampler: &dyn Sampler,
    mask: &(dyn Fn(Rect) -> Vec<f32> + Sync),
    content: Rect,
    canvas: Rect,
    radius: f32,
    protect: Option<&Region>,
) -> Result<Option<Region>, HairError> {
    check_size(content, canvas, radius)?;
    let radius = effective_radius(radius)?;
    let bbox = hair_area(content, canvas, radius);
    if bbox.is_empty() {
        return Ok(None);
    }
    // The edge's distance is exact within `reach` of a pixel, whose context is `ctx` more.
    let reach = radius.ceil() as i32;
    let ctx = (radius.ceil() as i32 + 8).clamp(16, 72);
    let halo = ctx + reach + 2;
    let mut tiles = Vec::new();
    let mut y = bbox.y0;
    while y < bbox.y1 {
        let mut x = bbox.x0;
        while x < bbox.x1 {
            tiles.push(Rect::new(x, y, (x + TILE).min(bbox.x1), (y + TILE).min(bbox.y1)));
            x += TILE;
        }
        y += TILE;
    }
    let run = |t: &Rect| -> (Rect, Vec<u8>) {
        let cw = t.width() as usize * t.height() as usize;
        let m_rect = t.inflate(halo).intersect(&canvas);
        let m = mask(m_rect);
        let (mw, mh) = (m_rect.width() as usize, m_rect.height() as usize);
        if m.len() != mw * mh {
            return (*t, vec![0; cw]);
        }
        let first = m.first().copied().unwrap_or(0.0);
        if (first == 0.0 || first == 1.0) && m.iter().all(|v| *v == first) {
            return (*t, vec![if first == 1.0 { 255 } else { 0 }; cw]);
        }
        let solve_rect = t.inflate(ctx).intersect(&canvas);
        let pin = protect.and_then(|p| pinned(p, m_rect, protect_erosion(radius)));
        let tr = Trimap::new(&m, m_rect, solve_rect, radius, pin.as_deref());
        let has_unknown = tr.kind.contains(&UNKNOWN);
        let mut alpha = if has_unknown {
            let img = sampler.rgb(solve_rect);
            if img.w == tr.w && img.px.len() == tr.kind.len() { solve(&img, &tr.kind, &tr.init, radius).alpha } else { tr.init.clone() }
        } else {
            tr.init.clone()
        };
        // The solver may free a sure pixel whose colour is far from its neighbours' (a deep shadow):
        // a protected one stays fully selected.
        for (a, _) in alpha.iter_mut().zip(&tr.pinned).filter(|(_, p)| **p) {
            *a = 1.0;
        }
        // The tile's core out of the solved rectangle.
        let mut out = Vec::with_capacity(cw);
        for yy in t.y0..t.y1 {
            let row = (yy - solve_rect.y0) as usize * tr.w;
            for xx in t.x0..t.x1 {
                let v = alpha.get(row + (xx - solve_rect.x0) as usize).copied().unwrap_or(0.0);
                out.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        (*t, out)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let results: Vec<(Rect, Vec<u8>)> = {
        use rayon::prelude::*;
        tiles.par_iter().map(run).collect()
    };
    #[cfg(target_arch = "wasm32")]
    let results: Vec<(Rect, Vec<u8>)> = tiles.iter().map(run).collect();
    let bw = bbox.width() as usize;
    let mut out = vec![0u8; bw * bbox.height() as usize];
    for (t, data) in results {
        let cw = t.width() as usize;
        for (yy, row) in data.chunks_exact(cw.max(1)).enumerate() {
            let o = ((t.y0 - bbox.y0) as usize + yy) * bw + (t.x0 - bbox.x0) as usize;
            if let Some(dst) = out.get_mut(o..o + cw) {
                dst.copy_from_slice(row);
            }
        }
    }
    Ok(trim_region(Region { bbox, mask: out }))
}

/// Which pixels of `rect` (row-major) lie inside `protect` by more than `erosion` px, `None` when
/// none do.
fn pinned(protect: &Region, rect: Rect, erosion: f32) -> Option<Vec<bool>> {
    let region = protect.bbox.intersect(&rect);
    if region.is_empty() {
        return None;
    }
    let (w, h) = (rect.width() as usize, rect.height() as usize);
    let mut outside = vec![true; w * h];
    let bw = protect.bbox.width() as usize;
    let mut any = false;
    for y in region.y0..region.y1 {
        for x in region.x0..region.x1 {
            let src = (y - protect.bbox.y0) as usize * bw + (x - protect.bbox.x0) as usize;
            if protect.mask.get(src).is_some_and(|v| *v >= 128)
                && let Some(o) = outside.get_mut((y - rect.y0) as usize * w + (x - rect.x0) as usize)
            {
                *o = false;
                any = true;
            }
        }
    }
    if !any {
        return None;
    }
    // Distance to the nearest pixel that is not protected; a pixel of the region's own edge is at 1.
    let dist = edt(&outside, w, h);
    let pins: Vec<bool> = dist.iter().zip(&outside).map(|(d, out)| !*out && *d > erosion).collect();
    pins.contains(&true).then_some(pins)
}

/// The trimap of one solve rectangle, from a selection read over a larger one.
struct Trimap {
    w: usize,
    kind: Vec<u8>,
    /// The pixels a protected subject holds as sure foreground (all `false` without one).
    pinned: Vec<bool>,
    /// Starting alpha: 1 and 0 where known, a ramp across the band where not.
    init: Vec<f32>,
}

impl Trimap {
    /// `m` is the selection over `m_rect` (row-major); the trimap covers `solve` inside it.
    fn new(m: &[f32], m_rect: Rect, solve: Rect, radius: f32, pin: Option<&[bool]>) -> Trimap {
        let (mw, mh) = (m_rect.width() as usize, m_rect.height() as usize);
        let inside: Vec<bool> = m.iter().map(|v| *v >= 0.5).collect();
        let mut edge = vec![false; mw * mh];
        for y in 0..mh {
            for x in 0..mw {
                let i = y * mw + x;
                let v = inside[i];
                edge[i] = (x > 0 && inside[i - 1] != v)
                    || (x + 1 < mw && inside[i + 1] != v)
                    || (y > 0 && inside[i - mw] != v)
                    || (y + 1 < mh && inside[i + mw] != v);
            }
        }
        let dist = edt(&edge, mw, mh);
        let (w, h) = (solve.width() as usize, solve.height() as usize);
        let mut kind = vec![BACKGROUND; w * h];
        let mut init = vec![0.0f32; w * h];
        let mut pinned = vec![false; w * h];
        let (ox, oy) = ((solve.x0 - m_rect.x0) as usize, (solve.y0 - m_rect.y0) as usize);
        for y in 0..h {
            for x in 0..w {
                let mi = (y + oy) * mw + x + ox;
                let (Some(&inn), Some(&d)) = (inside.get(mi), dist.get(mi)) else { continue };
                let i = y * w + x;
                // Hair lies mostly outside a selection's edge; inside it, shadows and dark skin
                // look like background, so less of the inside is left to the colours.
                let band = if inn { (radius * INNER_FRACTION).max(1.0) } else { radius };
                if d < band {
                    kind[i] = UNKNOWN;
                    // Distance 0 is the edge pixel itself: half way.
                    let ramp = ((d + 0.5) / (2.0 * band)).min(0.5);
                    init[i] = if inn { 0.5 + ramp } else { 0.5 - ramp };
                    // The protected subject is sure foreground, wherever the selection's edge puts it.
                    if pin.and_then(|p| p.get(mi)).copied().unwrap_or(false) {
                        kind[i] = FOREGROUND;
                        init[i] = 1.0;
                        pinned[i] = true;
                    }
                } else if inn {
                    kind[i] = FOREGROUND;
                    init[i] = 1.0;
                    pinned[i] = pin.and_then(|p| p.get(mi)).copied().unwrap_or(false);
                }
            }
        }
        Trimap { w, kind, pinned, init }
    }
}

/// The result of [`solve`].
pub struct Solution {
    /// Alpha for every pixel (1 and 0 where the trimap was known).
    pub alpha: Vec<f32>,
    /// Conjugate-gradient iterations used.
    pub iterations: usize,
}

/// The 3×3 windows that hold an unknown pixel: top-left pixel index, mean colour and the
/// inverse of the regularised covariance (`m00 m01 m02 m11 m12 m22`).
struct Windows {
    tl: Vec<u32>,
    mu: Vec<[f32; 3]>,
    inv: Vec<[f32; 6]>,
}

fn windows(img: &RgbImage, kind: &[u8], epsilon: f64) -> Windows {
    let (w, h) = (img.w, img.h);
    let mut win = Windows { tl: Vec::new(), mu: Vec::new(), inv: Vec::new() };
    for y in 0..h.saturating_sub(2) {
        for x in 0..w.saturating_sub(2) {
            let tl = y * w + x;
            let ids = |f: &mut dyn FnMut(usize)| {
                for dy in 0..3 {
                    for dx in 0..3 {
                        f(tl + dy * w + dx);
                    }
                }
            };
            let mut any = false;
            ids(&mut |i| any |= kind.get(i) == Some(&UNKNOWN));
            if !any {
                continue;
            }
            let mut mean = [0.0f64; 3];
            ids(&mut |i| {
                for (c, m) in mean.iter_mut().enumerate() {
                    *m += f64::from(img.px.get(i).map_or(0.0, |p| p[c]));
                }
            });
            mean = mean.map(|v| v / 9.0);
            // Covariance (divided by 9) plus ε/9 on the diagonal: Σ + ε/|w| · I.
            let mut cov = [[0.0f64; 3]; 3];
            ids(&mut |i| {
                let p = img.px.get(i).copied().unwrap_or([0.0; 3]);
                let d = [0, 1, 2].map(|c| f64::from(p[c]) - mean[c]);
                for (a, row) in cov.iter_mut().enumerate() {
                    for (b, v) in row.iter_mut().enumerate() {
                        *v += d[a] * d[b] / 9.0;
                    }
                }
            });
            for (a, row) in cov.iter_mut().enumerate() {
                if let Some(v) = row.get_mut(a) {
                    *v += epsilon / 9.0;
                }
            }
            let inv = invert_sym3(&cov, epsilon);
            win.tl.push(tl as u32);
            win.mu.push(mean.map(|v| v as f32));
            win.inv.push(inv.map(|v| v as f32));
        }
    }
    win
}

/// The inverse of a symmetric positive-definite 3×3 matrix (`m00 m01 m02 m11 m12 m22`); the
/// identity scaled by a large number when it is singular (a window the ε term should prevent).
fn invert_sym3(c: &[[f64; 3]; 3], epsilon: f64) -> [f64; 6] {
    let (a, b, d) = (c[0][0], c[0][1], c[0][2]);
    let (e, f, g) = (c[1][1], c[1][2], c[2][2]);
    let (c00, c01, c02) = (e * g - f * f, d * f - b * g, b * f - d * e);
    let det = a * c00 + b * c01 + d * c02;
    if !det.is_finite() || det <= 1e-30 {
        let s = 9.0 / epsilon;
        return [s, 0.0, 0.0, s, 0.0, s];
    }
    let k = 1.0 / det;
    [c00 * k, c01 * k, c02 * k, (a * g - d * d) * k, (b * d - a * f) * k, (a * e - b * b) * k]
}

/// `y = (L x)` on the unknown pixels, from the windows; `x` holds alpha everywhere (known pixels
/// with the values they are fixed at), `y` is written on unknown pixels only.
fn laplacian(win: &Windows, img: &RgbImage, unknown: &[bool], x: &[f32], y: &mut [f32]) {
    let w = img.w;
    for ((tl, mu), m) in win.tl.iter().zip(&win.mu).zip(&win.inv) {
        let tl = *tl as usize;
        let mut ids = [0usize; 9];
        for dy in 0..3 {
            for dx in 0..3 {
                if let Some(slot) = ids.get_mut(dy * 3 + dx) {
                    *slot = tl + dy * w + dx;
                }
            }
        }
        let (mut sum, mut sx) = (0.0f32, [0.0f32; 3]);
        for &i in &ids {
            let a = x.get(i).copied().unwrap_or(0.0);
            let p = img.px.get(i).copied().unwrap_or([0.0; 3]);
            sum += a;
            for c in 0..3 {
                sx[c] += a * p[c];
            }
        }
        let mean_a = sum / 9.0;
        let g = [sx[0] / 9.0 - mean_a * mu[0], sx[1] / 9.0 - mean_a * mu[1], sx[2] / 9.0 - mean_a * mu[2]];
        let t = [m[0] * g[0] + m[1] * g[1] + m[2] * g[2], m[1] * g[0] + m[3] * g[1] + m[4] * g[2], m[2] * g[0] + m[4] * g[1] + m[5] * g[2]];
        for &i in &ids {
            if unknown.get(i) != Some(&true) {
                continue;
            }
            let p = img.px.get(i).copied().unwrap_or([0.0; 3]);
            let dot = (p[0] - mu[0]) * t[0] + (p[1] - mu[1]) * t[1] + (p[2] - mu[2]) * t[2];
            if let (Some(yi), Some(xi)) = (y.get_mut(i), x.get(i)) {
                *yi += xi - mean_a - dot;
            }
        }
    }
}

/// The diagonal of the Laplacian on the unknown pixels (the Jacobi preconditioner).
fn diagonal(win: &Windows, img: &RgbImage, unknown: &[bool]) -> Vec<f32> {
    let w = img.w;
    let mut diag = vec![0.0f32; img.px.len()];
    for ((tl, mu), m) in win.tl.iter().zip(&win.mu).zip(&win.inv) {
        let tl = *tl as usize;
        for dy in 0..3 {
            for dx in 0..3 {
                let i = tl + dy * w + dx;
                if unknown.get(i) != Some(&true) {
                    continue;
                }
                let p = img.px.get(i).copied().unwrap_or([0.0; 3]);
                let c = [p[0] - mu[0], p[1] - mu[1], p[2] - mu[2]];
                let q = c[0] * (m[0] * c[0] + m[1] * c[1] + m[2] * c[2])
                    + c[1] * (m[1] * c[0] + m[3] * c[1] + m[4] * c[2])
                    + c[2] * (m[2] * c[0] + m[4] * c[1] + m[5] * c[2]);
                if let Some(d) = diag.get_mut(i) {
                    *d += 1.0 - (1.0 + q) / 9.0;
                }
            }
        }
    }
    diag
}

/// What a solve is tuned with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// The matting Laplacian's regulariser ε (colours in 0–1).
    pub epsilon: f64,
    /// Weight of the colour prior (0 = closed-form matting alone).
    pub prior: f32,
    /// Distance from a colour line (the local foreground–background blend), in 0–1 colour units,
    /// at which the prior's confidence falls to 1/e.
    pub tolerance: f32,
    /// Radius (px) of the neighbourhood the sure foreground and background colours are taken from.
    pub reach: usize,
    /// A sure pixel hanging off the band whose colour differs from the sure colour nearby by more
    /// than this in some channel is freed (0 frees none).
    pub deviation: f32,
}

impl Tuning {
    /// The tuning for a band of `radius` px each side of the edge.
    pub fn for_radius(radius: f32) -> Tuning {
        Tuning { epsilon: EPSILON, prior: PRIOR, tolerance: TOLERANCE_COLOUR, reach: radius.ceil() as usize + 4, deviation: DEVIATION }
    }
}

/// Closed-form matting over a whole image: alpha for the `UNKNOWN` pixels of `kind` (one label
/// per pixel of `img`), the others held at 1 (`FOREGROUND`) and 0 (`BACKGROUND`). `init` is the
/// starting alpha of the unknown pixels (it also decides where no known pixel is reached). Mismatched
/// lengths or an image under 3 × 3 pixels leave the starting values.
pub fn solve(img: &RgbImage, kind: &[u8], init: &[f32], radius: f32) -> Solution {
    solve_with(img, kind, init, &Tuning::for_radius(radius))
}

/// The average sure-foreground and sure-background colours near each pixel; NaN where there is
/// no such pixel anywhere in the picture.
struct Means {
    fg: Vec<[f32; 3]>,
    bg: Vec<[f32; 3]>,
}

/// How much wider than `reach` the second, fallback neighbourhood of [`local_means`] is.
const WIDE: usize = 5;

/// Normalised convolution of the sure colours over a box of radius `reach`; where that holds no
/// sure pixel (a strand a long way from the foreground), over one [`WIDE`] times as wide. A box mean
/// costs the same whatever its radius.
fn local_means(img: &RgbImage, kind: &[u8], reach: usize) -> Means {
    let (w, h) = (img.w, img.h);
    let n = w * h;
    let mean = |side: u8| -> Vec<[f32; 3]> {
        let weight: Vec<f32> = kind.iter().map(|k| if *k == side { 1.0 } else { 0.0 }).collect();
        let channels: [Vec<f32>; 3] = [0, 1, 2].map(|c| img.px.iter().zip(&weight).map(|(p, w)| p[c] * w).collect());
        let at = |r: usize| -> Vec<[f32; 4]> {
            let cover = box_mean(&weight, w, h, r);
            let chan: [Vec<f32>; 3] = [0, 1, 2].map(|c| box_mean(&channels[c], w, h, r));
            (0..n).map(|i| [chan[0][i], chan[1][i], chan[2][i], cover[i]]).collect()
        };
        let near = at(reach);
        let wide = at(reach.saturating_mul(WIDE));
        near.iter()
            .zip(&wide)
            .map(|(a, b)| {
                let m = if a[3] >= 1e-3 { a } else { b };
                if m[3] < 1e-3 { [f32::NAN; 3] } else { [m[0] / m[3], m[1] / m[3], m[2] / m[3]] }
            })
            .collect()
    };
    Means { fg: mean(FOREGROUND), bg: mean(BACKGROUND) }
}

/// Frees the sure pixels that look like neither side: a strand that leaves the band is not
/// background, and pinning its pixels to 0 would cut it off at the band's border (the solution
/// ramps down along it). A sure pixel whose colour is farther than `deviation` in some channel from
/// the sure colour of its side nearby is freed when it touches an unknown pixel or one freed
/// before it, so what is freed is what hangs off the band. Returns how many it freed.
fn free_outliers(img: &RgbImage, kind: &mut [u8], m: &Means, deviation: f32) -> usize {
    let (w, h) = (img.w, img.h);
    let odd = |i: usize, k: u8| -> bool {
        let sure = if k == FOREGROUND { m.fg.get(i) } else { m.bg.get(i) };
        match (sure, img.px.get(i)) {
            (Some(c), Some(p)) => (0..3).any(|ch| (p[ch] - c[ch]).abs() > deviation),
            _ => false,
        }
    };
    let mut stack: Vec<usize> = (0..kind.len()).filter(|i| kind.get(*i) == Some(&UNKNOWN)).collect();
    let mut freed = 0;
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        for ny in y.saturating_sub(1)..(y + 2).min(h) {
            for nx in x.saturating_sub(1)..(x + 2).min(w) {
                let j = ny * w + nx;
                let Some(&k) = kind.get(j) else { continue };
                if k != UNKNOWN && odd(j, k) {
                    if let Some(slot) = kind.get_mut(j) {
                        *slot = UNKNOWN;
                    }
                    freed += 1;
                    stack.push(j);
                }
            }
        }
    }
    freed
}

/// The colour prior: each unknown pixel's alpha if it were a blend of the average sure colours
/// nearby, and how far to trust that: the weight falls with the pixel's distance from the blend's
/// colour line, and where the two colours are alike (nothing to tell them apart by).
fn colour_prior(img: &RgbImage, kind: &[u8], m: &Means, t: &Tuning) -> (Vec<f32>, Vec<f32>) {
    let n = img.px.len();
    let mut alpha = vec![0.0f32; n];
    let mut weight = vec![0.0f32; n];
    for i in 0..n {
        if kind.get(i) != Some(&UNKNOWN) {
            continue;
        }
        let (Some(f), Some(b), Some(p)) = (m.fg.get(i), m.bg.get(i), img.px.get(i)) else { continue };
        let d = [f[0] - b[0], f[1] - b[1], f[2] - b[2]];
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if !dd.is_finite() || dd < 1e-6 {
            continue;
        }
        let q = [p[0] - b[0], p[1] - b[1], p[2] - b[2]];
        let a = ((q[0] * d[0] + q[1] * d[1] + q[2] * d[2]) / dd).clamp(0.0, 1.0);
        let e2: f32 = (0..3).map(|c| (q[c] - a * d[c]).powi(2)).sum();
        // The contrast between the two colours: 1 once they are 0.25 apart.
        let contrast = (dd.sqrt() / 0.25).min(1.0);
        let conf = (-e2 / (t.tolerance * t.tolerance)).exp() * contrast;
        if conf.is_finite()
            && let (Some(al), Some(wt)) = (alpha.get_mut(i), weight.get_mut(i))
        {
            *al = a;
            *wt = t.prior * conf;
        }
    }
    (alpha, weight)
}

/// [`solve`] with explicit [`Tuning`].
pub fn solve_with(img: &RgbImage, kind_in: &[u8], init: &[f32], tuning: &Tuning) -> Solution {
    let epsilon = tuning.epsilon;
    let n = img.px.len();
    let valid = img.w.checked_mul(img.h) == Some(n) && kind_in.len() == n && init.len() == n;
    let mut kind_buf = kind_in.to_vec();
    let mut means = None;
    if valid && img.w >= 3 && img.h >= 3 && kind_in.contains(&UNKNOWN) && tuning.deviation > 0.0 {
        let m = local_means(img, kind_in, tuning.reach);
        if free_outliers(img, &mut kind_buf, &m, tuning.deviation) == 0 {
            means = Some(m);
        }
    }
    let kind: &[u8] = &kind_buf;
    let known_alpha = |i: usize| match kind.get(i) {
        Some(&FOREGROUND) => 1.0,
        Some(&UNKNOWN) if valid => init.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let mut alpha: Vec<f32> = (0..n).map(known_alpha).collect();
    let unknown: Vec<bool> = (0..n).map(|i| kind.get(i) == Some(&UNKNOWN)).collect();
    let ids: Vec<usize> = (0..n).filter(|i| unknown.get(*i) == Some(&true)).collect();
    if !valid || ids.is_empty() || img.w < 3 || img.h < 3 {
        return Solution { alpha, iterations: 0 };
    }
    let win = windows(img, kind, epsilon.max(1e-9));
    let (prior_alpha, prior_weight) = if tuning.prior > 0.0 {
        let m = means.unwrap_or_else(|| local_means(img, kind, tuning.reach));
        colour_prior(img, kind, &m, tuning)
    } else {
        (vec![0.0; n], vec![0.0; n])
    };
    // Where the colours say what a pixel is, start from that: a long thin strand then needs few
    // iterations, where a ramp across the band would be slow to bend into it.
    if tuning.prior > 0.0 {
        for &i in &ids {
            let c = (prior_weight[i] / tuning.prior).clamp(0.0, 1.0);
            alpha[i] += (prior_alpha[i] - alpha[i]) * c;
        }
    }
    let diag: Vec<f32> = diagonal(&win, img, &unknown).iter().zip(&prior_weight).map(|(d, w)| (d + ANCHOR + w).max(1e-6)).collect();
    // Right-hand side: −L_UK · α_K (plus the anchor's pull to the starting value).
    let mut known_only = alpha.clone();
    for &i in &ids {
        known_only[i] = 0.0;
    }
    let mut lk = vec![0.0f32; n];
    laplacian(&win, img, &unknown, &known_only, &mut lk);
    let mut b = vec![0.0f32; n];
    for &i in &ids {
        b[i] = -lk[i] + ANCHOR * alpha[i] + prior_weight[i] * prior_alpha[i];
    }
    // x = starting alpha on unknown pixels, zero elsewhere.
    let mut x = vec![0.0f32; n];
    for &i in &ids {
        x[i] = alpha[i];
    }
    let mut ax = vec![0.0f32; n];
    let apply = |v: &[f32], out: &mut [f32]| {
        out.fill(0.0);
        laplacian(&win, img, &unknown, v, out);
        for &i in &ids {
            out[i] += (ANCHOR + prior_weight[i]) * v[i];
        }
    };
    apply(&x, &mut ax);
    let mut r = vec![0.0f32; n];
    let mut z = vec![0.0f32; n];
    let mut p = vec![0.0f32; n];
    let mut rz = 0.0f64;
    for &i in &ids {
        r[i] = b[i] - ax[i];
        z[i] = r[i] / diag[i];
        p[i] = z[i];
        rz += f64::from(r[i]) * f64::from(z[i]);
    }
    let tol2 = TOLERANCE * TOLERANCE * ids.len() as f64;
    let mut iterations = 0;
    while iterations < MAX_ITERATIONS {
        let rr: f64 = ids.iter().map(|&i| f64::from(r[i]) * f64::from(r[i])).sum();
        if rr <= tol2 || !rr.is_finite() {
            break;
        }
        apply(&p, &mut ax);
        let pap: f64 = ids.iter().map(|&i| f64::from(p[i]) * f64::from(ax[i])).sum();
        if pap <= 0.0 || !pap.is_finite() {
            break;
        }
        let step = (rz / pap) as f32;
        let mut rz_new = 0.0f64;
        for &i in &ids {
            x[i] += step * p[i];
            r[i] -= step * ax[i];
            z[i] = r[i] / diag[i];
            rz_new += f64::from(r[i]) * f64::from(z[i]);
        }
        let beta = (rz_new / rz.max(1e-30)) as f32;
        for &i in &ids {
            p[i] = z[i] + beta * p[i];
        }
        rz = rz_new;
        iterations += 1;
    }
    for &i in &ids {
        alpha[i] = if x[i].is_finite() { x[i].clamp(0.0, 1.0) } else { init[i].clamp(0.0, 1.0) };
    }
    Solution { alpha, iterations }
}

#[cfg(test)]
mod tests;
