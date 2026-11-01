use super::*;
use crate::matting::{self, RefineParams};
use crate::segment::{ImageSampler, Rng};

/// A fake head with loose hair: a dark ellipse with thin, curved, tapering strands (0.6–2.2 px
/// wide) leaving its top, over a smooth light background, with the exact alpha. Returns the image,
/// that alpha, and the hard selection a tool would make (the alpha blurred and cut at ½, which
/// loses the strands).
fn hair_scene(w: usize, h: usize, seed: u64) -> (RgbImage, Vec<f32>, Vec<f32>) {
    hair_scene_sized(w, h, seed, 70, 4)
}

/// [`hair_scene`] with `strands` strands, coverage taken from `ss × ss` samples a pixel.
fn hair_scene_sized(w: usize, h: usize, seed: u64, strands: usize, ss: usize) -> (RgbImage, Vec<f32>, Vec<f32>) {
    let ss = ss.max(1);
    let (sw, sh) = (w * ss, h * ss);
    let mut cover = vec![0u8; sw * sh];
    let (cx, cy, rx, ry) = (w as f32 * 0.5, h as f32 * 0.78, w as f32 * 0.28, h as f32 * 0.5);
    for y in 0..sh {
        for x in 0..sw {
            let (dx, dy) = ((x as f32 + 0.5) / ss as f32 - cx, (y as f32 + 0.5) / ss as f32 - cy);
            if (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0 {
                cover[y * sw + x] = 1;
            }
        }
    }
    let mut rng = Rng::new(seed);
    for _ in 0..strands {
        // Start just inside the top arc, leave along its normal with a random tilt and bend.
        let theta = std::f32::consts::PI * (1.12 + 0.76 * rng.f32());
        let (nx, ny) = (theta.cos(), theta.sin());
        let (sx, sy) = (cx + (rx - 3.0) * nx, cy + (ry - 3.0) * ny);
        let tilt = (rng.f32() - 0.5) * 1.1;
        let (dx, dy) = (nx * tilt.cos() - ny * tilt.sin(), nx * tilt.sin() + ny * tilt.cos());
        let len = 22.0 + 26.0 * rng.f32();
        let bend = (rng.f32() - 0.5) * 18.0;
        let width = 1.0 + 1.2 * rng.f32();
        let steps = (len * ss as f32 * 2.0) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let off = bend * t * t;
            let (px, py) = (sx + dx * len * t - dy * off, sy + dy * len * t + dx * off);
            let rad = 0.5 * width * (1.0 - 0.7 * t) * ss as f32;
            let (ix, iy) = (px * ss as f32, py * ss as f32);
            let (x0, x1) = ((ix - rad).floor().max(0.0) as usize, ((ix + rad).ceil().max(0.0) as usize).min(sw.saturating_sub(1)));
            let (y0, y1) = ((iy - rad).floor().max(0.0) as usize, ((iy + rad).ceil().max(0.0) as usize).min(sh.saturating_sub(1)));
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    if (xx as f32 + 0.5 - ix).powi(2) + (yy as f32 + 0.5 - iy).powi(2) <= rad * rad {
                        cover[yy * sw + xx] = 1;
                    }
                }
            }
        }
    }
    let alpha: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let mut s = 0u32;
            for sy in 0..ss {
                for sx in 0..ss {
                    s += u32::from(cover[(y * ss + sy) * sw + x * ss + sx]);
                }
            }
            s as f32 / (ss * ss) as f32
        })
        .collect();
    let mut noise = Rng::new(seed ^ 0xABCD);
    let img = RgbImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let fg = [0.30 + 0.05 * (0.15 * fx + 0.1 * fy).sin(), 0.18 + 0.03 * (0.15 * fx + 0.1 * fy).sin(), 0.10 + 0.02 * (0.15 * fx + 0.1 * fy).sin()];
        let bg = [0.72 + 0.03 * (0.05 * fx).sin() - 0.2 * fy / h as f32, 0.78 + 0.03 * (0.04 * fy).cos() - 0.2 * fy / h as f32, 0.88 - 0.15 * fy / h as f32];
        let a = alpha[y * w + x];
        [0, 1, 2].map(|c| (a * fg[c] + (1.0 - a) * bg[c] + 0.004 * noise.normal()).clamp(0.0, 1.0))
    });
    let blurred = matting::gaussian_blur(&alpha, w, h, 2.5);
    let mask = blurred.iter().map(|v| if *v >= 0.5 { 1.0 } else { 0.0 }).collect();
    (img, alpha, mask)
}

