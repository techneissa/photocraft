use super::*;
use crate::levels_clipping::{colour, image, scan};
use craft::imaging::ui::adjust::LevelsClip;
use photocraft_algo::histogram::clipping;
use photocraft_color::{Color, ColorMode, PixelFormat};
use photocraft_compose::Buffer;
use photocraft_compose::adjust::{Transfer, apply_depth};
use photocraft_doc::adjust::CurvePoint;
use photocraft_doc::{Document, Size};
use photocraft_geom::Rect;
use photocraft_raster::Surface;

const DEPTHS: [SampleType; 3] = [SampleType::U8, SampleType::U16, SampleType::F32];

fn pts(p: &[(f32, f32)]) -> Vec<CurvePoint> {
    p.iter().map(|(i, o)| CurvePoint { input: i / 255.0, output: o / 255.0 }).collect()
}

fn curves(master: &[(f32, f32)], channels: [&[(f32, f32)]; 3]) -> Adjustment {
    Adjustment::Curves { master: pts(master), per_channel: channels.map(pts), space: ToneSpace::Rgb, black: Vec::new(), pencil: false }
}

/// Samples to try at `depth`: every 8-bit level, a 16-bit sweep, and float values up to both ends.
fn samples(depth: SampleType) -> Vec<f32> {
    match depth {
        SampleType::U8 => (0..=255).map(|n| n as f32 / 255.0).collect(),
        SampleType::U16 => (0..=65535).step_by(13).chain([65535]).map(|n| n as f32 / 65535.0).collect(),
        SampleType::F32 => (0..=1000).map(|n| n as f32 / 1000.0).collect(),
    }
}

/// The compositor's result for a gray sample, as the document stores it.
fn result(adj: &Adjustment, depth: SampleType, v: f32) -> [f32; 3] {
    let mut b = Buffer::filled(Rect::new(0, 0, 1, 1), [v, v, v, 1.0]);
    apply_depth(adj, &mut b, Transfer::for_document(ColorMode::Rgb, depth), Some(depth));
    let q = photocraft_compose::adjustment_quantum(depth);
    std::array::from_fn(|c| {
        let r = b.px[0][c];
        q.map_or(r, |q| (r * q + 0.5).floor() / q)
    })
}

fn cases() -> Vec<Adjustment> {
    let id: &[(f32, f32)] = &[(0.0, 0.0), (255.0, 255.0)];
    vec![
        curves(id, [id, id, id]),
        // Black point slider at 30, white point slider at 220, on the composite.
        curves(&[(30.0, 0.0), (220.0, 255.0)], [id, id, id]),
        // Per channel: red's black point at 45, green's white point at 180, blue untouched with a bend.
        curves(id, [&[(45.0, 0.0), (255.0, 255.0)], &[(0.0, 0.0), (180.0, 255.0)], &[(0.0, 0.0), (128.0, 150.0), (255.0, 255.0)]]),
        // A raised black output: nothing is black.
        curves(&[(0.0, 20.0), (255.0, 255.0)], [id, id, id]),
        // Channel and composite both clip.
        curves(&[(10.0, 0.0), (240.0, 255.0)], [&[(25.0, 0.0), (255.0, 255.0)], id, &[(0.0, 0.0), (200.0, 255.0)]]),
    ]
}

