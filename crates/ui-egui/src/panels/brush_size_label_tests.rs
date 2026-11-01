//! The brush size under the brush preview in the options bar stays inside the chip, whatever the
//! size (1 to 5000) and the display scale, so the options bar never cuts it (also in the narrow
//! bar, where the chip is as tall as the bar).

use super::brush_controls::brush_preset_chip;
use super::*;

/// The chip's rectangle and the bounds of the text it draws, at display scale `ppp`.
fn chip_and_label(size: f32, ppp: f32) -> (Rect, Rect, Rect) {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let mut brush = app.session.tools.brush.clone();
    brush.size = size;
    let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))), ..Default::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let mut chip = Rect::NOTHING;
    let mut tip = Rect::NOTHING;
    // Two passes: the first lays fonts out.
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(input.clone(), |ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
                chip = Rect::from_min_size(ui.cursor().min, vec2(44.0, 30.0));
                brush_preset_chip(ui, &brush, &mut app.ui);
            });
        });
        // Nothing paints these textures here; the context wants them taken.
        out.textures_delta.clear();
        shapes = out.shapes;
    }
    let label = size.round().to_string();
    let mut bounds = Rect::NOTHING;
    for clipped in shapes {
        match clipped.shape {
            egui::Shape::Text(text) if text.galley.text() == label => bounds = text.galley.rect.translate(text.pos.to_vec2()),
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() && !mesh.vertices.is_empty() => {
                let r = mesh.calc_bounds();
                if r.width() <= 16.0 && r.min.x < chip.min.x + 24.0 {
                    tip = r;
                }
            }
            _ => {}
        }
    }
    (chip, bounds, tip)
}

#[test]
fn the_brush_size_stays_inside_the_chip_at_every_size_and_scale() {
    for ppp in [1.0, 1.25, 1.5, 2.0, 3.0] {
        for size in [1.0, 9.0, 99.0, 348.0, 1000.0, 5000.0] {
            let (chip, label, _) = chip_and_label(size, ppp);
            assert!(label.is_positive(), "the size {size} is drawn (ppp {ppp})");
            assert!(chip.contains_rect(label), "size {size} at scale {ppp}: the text {label:?} leaves the chip {chip:?}");
            assert!(chip.bottom() - label.bottom() >= 1.0, "size {size} at scale {ppp}: no room under the text to the bar's bottom edge");
        }
    }
}

#[test]
fn the_brush_size_does_not_touch_the_preview_above_it() {
    let (_, label, tip) = chip_and_label(348.0, 2.0);
    if tip.is_positive() {
        assert!(tip.bottom() <= label.top() + 0.5, "the preview {tip:?} and the text {label:?} overlap");
    }
}
