//! Tests of the pure parts and of the public API on every platform; the Vision tests are in
//! `mac_tests` (macOS only).

use crate::scale::{Layout, box_downscale_rgba, coverage, read_mask};
use crate::{Error, MAX_INPUT_PIXELS, MAX_VISION_PIXELS, Mask, available, subject_mask, vision_size};

fn mask(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> Mask {
    Mask { width: w, height: h, data: (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| f(x, y)).collect() }
}

#[test]
fn small_pictures_go_to_vision_as_they_are() {
    assert_eq!(vision_size(1440, 1800), (1440, 1800));
    assert_eq!(vision_size(2000, 2000), (2000, 2000));
    assert_eq!(vision_size(1, 1), (1, 1));
    assert_eq!(vision_size(0, 5), (0, 5));
}

#[test]
fn big_pictures_shrink_to_the_pixel_budget_keeping_their_shape() {
    for (w, h) in [(6000u32, 4000u32), (4400, 5500), (16384, 16384), (8000, 100_000), (30_000, 3)] {
        let (vw, vh) = vision_size(w, h);
        assert!(u64::from(vw) * u64::from(vh) <= MAX_VISION_PIXELS, "{w}×{h} → {vw}×{vh}");
        assert!(vw >= 1 && vh >= 1 && vw <= w && vh <= h);
    }
    let (vw, vh) = vision_size(6000, 4000);
    assert!((f64::from(vw) / f64::from(vh) - 1.5).abs() < 0.01, "{vw}×{vh}");
    assert!(u64::from(vw) * u64::from(vh) > MAX_VISION_PIXELS * 9 / 10, "uses the budget: {vw}×{vh}");
}

#[test]
fn box_downscale_averages_each_block() {
    // 4×2 → 2×1: left block {0,0,100,100}, right block {200,200,40,40} in each channel.
    let px = |v: u8| [v, v, v, 255];
    let rows: [[u8; 4]; 2] = [[0, 0, 200, 200], [100, 100, 40, 40]];
    let src: Vec<u8> = rows.iter().flat_map(|r| r.iter().flat_map(|v| px(*v))).collect();
    let out = box_downscale_rgba(&src, (4, 2), (2, 1)).unwrap();
    assert_eq!(out, vec![50, 50, 50, 255, 120, 120, 120, 255]);
}

#[test]
fn box_downscale_covers_every_source_pixel_for_uneven_sizes() {
    // 5×5 of one colour → 2×2 stays that colour (no block is empty or double counted).
    let src: Vec<u8> = (0..25).flat_map(|_| [10, 20, 30, 40]).collect();
    let out = box_downscale_rgba(&src, (5, 5), (2, 2)).unwrap();
    assert_eq!(out, (0..4).flat_map(|_| [10, 20, 30, 40]).collect::<Vec<u8>>());
}

#[test]
fn box_downscale_refuses_what_is_not_a_shrink_of_its_buffer() {
    let src = vec![0u8; 4 * 4 * 4];
    assert!(box_downscale_rgba(&src, (4, 4), (8, 8)).is_none(), "upscale");
    assert!(box_downscale_rgba(&src, (4, 4), (0, 2)).is_none(), "empty target");
    assert!(box_downscale_rgba(&src, (5, 4), (2, 2)).is_none(), "buffer too short");
    assert!(box_downscale_rgba(&src, (0, 4), (0, 2)).is_none(), "empty source");
}

#[test]
fn resizing_keeps_a_constant_mask_constant_and_the_size_exact() {
    let m = mask(7, 5, |_, _| 200);
    for (w, h) in [(14, 10), (3, 2), (100, 1), (1, 100), (7, 5)] {
        let r = m.resized(w, h).unwrap();
        assert_eq!((r.width, r.height, r.data.len()), (w, h, (w * h) as usize));
        assert!(r.data.iter().all(|v| *v == 200), "{w}×{h}");
    }
}

#[test]
fn upscaling_a_hard_edge_gives_a_soft_one_in_the_right_place() {
    // Left half 0, right half 255, scaled 8× wide: monotonic, 0 at the left end, 255 at the right,
    // and the middle crossing sits at the middle (no shift).
    let m = mask(4, 2, |x, _| if x < 2 { 0 } else { 255 });
    let r = m.resized(32, 16).unwrap();
    let row = &r.data[..32];
    assert_eq!(row[0], 0);
    assert_eq!(row[31], 255);
    assert!(row.windows(2).all(|p| p[0] <= p[1]), "monotonic: {row:?}");
    assert!(row[15] < 128 && row[16] > 127, "centred: {} {}", row[15], row[16]);
    assert!(row.iter().any(|v| (1..255).contains(v)), "soft");
}

#[test]
fn resizing_the_same_size_copies_and_bad_masks_are_refused() {
    let m = mask(3, 3, |x, y| (x * 40 + y * 10) as u8);
    assert_eq!(m.resized(3, 3).unwrap(), m);
    assert!(m.resized(0, 3).is_none());
    assert!(m.resized(3, 0).is_none());
    assert!(Mask { width: 3, height: 3, data: vec![0; 8] }.resized(6, 6).is_none(), "data shorter than size");
    assert!(Mask { width: 0, height: 3, data: vec![] }.resized(6, 6).is_none());
}

#[test]
fn coverage_clamps_and_rounds() {
    assert_eq!(coverage(0.0), 0);
    assert_eq!(coverage(1.0), 255);
    assert_eq!(coverage(0.5), 128);
    assert_eq!(coverage(-3.0), 0);
    assert_eq!(coverage(7.0), 255);
    assert_eq!(coverage(f32::NAN), 0);
    assert_eq!(coverage(f32::INFINITY), 255);
    assert_eq!(coverage(f32::NEG_INFINITY), 0);
}

#[test]
fn float_buffers_are_read_row_by_row_skipping_row_padding() {
    // 3×2 floats, rows 16 bytes apart (12 used + 4 padding that must not be read).
    let mut bytes = Vec::new();
    for row in [[0.0f32, 0.5, 1.0], [1.0, 0.25, 0.0]] {
        for v in row {
            bytes.extend_from_slice(&v.to_ne_bytes());
        }
        bytes.extend_from_slice(&f32::NAN.to_ne_bytes());
    }
    // The last row's padding is optional: a buffer may end right after the last pixel.
    bytes.truncate(bytes.len() - 4);
    let m = read_mask(&bytes, 16, 3, 2, Layout::Float32).unwrap();
    assert_eq!((m.width, m.height), (3, 2));
    assert_eq!(m.data, vec![0, 128, 255, 255, 64, 0]);
}

#[test]
fn byte_buffers_are_read_and_short_ones_refused() {
    let bytes = [1u8, 2, 3, 99, 4, 5, 6];
    assert_eq!(read_mask(&bytes, 4, 3, 2, Layout::Byte).unwrap().data, vec![1, 2, 3, 4, 5, 6]);
    assert!(read_mask(&bytes[..6], 4, 3, 2, Layout::Byte).is_none(), "one pixel short");
    assert!(read_mask(&bytes, 2, 3, 2, Layout::Byte).is_none(), "rows narrower than the picture");
    assert!(read_mask(&bytes, 4, 0, 2, Layout::Byte).is_none());
    assert!(read_mask(&bytes, usize::MAX, 3, 2, Layout::Byte).is_none(), "overflow");
}

#[test]
fn wrong_input_is_refused_before_anything_else() {
    assert_eq!(subject_mask(&[], 0, 0), Err(Error::BadInput));
    assert_eq!(subject_mask(&[0; 16], 2, 3), Err(Error::BadInput), "length does not match");
    assert_eq!(subject_mask(&[0; 16], 0, 4), Err(Error::BadInput));
    let huge = u32::try_from(MAX_INPUT_PIXELS).unwrap_or(u32::MAX);
    assert_eq!(subject_mask(&[0; 4], huge, 2), Err(Error::BadInput), "past the pixel cap, whatever the buffer");
}

#[test]
fn a_system_without_vision_says_so() {
    if available() {
        return;
    }
    assert_eq!(subject_mask(&vec![0; 64 * 64 * 4], 64, 64), Err(Error::Unavailable));
}

#[test]
fn every_error_has_a_message() {
    for e in [Error::Unavailable, Error::NoSubject, Error::BadInput, Error::Os("boom".into())] {
        assert!(!e.to_string().is_empty());
    }
    assert!(Error::Os("boom".into()).to_string().contains("boom"));
}