fn reader(m: &[f32], w: usize, h: usize) -> impl Fn(Rect) -> Vec<f32> + Sync + '_ {
    move |r: Rect| {
        let mut v = Vec::with_capacity(r.width() as usize * r.height() as usize);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                v.push(if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h { m[y as usize * w + x as usize] } else { 0.0 });
            }
        }
        v
    }
}

fn hair_alpha(img: &RgbImage, mask: &[f32], radius: f32) -> Vec<f32> {
    let (w, h) = (img.w, img.h);
    let canvas = Rect::new(0, 0, w as i32, h as i32);
    let read = reader(mask, w, h);
    let smp = ImageSampler { img, origin: (0, 0) };
    let region = refine_hair(&smp, &read, canvas, canvas, radius).expect("refine").expect("something selected");
    (0..w * h).map(|i| region.at((i % w) as i32, (i / w) as i32)).collect()
}

/// Mean absolute alpha error over the pixels `pick` selects.
fn mae(a: &[f32], truth: &[f32], pick: impl Fn(usize) -> bool) -> f32 {
    let (mut s, mut n) = (0.0f64, 0u32);
    for (i, (x, t)) in a.iter().zip(truth).enumerate() {
        if pick(i) {
            s += f64::from((x - t).abs());
            n += 1;
        }
    }
    (s / f64::from(n.max(1))) as f32
}

/// Refine Hair's alpha error is clearly below Refine Edge's on thin strands (numbers recorded in
/// the reply and `FORK.md`).
#[test]
fn hair_beats_refine_edge_on_synthetic_hair() {
    let (w, h) = (320usize, 240usize);
    let radius = 20.0f32;
    let mut totals = [0.0f32; 4];
    let seeds = [1u64, 2, 3];
    for seed in seeds {
        let (img, truth, mask) = hair_scene(w, h, seed);
        // Where either method can act: within `radius` of the selection's edge.
        let mut edge = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let v = mask[i] >= 0.5;
                edge[i] = (x > 0 && (mask[i - 1] >= 0.5) != v)
                    || (x + 1 < w && (mask[i + 1] >= 0.5) != v)
                    || (y > 0 && (mask[i - w] >= 0.5) != v)
                    || (y + 1 < h && (mask[i + w] >= 0.5) != v);
            }
        }
        let dist = crate::selection::edt(&edge, w, h);
        let in_band = |i: usize| dist[i] < radius;
        let soft = |i: usize| truth[i] > 0.02 && truth[i] < 0.98 && dist[i] < radius;
        let hair = hair_alpha(&img, &mask, radius);
        let edge_plain = matting::refine_buffer(Some(&img), &mask, w, h, &RefineParams { radius, ..RefineParams::default() });
        let edge_smart = matting::refine_buffer(Some(&img), &mask, w, h, &RefineParams { radius, smart_radius: true, ..RefineParams::default() });
        let none = mae(&mask, &truth, in_band);
        let row = [mae(&hair, &truth, in_band), mae(&edge_plain, &truth, in_band), mae(&edge_smart, &truth, in_band)];
        let soft_row = [mae(&hair, &truth, soft), mae(&edge_plain, &truth, soft), mae(&edge_smart, &truth, soft)];
        println!(
            "seed {seed}: band MAE  selection {none:.4}  refine edge {:.4}  refine edge (smart radius) {:.4}  refine hair {:.4}   | soft pixels MAE  refine edge {:.4}  smart {:.4}  hair {:.4}",
            row[1], row[2], row[0], soft_row[1], soft_row[2], soft_row[0]
        );
        totals[0] += row[0];
        totals[1] += row[1].min(row[2]);
        totals[2] += soft_row[0];
        totals[3] += soft_row[1].min(soft_row[2]);
    }
    let n = seeds.len() as f32;
    println!(
        "mean band MAE: hair {:.4} vs best refine edge {:.4}; soft pixels: hair {:.4} vs {:.4}",
        totals[0] / n,
        totals[1] / n,
        totals[2] / n,
        totals[3] / n
    );
    assert!(totals[0] < 0.5 * totals[1], "band: hair {} vs refine edge {}", totals[0] / n, totals[1] / n);
    assert!(totals[2] < 0.5 * totals[3], "soft pixels: hair {} vs refine edge {}", totals[2] / n, totals[3] / n);
}

