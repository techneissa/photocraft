use super::*;
use crate::levels_clipping::{ClipEnd, FIELD, colour, image, paint, request, scan};
use egui_kittest::Harness;
use photocraft_algo::histogram::clipping;
use photocraft_color::{Color, ColorMode, PixelFormat};
use photocraft_compose::Buffer;
use photocraft_compose::adjust::{Transfer, apply_depth};
use photocraft_doc::adjust::CurvePoint;
use photocraft_doc::{Document, Size};
use photocraft_geom::Rect;
use photocraft_raster::Surface;
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::canvas::ViewXform;
use egui::Color32;

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
    let shadows = image(&stats, &thr, ClipEnd::Shadows);
    let highlights = image(&stats, &thr, ClipEnd::Highlights);
    for x in 0..256usize {
        // Red clips to black up to 45; green and blue only at 0 (green's black point is 0, blue's too).
        let black = u8::from(x <= 45) | (u8::from(x == 0) << 1) | (u8::from(x == 0) << 2);
        assert_eq!(shadows[x], colour(ClipEnd::Shadows, black, 0), "shadows x={x}");
        // Red clips to white at 255; green from 180; blue at 255.
        let white = u8::from(x == 255) | (u8::from(x >= 180) << 1) | (u8::from(x == 255) << 2);
        assert_eq!(highlights[x], colour(ClipEnd::Highlights, 0, white), "highlights x={x}");
    }
}

/// A 16×8 document: the background's red channel ramps 0..1 left to right, green is 0.5, blue 1.
fn ramp_app(mode: ColorMode, depth: SampleType) -> PhotocraftApp {
    let mut doc = Document::with_background("t", Size::new(16, 8), mode, depth, Color::rgba(1.0, 1.0, 1.0, 1.0));
    let gray = mode == ColorMode::Grayscale;
    if let Some(s) = doc.layers[0].surface_mut() {
        let data: Vec<f32> =
            (0..8).flat_map(|_| 0..16).flat_map(|x| if gray { vec![x as f32 / 15.0, 1.0] } else { vec![x as f32 / 15.0, 0.5, 1.0, 1.0] }).collect();
        s.write_region(Rect::new(0, 0, 16, 8), &data);
    }
    let target = doc.layers[0].id;
    let mut session = photocraft_engine::Session::new();
    session.add_document(doc, None);
    session.select_layer(target).unwrap();
    PhotocraftApp::new(session, crate::Services::default())
}

fn view(zoom: f32) -> ViewXform {
    ViewXform {
        rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(16.0 * zoom, 8.0 * zoom)),
        zoom,
        center: [8.0, 4.0],
        flip: false,
        rotation: 0.0,
        aspect: 1.0,
    }
}

/// The canvas display of an open Curves dialog covers the view in every depth and in Gray.
#[test]
fn the_canvas_shows_the_curves_clipping() {
    let ctx = egui::Context::default();
    for (mode, depth) in
        [(ColorMode::Rgb, SampleType::U8), (ColorMode::Rgb, SampleType::U16), (ColorMode::Rgb, SampleType::F32), (ColorMode::Grayscale, SampleType::U8)]
    {
        let mut app = ramp_app(mode, depth);
        let id = crate::adjust_dialog::open(&mut app, "image.adjustments.curves").unwrap();
        // Black point slider moved to level 100 on the composite.
        let fields = &mut app.ui.dialog_mut(id).unwrap().fields;
        fields.insert("points".into(), json!([[100, 0], [255, 255]]));
        fields.insert(FIELD.into(), json!("shadows"));
        assert!(request(&app, 0).is_some(), "{mode:?} {depth:?}");
        let painter = egui::Painter::new(ctx.clone(), egui::LayerId::background(), egui::Rect::EVERYTHING);
        paint(&mut app, &ctx, &painter, &view(10.0), 0);
        let c = app.levels_clipping.as_ref().unwrap();
        let (end, thr) = c.drawn.unwrap();
        let px = image(&c.stats, &thr, end);
        let gray = mode == ColorMode::Grayscale;
        let clipped = if gray { Color32::BLACK } else { Color32::from_rgb(0, 255, 255) };
        for (x, p) in px.iter().take(16).enumerate() {
            let want = if x as f32 / 15.0 * 255.0 <= 100.0 { clipped } else { Color32::WHITE };
            assert_eq!(*p, want, "{mode:?} {depth:?} x={x}");
        }
    }
}

fn dialog_harness(app: PhotocraftApp) -> Harness<'static, PhotocraftApp> {
    let h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(|ui, app| crate::dialogs::show(app, ui.ctx()), app);
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
    h
}

