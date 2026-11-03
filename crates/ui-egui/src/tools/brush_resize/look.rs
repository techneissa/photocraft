//! Photoshop's on-canvas resize look: a small dark rounded label beside the pointer in short plain
//! text (`Diameter: 120 px`, and `Hardness: 50%` once the hardness is being changed). The red tip
//! overlay is `tip_preview` in `brush_resize.rs`.

use egui::{Color32, Pos2, vec2};

/// The overlay's alpha: about half, as Photoshop's.
pub const OVERLAY_ALPHA: u8 = 128;

/// The label's lines: the diameter, and the second value (Hardness, or Opacity with "Vary Round
/// Brush Hardness on HUD" off) once the vertical movement has changed it from where the drag began.
pub fn lines(size: f32, second: (&str, f32), second_changed: bool) -> Vec<String> {
    let mut lines = vec![format!("{} {} px", tl!("Diameter:"), size.round() as i64)];
    if second_changed {
        lines.push(format!("{} {}%", second.0, (second.1 * 100.0).round() as i64));
    }
    lines
}

/// Draw the label beside `at`, above everything.
pub fn label(ctx: &egui::Context, id: &str, at: Pos2, lines: &[String]) {
    let font = egui::FontId::proportional(12.0);
    egui::Area::new(egui::Id::unique(id)).order(egui::Order::Tooltip).fixed_pos(at + vec2(16.0, 18.0)).interactable(false).constrain(true).show(ctx, |ui| {
        egui::Frame::new().fill(Color32::from_rgba_unmultiplied(28, 28, 28, 235)).corner_radius(6).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
            for line in lines {
                ui.add(egui::Label::new(egui::RichText::new(line).font(font.clone()).color(Color32::from_gray(240))).extend());
            }
        });
    });
}