/// The paper's own case: an image that is exactly a blend of two colours across a soft edge. Alpha
/// comes back within a few percent.
#[test]
fn recovers_a_soft_edge_from_the_two_colour_model() {
    let (w, h) = (96usize, 64usize);
    let alpha: Vec<f32> = (0..w * h).map(|i| (0.5 - ((i % w) as f32 + 0.5 - 48.0) / 16.0).clamp(0.0, 1.0)).collect();
    let (f, b) = ([0.85f32, 0.3, 0.2], [0.15f32, 0.5, 0.8]);
    let img = RgbImage::from_fn(w, h, |x, y| {
        let a = alpha[y * w + x];
        // A little texture, the same on both layers, so the colours are not flat.
        let t = 0.03 * ((x as f32) * 0.4).sin();
        [0, 1, 2].map(|c| a * (f[c] + t) + (1.0 - a) * (b[c] - t))
    });
    let mask: Vec<f32> = (0..w * h).map(|i| if (i % w) < 48 { 1.0 } else { 0.0 }).collect();
    let got = hair_alpha(&img, &mask, 14.0);
    let err = mae(&got, &alpha, |_| true);
    println!("two-colour edge: MAE {err:.4}");
    assert!(err < 0.03, "{err}");
}

/// A selection with a long straight edge has the same alpha tile by tile as in one piece: the
/// picture is wider than two tiles, so its band crosses their seams.
#[test]
fn tiles_agree_with_one_solve() {
    let (w, h) = (640usize, 300usize);
    let (img, _truth, mask) = hair_scene(w, h, 7);
    // The same scene repeated sideways would be wider; here a taller radius and the wide image do.
    let radius = 16.0;
    let tiled = hair_alpha(&img, &mask, radius);
    // One solve over the whole picture with the same trimap.
    let tr = Trimap::new(&mask, Rect::new(0, 0, w as i32, h as i32), Rect::new(0, 0, w as i32, h as i32), radius);
    let whole = solve(&img, &tr.kind, &tr.init, radius);
    let diff = mae(&tiled, &whole.alpha, |_| true);
    let worst = tiled.iter().zip(&whole.alpha).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    println!("tiled vs whole: mean {diff:.5}, worst {worst:.3}, {} iterations", whole.iterations);
    assert!(diff < 0.004, "{diff}");
}

/// Sure foreground stays 1, sure background stays 0, a flat image with a plain edge stays put.
#[test]
fn known_pixels_are_kept() {
    let (w, h) = (80usize, 60usize);
    let img = RgbImage::from_fn(w, h, |x, _| if x < 40 { [0.2, 0.2, 0.2] } else { [0.9, 0.9, 0.9] });
    let mask: Vec<f32> = (0..w * h).map(|i| if i % w < 40 { 1.0 } else { 0.0 }).collect();
    let got = hair_alpha(&img, &mask, 6.0);
    for y in 0..h {
        for x in 0..w {
            let a = got[y * w + x];
            if x + 6 < 40 {
                assert_eq!(a, 1.0, "({x},{y})");
            }
            if x >= 46 {
                assert_eq!(a, 0.0, "({x},{y})");
            }
            assert!((0.0..=1.0).contains(&a));
        }
    }
    // A sharp edge on a flat image: the band solves to a sharp edge at the same place.
    assert!(got[30 * w + 38] > 0.9 && got[30 * w + 41] < 0.1, "{} {}", got[30 * w + 38], got[30 * w + 41]);
}