fn field(h: &Harness<'static, PhotocraftApp>) -> Option<Value> {
    h.state().ui.dialogs.first().and_then(|d| d.fields.get(FIELD).cloned())
}

fn press(h: &mut Harness<'static, PhotocraftApp>, at: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) {
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers });
    h.run_steps(1);
}

fn modifiers(h: &mut Harness<'static, PhotocraftApp>, m: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(m));
    h.run_steps(1);
}

fn mem() -> egui::Id {
    egui::Id::unique(("adjust-dialog", "curves"))
}

/// Alt-dragging the black point slider shows the shadows display and the white point slider the
/// highlights, while both Alt and the button are held; a drag without Alt shows nothing, with
/// Show Clipping on it shows the display without Alt, and the view ends on release. The sliders
/// move the first and last point; the document stays untouched.
#[test]
fn the_point_sliders_turn_the_display_on() {
    let mut h = dialog_harness(ramp_app(ColorMode::Rgb, SampleType::U8));
    let original = h.state().session.active().unwrap().doc.clone();
    crate::adjust_dialog::open(h.state_mut(), "image.adjustments.curves").unwrap();
    h.run_steps(3);
    let graph = h.ctx.data(|d| d.get_temp::<egui::Rect>(mem().with("curves-graph"))).unwrap();
    let strip = crate::adjust_editors::curve_strip(graph);
    let at = |level: f32| egui::pos2(graph.left() + level / 255.0 * graph.width(), strip.center().y);
    let alt = egui::Modifiers::ALT;
    // Black point slider.
    modifiers(&mut h, alt);
    h.event(egui::Event::PointerMoved(at(0.0)));
    press(&mut h, at(0.0), true, alt);
    assert_eq!(field(&h), Some(json!("shadows")));
    h.event(egui::Event::PointerMoved(at(60.0)));
    h.run_steps(2);
    let points = h.state().ui.dialogs[0].fields["points"].clone();
    let first = points[0][0].as_f64().unwrap();
    assert!((55.0..=65.0).contains(&first), "the drag moves the black point: {points}");
    assert_eq!(points[0][1].as_f64(), Some(0.0), "its output stays");
    assert_eq!(field(&h), Some(json!("shadows")));
    modifiers(&mut h, egui::Modifiers::NONE);
    assert_eq!(field(&h), None, "releasing Alt shows the preview");
    modifiers(&mut h, alt);
    assert_eq!(field(&h), Some(json!("shadows")), "Alt again");
    press(&mut h, at(60.0), false, alt);
    assert_eq!(field(&h), None, "releasing the mouse shows the preview");
    // White point slider.
    h.event(egui::Event::PointerMoved(at(255.0)));
    press(&mut h, at(255.0), true, alt);
    assert_eq!(field(&h), Some(json!("highlights")));
    press(&mut h, at(255.0), false, alt);
    assert_eq!(field(&h), None);
    // No Alt, no Show Clipping: the sliders move but show nothing.
    modifiers(&mut h, egui::Modifiers::NONE);
    h.event(egui::Event::PointerMoved(at(255.0)));
    press(&mut h, at(255.0), true, egui::Modifiers::NONE);
    h.event(egui::Event::PointerMoved(at(200.0)));
    h.run_steps(2);
    assert_eq!(field(&h), None, "no Alt, no display");
    press(&mut h, at(200.0), false, egui::Modifiers::NONE);
    // Show Clipping on: the display shows without Alt, for the slider that is held.
    h.state_mut().ui.dialogs[0].fields.insert("__showClipping".into(), json!(true));
    h.run_steps(2);
    assert!(crate::adjust_editors::show_clipping(&h.ctx, mem()), "the field sets the checkbox");
    h.event(egui::Event::PointerMoved(at(200.0)));
    press(&mut h, at(200.0), true, egui::Modifiers::NONE);
    assert_eq!(field(&h), Some(json!("highlights")), "Show Clipping");
    press(&mut h, at(200.0), false, egui::Modifiers::NONE);
    assert_eq!(field(&h), None, "ends on release");
    assert!(std::sync::Arc::ptr_eq(&h.state().session.active().unwrap().doc, &original));
}

