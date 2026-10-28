//! Mac-only tests that run real Vision. They assert properties (size, where the subject is, the
//! edge is soft), never stored Apple output. `PHOTOCRAFT_MACVISION_IMAGE` names a portrait (JPEG or
//! PNG) for the real-photo test, which skips without it; `PHOTOCRAFT_MACVISION_OUT` (a folder) makes
//! it also write the mask there as `mask.png`.

use crate::{Error, Mask, available, subject_mask};
use std::path::PathBuf;
use std::time::Instant;

fn rgba_of(img: image::DynamicImage) -> (Vec<u8>, u32, u32) {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    (rgba.into_raw(), w, h)
}

fn mean(m: &Mask, x0: u32, y0: u32, x1: u32, y1: u32) -> f32 {
    let mut sum = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            sum += u64::from(m.data[(y * m.width + x) as usize]);
        }
    }
    sum as f32 / ((x1 - x0) * (y1 - y0)) as f32 / 255.0
}

/// A smooth warm-grey backdrop with a "person" drawn inside the box `(bx, by, bw, bh)`: a head and a
/// torso with shirt folds and a face, so it has what a subject detector looks for. Deterministic.
fn figure_in(w: u32, h: u32, (bx, by, bw, bh): (u32, u32, u32, u32)) -> Vec<u8> {
    let (cx, head_cy, head_r) = (bx as f32 + bw as f32 * 0.5, by as f32 + bh as f32 * 0.30, bw as f32 * 0.11);
    let (shoulder, torso_end) = (by as f32 + bh as f32 * 0.45, by as f32 + bh as f32 * 0.95);
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let bg = 200.0 - 30.0 * (fy / h as f32);
            let in_head = (fx - cx).powi(2) + (fy - head_cy).powi(2) < head_r.powi(2);
            let half_w = bw as f32 * (0.14 + 0.16 * ((fy - shoulder) / (bh as f32 * 0.1)).clamp(0.0, 1.0));
            let in_neck = (fx - cx).abs() < bw as f32 * 0.04 && fy > head_cy && fy < shoulder;
            let in_torso = fy >= shoulder && fy < torso_end && (fx - cx).abs() < half_w;
            let px = if in_head {
                let eye = ((fx - cx).abs() - head_r * 0.4).powi(2) + (fy - head_cy + head_r * 0.15).powi(2) < (head_r * 0.1).powi(2);
                if eye { [60, 40, 30] } else { [225, 175, 140] }
            } else if in_neck {
                [210, 160, 125]
            } else if in_torso {
                let fold = 20.0 * ((fx * 0.12).sin() + (fy * 0.05).cos());
                [(40.0 + fold) as u8, (70.0 + fold) as u8, (130.0 + fold) as u8]
            } else {
                [bg as u8, (bg - 6.0) as u8, (bg - 14.0) as u8]
            };
            out.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    out
}

fn figure(w: u32, h: u32) -> Vec<u8> {
    figure_in(w, h, (0, 0, w, h))
}

#[test]
fn a_synthetic_figure_on_a_plain_background_is_cut_out() {
    if !available() {
        eprintln!("skip: no Vision subject request on this Mac");
        return;
    }
    let (w, h) = (640, 800);
    let t = Instant::now();
    let m = match subject_mask(&figure(w, h), w, h) {
        Ok(m) => m,
        Err(e) => panic!("{e}"),
    };
    eprintln!("synthetic {w}×{h}: {:.2} s", t.elapsed().as_secs_f64());
    assert_eq!((m.width, m.height, m.data.len()), (w, h, (w * h) as usize));
    let torso = mean(&m, 280, 480, 360, 640);
    let corner = mean(&m, 0, 0, 80, 80);
    assert!(torso > 0.8, "torso covered: {torso}");
    assert!(corner < 0.1, "background not covered: {corner}");
}