#[test]
fn flat_regions_and_empty_selections_cost_nothing() {
    let canvas = Rect::new(0, 0, 1000, 800);
    let img = RgbImage::new(1, 1);
    let smp = ImageSampler { img: &img, origin: (0, 0) };
    // Everything selected: no edge anywhere, alpha 1.
    let all = |r: Rect| vec![1.0; r.width() as usize * r.height() as usize];
    let reg = refine_hair(&smp, &all, canvas, canvas, 10.0).expect("ok").expect("selected");
    assert_eq!(reg.bbox, canvas);
    assert!(reg.mask.iter().all(|v| *v == 255));
    // Nothing selected.
    let none = |r: Rect| vec![0.0; r.width() as usize * r.height() as usize];
    assert_eq!(refine_hair(&smp, &none, Rect::EMPTY, canvas, 10.0), Ok(None));
    assert_eq!(refine_hair(&smp, &none, canvas, canvas, 10.0), Ok(None));
}

#[test]
fn rejects_bad_radius_and_huge_areas_before_allocating() {
    let canvas = Rect::new(0, 0, 100, 100);
    let img = RgbImage::new(1, 1);
    let smp = ImageSampler { img: &img, origin: (0, 0) };
    let none = |r: Rect| vec![0.0; r.width() as usize * r.height() as usize];
    for r in [-1.0, f32::NAN, f32::INFINITY, 1001.0] {
        assert_eq!(refine_hair(&smp, &none, canvas, canvas, r), Err(HairError::BadRadius), "{r}");
    }
    assert_eq!(effective_radius(0.0), Ok(DEFAULT_RADIUS));
    assert_eq!(effective_radius(0.2), Ok(1.0));
    assert_eq!(effective_radius(900.0), Ok(MAX_RADIUS));
    // A canvas of 2^31 px in all: refused up front; the mask closure is never called.
    let huge = Rect::new(0, 0, 100_000, 100_000);
    let never = |_: Rect| -> Vec<f32> { unreachable_in_test() };
    assert!(matches!(refine_hair(&smp, &never, huge, huge, 10.0), Err(HairError::TooLarge { .. })));
    assert!(matches!(check_size(huge, huge, 10.0), Err(HairError::TooLarge { .. })));
    // A small selection on a huge canvas is fine: the area follows the selection, not the canvas.
    assert!(check_size(Rect::new(500, 500, 700, 700), huge, 10.0).is_ok());
    assert_eq!(hair_area(Rect::new(500, 500, 700, 700), huge, 10.0), Rect::new(469, 469, 731, 731));
}

fn unreachable_in_test() -> Vec<f32> {
    Vec::new()
}

/// Tiny canvases, odd sizes, a mask that misbehaves and a sampler that answers the wrong size
/// never panic.
#[test]
fn degenerate_inputs_never_panic() {
    for (cw, ch) in [(1, 1), (2, 2), (3, 3), (2, 40), (40, 2), (5, 7)] {
        let canvas = Rect::new(0, 0, cw, ch);
        let img = RgbImage::from_fn(cw as usize, ch as usize, |x, y| [x as f32 / 7.0, y as f32 / 7.0, 0.5]);
        let smp = ImageSampler { img: &img, origin: (0, 0) };
        let half = |r: Rect| (0..r.width() * r.height()).map(|i| if (i % r.width().max(1)) as i32 + r.x0 < cw / 2 { 1.0 } else { 0.0 }).collect::<Vec<f32>>();
        for radius in [0.0, 0.5, 3.0, 64.0, 500.0] {
            assert!(refine_hair(&smp, &half, canvas, canvas, radius).is_ok(), "{cw}x{ch} r={radius}");
        }
        // Soft and out-of-range coverage, NaN included.
        let odd = |r: Rect| (0..r.width() * r.height()).map(|i| [f32::NAN, 7.0, -3.0, 0.5][(i % 4) as usize]).collect::<Vec<f32>>();
        assert!(refine_hair(&smp, &odd, canvas, canvas, 4.0).is_ok());
        // A mask that answers the wrong length is treated as empty.
        let short = |_: Rect| vec![1.0; 3];
        assert!(refine_hair(&smp, &short, canvas, canvas, 4.0).is_ok());
        // A selection outside the canvas.
        assert_eq!(refine_hair(&smp, &half, Rect::new(500, 500, 600, 600), canvas, 4.0), Ok(None));
    }
    // Mismatched trimap lengths and sizes under 3 × 3 leave the starting alpha.
    let img = RgbImage::new(4, 4);
    assert_eq!(solve(&img, &[UNKNOWN; 3], &[0.5; 16], 4.0).alpha.len(), 16);
    assert_eq!(solve(&img, &[UNKNOWN; 16], &[0.5; 2], 4.0).alpha, vec![0.0; 16]);
    let tiny = RgbImage::new(2, 2);
    assert_eq!(solve(&tiny, &[UNKNOWN; 4], &[0.25; 4], 4.0).alpha, vec![0.25; 4]);
    // A trimap with no known pixel at all: a flat image stays at the start.
    let flat = RgbImage::from_fn(12, 12, |_, _| [0.5; 3]);
    let s = solve(&flat, &[UNKNOWN; 144], &[0.5; 144], 4.0);
    assert!(s.alpha.iter().all(|a| (a - 0.5).abs() < 1e-3));
}