/// Scan once per view, then per slider move the thresholds of the curves + colours (the texture
/// upload comes on top), on 24 and 36 MP layers. `cargo test -p photocraft-ui-egui --lib
/// curves_clipping_bench -- --ignored --nocapture`
#[test]
#[ignore = "benchmark"]
fn curves_clipping_bench() {
    let adj =
        curves(&[(30.0, 0.0), (220.0, 255.0)], [&[(0.0, 0.0), (128.0, 140.0), (255.0, 255.0)], &[(0.0, 0.0), (255.0, 255.0)], &[(5.0, 0.0), (250.0, 255.0)]]);
    for (w, h) in [(6000u32, 4000u32), (7360, 4912)] {
        let mut s = Surface::new(PixelFormat::RGBA8);
        for y0 in (0..h).step_by(256) {
            let rows = 256.min(h - y0);
            let data: Vec<f32> = (0..rows * w)
                .flat_map(|i| {
                    let (x, y) = (i % w, y0 + i / w);
                    let n = ((x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) >> 7) % 64;
                    let v = (x as f32 / w as f32 * 0.8 + n as f32 / 255.0).min(1.0);
                    [v, 1.0 - v, (y as f32 / h as f32), 1.0]
                })
                .collect();
            s.write_region(Rect::new(0, y0 as i32, w as i32, (y0 + rows) as i32), &data);
        }
        let fit = w.div_ceil(2400);
        let best = |f: &mut dyn FnMut()| {
            (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    f();
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .fold(f64::MAX, f64::min)
        };
        let rect = Rect::new(0, 0, w as i32, h as i32);
        let scan_ms = best(&mut || {
            std::hint::black_box(scan(&s, None, rect, fit));
        });
        let stats = scan(&s, None, rect, fit);
        let thr_ms = best(&mut || {
            std::hint::black_box(thresholds(&adj, SampleType::U8));
        });
        let update_ms = best(&mut || {
            let thr = thresholds(&adj, SampleType::U8).unwrap();
            std::hint::black_box(egui::ColorImage::new(stats.size, image(&stats, &thr, ClipEnd::Shadows)));
        });
        println!(
            "{} MP fit (1/{fit}, {}×{} texels): scan {scan_ms:.1} ms, thresholds {thr_ms:.2} ms, slider move (thresholds + colours) {update_ms:.1} ms",
            (w * h) / 1_000_000,
            stats.size[0],
            stats.size[1]
        );
    }
}

/// With one channel chosen, the sliders move that channel's curve and the display marks that
/// channel's colour only.
#[test]
fn a_single_channel_moves_and_marks_only_itself() {
    let mut h = dialog_harness(ramp_app(ColorMode::Rgb, SampleType::U8));
    let id = crate::adjust_dialog::open(h.state_mut(), "image.adjustments.curves").unwrap();
    h.ctx.data_mut(|d| d.insert_temp(mem().with("curves-ch"), 1usize));
    h.run_steps(3);
    let graph = h.ctx.data(|d| d.get_temp::<egui::Rect>(mem().with("curves-graph"))).unwrap();
    let strip = crate::adjust_editors::curve_strip(graph);
    let at = |level: f32| egui::pos2(graph.left() + level / 255.0 * graph.width(), strip.center().y);
    let alt = egui::Modifiers::ALT;
    modifiers(&mut h, alt);
    h.event(egui::Event::PointerMoved(at(0.0)));
    press(&mut h, at(0.0), true, alt);
    h.event(egui::Event::PointerMoved(at(100.0)));
    h.run_steps(2);
    assert_eq!(field(&h), Some(json!("shadows")));
    let fields = h.state().ui.dialogs[0].fields.clone();
    let red = fields["red"][0][0].as_f64().unwrap();
    assert!((95.0..=105.0).contains(&red), "the red curve's black point moved: {}", fields["red"]);
    assert!(fields.get("points").is_none_or(|p| p[0][0].as_f64() == Some(0.0)), "the composite stays");
    // The canvas display: red clipped up to the black point (the ramp has green 0.5, blue 1).
    let ctx = egui::Context::default();
    let painter = egui::Painter::new(ctx.clone(), egui::LayerId::background(), egui::Rect::EVERYTHING);
    paint(h.state_mut(), &ctx, &painter, &view(10.0), 0);
    let c = h.state().levels_clipping.as_ref().unwrap();
    let (end, thr) = c.drawn.unwrap();
    let px = image(&c.stats, &thr, end);
    let level = |x: usize| x as f32 / 15.0 * 255.0;
    for (x, p) in px.iter().take(16).enumerate() {
        assert_eq!(*p, if level(x) <= red as f32 { Color32::from_rgb(0, 255, 255) } else { Color32::WHITE }, "x={x}");
    }
    press(&mut h, at(100.0), false, alt);
    assert!(h.state().ui.dialogs.iter().any(|d| d.id == id));
}
