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

/// A smooth warm-grey backdrop with a "person": a head and a torso with shirt folds and a face, so
/// it has what a subject detector looks for. Deterministic.
fn figure(w: u32, h: u32) -> Vec<u8> {
    let (cx, head_cy, head_r) = (w as f32 * 0.5, h as f32 * 0.30, w as f32 * 0.11);
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let bg = 200.0 - 30.0 * (fy / h as f32);
            let in_head = (fx - cx).powi(2) + (fy - head_cy).powi(2) < head_r.powi(2);
            let shoulder = h as f32 * 0.45;
            let half_w = w as f32 * (0.14 + 0.16 * ((fy - shoulder) / (h as f32 * 0.1)).clamp(0.0, 1.0));
            let in_neck = (fx - cx).abs() < w as f32 * 0.04 && fy > head_cy && fy < shoulder;
            let in_torso = fy >= shoulder && fy < h as f32 * 0.95 && (fx - cx).abs() < half_w;
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
fn a_tiny_picture_is_refused_not_thrown_at_vision() {
    if !available() {
        return;
    }
    assert_eq!(subject_mask(&[0; 4 * 4 * 4], 4, 4), Err(Error::BadInput));
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