/// The display marks exactly the channels Camera Raw's own clipping check finds at an end in the
/// compositor's result, for every sample, in 8, 16 and 32 bits.
#[test]
fn clipping_matches_the_curves_result_and_camera_raws_check() {
    for (n, adj) in cases().iter().enumerate() {
        for depth in DEPTHS {
            let thr = thresholds(adj, depth).expect("RGB Curves");
            for v in samples(depth) {
                let key = (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
                let [r, g, b] = result(adj, depth, v);
                let (shadows, highlights) = clipping([r, g, b, 1.0]);
                let mut black = 0u8;
                let mut white = 0u8;
                for (c, (lo, hi)) in thr.iter().enumerate() {
                    black |= u8::from(lo.is_some_and(|lo| key <= lo)) << c;
                    white |= u8::from(hi.is_some_and(|hi| key >= hi)) << c;
                }
                assert_eq!(black, shadows, "case {n} {depth:?} v={v}: shadows");
                assert_eq!(white, highlights, "case {n} {depth:?} v={v}: highlights");
            }
        }
    }
}

/// The black point slider at 30 clips levels up to and including 30, the white point slider at 220
/// from 220 on, in an 8-bit document.
#[test]
fn point_sliders_clip_up_to_and_from_their_levels() {
    let adj = cases().remove(1);
    let thr = thresholds(&adj, SampleType::U8).unwrap();
    for (lo, hi) in thr {
        for n in 0..=255u32 {
            let key = (n * 257) as u16;
            assert_eq!(lo.is_some_and(|lo| key <= lo), n <= 30, "black at {n}");
            assert_eq!(hi.is_some_and(|hi| key >= hi), n >= 220, "white at {n}");
        }
    }
}

/// An inverted curve returns to black and white at the far ends: the run from each end is empty,
/// so nothing is marked. Hostile curves don't panic, and other spaces show no display.
#[test]
fn inverted_and_hostile_curves_are_safe() {
    let inv: &[(f32, f32)] = &[(0.0, 255.0), (255.0, 0.0)];
    let id: &[(f32, f32)] = &[(0.0, 0.0), (255.0, 255.0)];
    for depth in DEPTHS {
        assert_eq!(thresholds(&curves(inv, [id, id, id]), depth), Some([(None, None); 3]));
        for bad in [
            curves(&[], [&[], &[], &[]]),
            curves(&[(0.0, f32::NAN), (f32::INFINITY, 3.0)], [id, id, id]),
            curves(&[(1000.0, -1000.0)], [id, id, id]),
            curves(&[(5.0, 5.0), (5.0, 9.0), (5.0, 1.0)], [id, id, id]),
        ] {
            let _ = thresholds(&bad, depth);
        }
    }
    let Adjustment::Curves { master, per_channel, black, .. } = cases().remove(1) else { panic!("Curves") };
    for space in [ToneSpace::Cmyk, ToneSpace::Lab] {
        let adj = Adjustment::Curves { master: master.clone(), per_channel: per_channel.clone(), space, black: black.clone(), pencil: false };
        assert_eq!(thresholds(&adj, SampleType::U8), None);
    }
    assert_eq!(thresholds(&Adjustment::Invert, SampleType::U8), None);
}

fn surface(w: u32, h: u32, px: impl Fn(u32, u32) -> Vec<f32>) -> Surface {
    let mut s = Surface::new(PixelFormat::RGBA32F);
    let data: Vec<f32> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).flat_map(|(x, y)| px(x, y)).collect();
    s.write_region(Rect::new(0, 0, w as i32, h as i32), &data);
    s
}

/// The colours of a synthetic image: the black point slider turns it white except what is
/// clipped, the white point slider turns it black except what is clipped, per channel.
#[test]
fn synthetic_image_marks_the_clipped_pixels_per_channel() {
    // Left to right: gray ramp 0..255 in a 256-wide strip.
    let s = surface(256, 1, |x, _| vec![x as f32 / 255.0, x as f32 / 255.0, x as f32 / 255.0, 1.0]);
    let stats = scan(&s, None, Rect::new(0, 0, 256, 1), 1);
    // Red's black point at 45, green's white point at 180 (case 2).
    let thr = thresholds(&cases()[2], SampleType::U8).unwrap();
    let shadows = image(&stats, &thr, LevelsClip::Shadows);
    let highlights = image(&stats, &thr, LevelsClip::Highlights);
    for x in 0..256usize {
        // Red clips to black up to 45; green and blue only at 0 (green's black point is 0, blue's too).
        let black = u8::from(x <= 45) | (u8::from(x == 0) << 1) | (u8::from(x == 0) << 2);
        assert_eq!(shadows[x], colour(LevelsClip::Shadows, black, 0), "shadows x={x}");
        // Red clips to white at 255; green from 180; blue at 255.
        let white = u8::from(x == 255) | (u8::from(x >= 180) << 1) | (u8::from(x == 255) << 2);
        assert_eq!(highlights[x], colour(LevelsClip::Highlights, 0, white), "highlights x={x}");
    }
}

/// The canvas display of an open Curves dialog is asked for by the dialog's clipping end and covers the view.
#[test]
fn the_curves_dialog_asks_for_the_display() {
    use crate::adjust_dialog::HostedAdjustDialog as Hosted;
    let mut doc = Document::with_background("t", Size::new(16, 8), ColorMode::Rgb, SampleType::U8, Color::rgba(1.0, 1.0, 1.0, 1.0));
    if let Some(s) = doc.layers[0].surface_mut() {
        let data: Vec<f32> = (0..8).flat_map(|_| 0..16).flat_map(|x| vec![x as f32 / 15.0, 0.5, 1.0, 1.0]).collect();
        s.write_region(Rect::new(0, 0, 16, 8), &data);
    }
    let target = doc.layers[0].id;
    let mut session = photocraft_engine::Session::new();
    session.add_document(doc, None);
    session.select_layer(target).unwrap();
    let mut app = crate::PhotocraftApp::new(session, crate::Services::default());
    let id = crate::adjust_dialog::open(&mut app, "image.adjustments.curves").unwrap();
    assert!(crate::levels_clipping::request(&app, 0).is_none(), "no end, no display");
    app.framework.dialogs.get_mut_by_id::<Hosted>(id).unwrap().set_clipping(Some(LevelsClip::Shadows));
    assert!(crate::levels_clipping::request(&app, 0).is_some(), "the Curves dialog shows clipping");
}
