//! The presets control of Preferences › Context Menus: one small dropdown showing the active preset
//! (a dot after its name when the menus changed since it was saved), then the Export and Import
//! icon buttons for the backup file. The dropdown's menu switches presets, saves the menus to one,
//! clears one and undoes the last switch. The dropdown itself shows the state (the active preset, a
//! dot when it changed, empty presets greyed), so no action says anything on the status line.

use egui::{Rect, Sense, Stroke, pos2, vec2};

use super::presets::COUNT;
use super::{Custom, backup};
use crate::PhotocraftApp;
use crate::theme::Tokens;

/// A small icon button: a tray with an arrow out of it (`up`, Export) or into it (Import).
fn tray_button(ui: &mut egui::Ui, t: &Tokens, up: bool, tip: &str) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    if r.hovered() {
        ui.painter().rect_filled(rect, t.radius_sm, t.hover);
    }
    let stroke = Stroke::new(1.5, if r.hovered() { t.text } else { t.icon });
    let c = rect.center();
    let p = ui.painter();
    p.add(egui::Shape::line(vec![pos2(c.x - 6.0, c.y + 1.0), pos2(c.x - 6.0, c.y + 6.0), pos2(c.x + 6.0, c.y + 6.0), pos2(c.x + 6.0, c.y + 1.0)], stroke));
    let (from, to, wing) = if up { (c.y + 2.0, c.y - 7.0, 3.0) } else { (c.y - 7.0, c.y + 2.0, -3.0) };
    p.line_segment([pos2(c.x, from), pos2(c.x, to)], stroke);
    p.add(egui::Shape::line(vec![pos2(c.x - 3.0, to + wing), pos2(c.x, to), pos2(c.x + 3.0, to + wing)], stroke));
    r.on_hover_text(tip).clicked()
}

/// The dropdown's label: the active preset, with a dot when its menus were changed since.
fn label(custom: &Custom) -> (String, bool) {
    let states = custom.states();
    match states.iter().position(|s| s.active) {
        Some(i) => (format!("{} {}{}", tl!("Preset"), i + 1, if states[i].edited { " •" } else { "" }), states[i].edited),
        None => (tl!("Presets").to_string(), false),
    }
}

/// A menu entry; a check mark at its right end when `checked`, greyed when not `enabled`. True when clicked.
fn entry(ui: &mut egui::Ui, text: &str, checked: bool, enabled: bool) -> bool {
    let r = ui.add_enabled(enabled, egui::Button::new(text));
    if checked {
        let t = Tokens::get(ui.ctx());
        crate::icons::paint(ui, Rect::from_center_size(pos2(r.rect.right() - 12.0, r.rect.center().y), vec2(13.0, 13.0)), "check", 13.0, t.text);
    }
    r.clicked()
}

/// The dropdown: a button showing `text`, and the chevron.
fn dropdown_button(ui: &mut egui::Ui, t: &Tokens, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(12.0), t.text);
    let size = vec2(galley.size().x.max(60.0) + 36.0, 24.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    let fill = if r.hovered() { t.hover } else { t.field };
    ui.painter().rect(rect, t.radius_sm, fill, Stroke::new(1.0, t.field_border), egui::StrokeKind::Inside);
    ui.painter().galley(pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    crate::icons::paint(ui, Rect::from_center_size(pos2(rect.right() - 12.0, rect.center().y), vec2(14.0, 14.0)), "chevron-down", 14.0, t.icon);
    r
}

/// The Save to Preset and Clear Preset submenus.
fn manage(ui: &mut egui::Ui, custom: &mut Custom) {
    ui.menu_button(tl!("Save to Preset"), |ui| {
        for n in 1..=COUNT {
            if entry(ui, &n.to_string(), false, true) {
                let _ = custom.save_to(n - 1);
                ui.close();
            }
        }
    });
    let states = custom.states();
    if states.iter().any(|s| s.saved) {
        ui.menu_button(tl!("Clear Preset"), |ui| {
            for n in (1..=COUNT).filter(|n| states[n - 1].saved) {
                if entry(ui, &format!("{} {n}", tl!("Preset")), false, true) {
                    let _ = custom.clear(n - 1);
                    ui.close();
                }
            }
        });
    }
}

/// The presets control, laid out right to left on the heading's line.
pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui, custom: &mut Custom, t: &Tokens) {
    if tray_button(ui, t, false, tl!("Import Context Menus…")) {
        backup::import(app);
    }
    if tray_button(ui, t, true, tl!("Export Context Menus…")) {
        backup::export(app);
    }
    let (text, edited) = label(custom);
    let r = dropdown_button(ui, t, &text);
    let r = if edited { r.on_hover_text(tl!("Changed since saved")) } else { r };
    egui::Popup::menu(&r).show(|ui| {
        crate::widgets::style_spectrum_popup_menu(ui);
        ui.set_min_width(150.0);
        let states = custom.states();
        for n in 1..=COUNT {
            let s = states[n - 1];
            let name = format!("{} {n}", tl!("Preset"));
            let text = if s.saved { name } else { format!("{name} ({})", tl!("Empty")) };
            // an empty preset has nothing to switch to
            if entry(ui, &text, s.active, s.saved) {
                let _ = custom.switch_to(n - 1);
                ui.close();
            }
        }
        ui.separator();
        manage(ui, custom);
        if custom.can_undo() {
            ui.separator();
            if ui.button(tl!("Undo")).on_hover_text(tl!("Undo Switch")).clicked() {
                let _ = custom.undo();
                ui.close();
            }
        }
    });
}