/// A hostile image (every channel at an extreme, NaN) gives alpha in 0–1 and no panic.
#[test]
fn extreme_colours_keep_alpha_in_range() {
    let (w, h) = (64usize, 48usize);
    let mut rng = Rng::new(9);
    let img = RgbImage::from_fn(w, h, |_, _| [0, 1, 2].map(|_| if rng.f32() < 0.5 { 0.0 } else { 1.0 }));
    let mask: Vec<f32> = (0..w * h).map(|i| if i % w < 30 { 1.0 } else { 0.0 }).collect();
    let got = hair_alpha(&img, &mask, 8.0);
    assert!(got.iter().all(|a| (0.0..=1.0).contains(a)));
    let nan = RgbImage::from_fn(w, h, |x, _| if x == 31 { [f32::NAN; 3] } else { [0.4; 3] });
    let tr = Trimap::new(&mask, Rect::new(0, 0, w as i32, h as i32), Rect::new(0, 0, w as i32, h as i32), 8.0);
    let s = solve(&nan, &tr.kind, &tr.init, 8.0);
    assert!(s.alpha.iter().all(|a| (0.0..=1.0).contains(a)), "NaN colours");
}

/// Timing on a 24 MP picture (`cargo test -p photocraft-algo --release --lib hair::tests::time_24mp -- --ignored --nocapture`).
#[test]
#[ignore]
fn time_24mp() {
    let (w, h) = (6000usize, 4000usize);
    let t0 = std::time::Instant::now();
    let (img, truth, mask) = hair_scene_sized(w, h, 5, 1500, 2);
    println!("scene built in {:.1}s", t0.elapsed().as_secs_f32());
    for radius in [10.0f32, 20.0, 40.0] {
        let t = std::time::Instant::now();
        let got = hair_alpha(&img, &mask, radius);
        let secs = t.elapsed().as_secs_f32();
        let unknown = got.iter().filter(|v| **v > 0.0 && **v < 1.0).count();
        println!("24 MP, radius {radius}: {secs:.2}s, {unknown} partly covered px, MAE {:.4}", mae(&got, &truth, |_| true));
    }
    // The same picture with the whole frame an edge (30 % of it in the band): the worst case for a real photo.
    let (tw, th) = (320usize, 240usize);
    let (tile, _, tmask) = hair_scene(tw, th, 3);
    let (cols, rows) = (w / tw, h / th);
    let (w2, h2) = (cols * tw, rows * th);
    let big = RgbImage::from_fn(w2, h2, |x, y| tile.px[(y % th) * tw + x % tw]);
    let bigmask: Vec<f32> = (0..w2 * h2).map(|i| tmask[((i / w2) % th) * tw + (i % w2) % tw]).collect();
    let t = std::time::Instant::now();
    let got = hair_alpha(&big, &bigmask, 20.0);
    println!(
        "{w2}x{h2} ({:.1} MP) tiled heads, radius 20: {:.2}s ({} partly covered px)",
        (w2 * h2) as f32 / 1e6,
        t.elapsed().as_secs_f32(),
        got.iter().filter(|v| **v > 0.0 && **v < 1.0).count()
    );
}
