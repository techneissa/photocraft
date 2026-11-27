//! The Menus tab's colour control: the item's colour as a dot (an empty circle with a red slash for
//! none) that opens a small popover of colour dots; the names are tooltips.

use egui::{Align, Color32, Layout, PopupCloseBehavior, Pos2, Response, Sense, Stroke, vec2};

use crate::theme::Tokens;

/// The width of the Color column.
pub(super) const COLOR_WIDTH: f32 = 40.0;

/// The colours a menu item can have, in the order of the popover: the id stored in the settings
/// and the name shown as its tooltip.
pub(super) fn colors() -> [(&'static str, &'static str); 8] {
    [
        ("none", tl!("None")),
        ("red", tl!("Red")),
        ("orange", tl!("Orange")),
        ("yellow", tl!("Yellow")),
        ("green", tl!("Green")),
        ("blue", tl!("Blue")),
        ("violet", tl!("Violet")),
        ("gray", tl!("Gray")),
    ]
}

fn name_of(color: &str) -> &'static str {
    colors().into_iter().find(|(id, _)| *id == color).map_or(tl!("None"), |(_, name)| name)
}

/// Paint a colour dot: a filled circle, or for `None` an empty one with a red diagonal slash.
fn paint_dot(painter: &egui::Painter, center: Pos2, radius: f32, color: &str, t: &Tokens) {
    match crate::menus::menu_tint(color) {
        Some(fill) => {
            painter.circle_filled(center, radius, fill);
            painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_black_alpha(90)));
        }
        None => {
            painter.circle_stroke(center, radius, Stroke::new(1.3, t.text_faint));
            let d = radius * std::f32::consts::FRAC_1_SQRT_2;
            painter.line_segment([center + vec2(-d, d), center + vec2(d, -d)], Stroke::new(1.5, t.danger));
        }
    }
}

/// One clickable dot of `size` points with a ring when `selected` or hovered.
fn dot_button(ui: &mut egui::Ui, t: &Tokens, color: &str, size: f32, selected: bool) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
    if selected || r.hovered() {
        ui.painter().circle_stroke(rect.center(), size / 2.0 - 0.5, Stroke::new(1.5, if selected { t.text } else { t.text_dim }));
    }
    paint_dot(ui.painter(), rect.center(), size / 2.0 - 3.5, color, t);
    r
}

/// The colour dot of a row, centred in its column. A click opens a popover of the colour dots;
/// true when one was chosen (`color` is then the new id, `"none"` for no colour).
pub(super) fn color_dot(ui: &mut egui::Ui, t: &Tokens, color: &mut String) -> bool {
    let shown = name_of(color);
    let label = crate::i18n::fmt(tl!("Color: {name}"), &[("name", shown)]);
    let r = ui.allocate_ui_with_layout(vec2(COLOR_WIDTH, 22.0), Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
        let (rect, r) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
        if r.hovered() {
            ui.painter().circle_stroke(rect.center(), 9.5, Stroke::new(1.5, t.text_dim));
        }
        paint_dot(ui.painter(), rect.center(), 6.5, color, t);
        r.on_hover_cursor(egui::CursorIcon::PointingHand)
    });
    let r = r.inner;
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    let r = r.on_hover_text(label.clone());
    let mut chosen = false;
    egui::Popup::menu(&r).close_behavior(PopupCloseBehavior::CloseOnClick).align(egui::RectAlign::BOTTOM_END).show(|ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (id, name) in colors() {
                let dot = dot_button(ui, t, id, 22.0, *color == id);
                dot.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                if dot.on_hover_text(name).clicked() {
                    *color = id.to_string();
                    chosen = true;
                }
            }
        });
    });
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_the_setting_supports_is_in_the_popover_and_has_a_name() {
        let ids: Vec<&str> = colors().iter().map(|c| c.0).collect();
        assert_eq!(ids, ["none", "red", "orange", "yellow", "green", "blue", "violet", "gray"]);
        for (id, name) in colors() {
            assert!(!name.is_empty(), "{id}");
            assert_eq!(crate::menus::menu_tint(id).is_some(), id != "none", "{id}: only none is without a tint");
        }
        assert_eq!(name_of("violet"), "Violet");
        assert_eq!(name_of("unknown"), "None");
    }
}