#[test]
fn a_flat_picture_has_no_subject() {
    if !available() {
        return;
    }
    let (w, h) = (256u32, 256u32);
    let flat: Vec<u8> = (0..w * h).flat_map(|_| [128u8, 128, 128, 255]).collect();
    match subject_mask(&flat, w, h) {
        Err(Error::NoSubject) => {}
        // Vision may also hand back an empty mask; either way nothing is selected.
        Ok(m) => assert!(mean(&m, 0, 0, w, h) < 0.05, "a flat grey picture has no subject"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn a_real_portrait_is_cut_out_whole() {
    let Some(path) = std::env::var_os("PHOTOCRAFT_MACVISION_IMAGE").map(PathBuf::from).filter(|p| p.is_file()) else {
        eprintln!("skip: set PHOTOCRAFT_MACVISION_IMAGE to a portrait (JPEG or PNG)");
        return;
    };
    if !available() {
        eprintln!("skip: no Vision subject request on this Mac");
        return;
    }
    let img = image::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let (rgba, w, h) = rgba_of(img);
    let t = Instant::now();
    let m = subject_mask(&rgba, w, h).unwrap_or_else(|e| panic!("{e}"));
    eprintln!("{}: {w}×{h} in {:.2} s", path.display(), t.elapsed().as_secs_f64());
    assert_eq!((m.width, m.height), (w, h));
    let covered = m.data.iter().filter(|v| **v > 127).count() as f32 / m.data.len() as f32;
    let soft = m.data.iter().filter(|v| (8..248).contains(*v)).count();
    eprintln!("covers {:.1} % of the picture, {soft} soft pixels", covered * 100.0);
    assert!((0.05..0.95).contains(&covered), "a portrait's subject is part of the picture: {covered}");
    assert!(soft > 100, "edges are soft");
    if let Some(dir) = std::env::var_os("PHOTOCRAFT_MACVISION_OUT").map(PathBuf::from) {
        let out = dir.join("mask.png");
        image::GrayImage::from_raw(w, h, m.data).map(|g| g.save(&out)).transpose().unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    }
}

/// Which way up the mask is: a subject only in the top 15 % of the picture comes back there and
/// nothing in the bottom 15 % (a flipped mask would swap them); the mirror image at the bottom too.
#[test]
fn a_subject_at_the_top_stays_at_the_top_and_at_the_bottom_at_the_bottom() {
    if !available() {
        eprintln!("skip: no Vision subject request on this Mac");
        return;
    }
    let (w, h, bw, bh) = (640u32, 800u32, 300u32, 120u32);
    let bx = (w - bw) / 2;
    // The torso of the figure drawn in `by..by + bh`: its middle, away from the edges.
    let torso = |m: &Mask, by: u32| mean(m, 320 - bw / 8, by + bh * 6 / 10, 320 + bw / 8, by + bh * 9 / 10);
    let top = subject_mask(&figure_in(w, h, (bx, 0, bw, bh)), w, h).unwrap_or_else(|e| panic!("{e}"));
    assert!(torso(&top, 0) > 0.9, "the figure is covered at the top: {}", torso(&top, 0));
    assert!(mean(&top, 0, h - h * 15 / 100, w, h) < 0.01, "nothing in the bottom 15 %");
    let by = h - bh;
    let bottom = subject_mask(&figure_in(w, h, (bx, by, bw, bh)), w, h).unwrap_or_else(|e| panic!("{e}"));
    assert!(torso(&bottom, by) > 0.9, "the figure is covered at the bottom: {}", torso(&bottom, by));
    assert!(mean(&bottom, 0, 0, w, h * 15 / 100) < 0.01, "nothing in the top 15 %");
}

#[test]
fn a_strip_too_thin_for_vision_is_no_subject_not_a_failure() {
    if !available() {
        return;
    }
    for (w, h) in [(3000u32, 5u32), (7, 400), (1, 1)] {
        assert_eq!(subject_mask(&vec![128; (w * h * 4) as usize], w, h), Err(Error::NoSubject), "{w}×{h}");
    }
    // A strip that is only thin once shrunk: 1 000 000 × 8 is past the pixel budget.
    assert_eq!(subject_mask(&vec![128; 1_000_000 * 8 * 4], 1_000_000, 8), Err(Error::NoSubject));
}
