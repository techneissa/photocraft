//! The Control+Option drag begins on the press, not once egui calls the movement a drag (about
//! 6 points later): until then the brush stood still, and the first stretch of the drag was lost.

use egui::PointerButton;

use super::*;
use crate::canvas::tool_event;

/// A press with Control+Option on a painting tool begins the resize at the press point.
pub fn begin_on_press(app: &mut PhotocraftApp, ui: &egui::Ui, rect: egui::Rect, xf: &ViewXform, tool: Tool, mods: egui::Modifiers) {
    if !applies(tool) || app.drag.is_some() || app.brush_resize.is_some() {
        return;
    }
    let (pressed, origin, press_mods) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.press_origin(),
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::PointerButton { button: PointerButton::Primary, pressed: true, modifiers, .. } => Some(*modifiers),
                _ => None,
            }),
        )
    });
    let press_mods = press_mods.unwrap_or(mods);
    let Some(p) = origin.filter(|p| pressed && rect.contains(*p) && is_gesture(press_mods)) else { return };
    if ui.input(|i| i.pointer.secondary_down()) {
        return;
    }
    let d = xf.to_doc(p);
    tool_event(app, ToolEvent::Down { x: d[0], y: d[1], pressure: 1.0 }, press_mods);
}

/// A primary-button resize is in progress: it owns the press, so egui's drag start and click for
/// the same press must not begin another gesture (a second `Down` would restart from the changed brush).
pub fn owns_press(app: &PhotocraftApp) -> bool {
    app.brush_resize.is_some_and(|r| !r.secondary)
}

/// True while a primary-button resize is in progress: the canvas then feeds it every frame's
/// pointer position, drag recognised or not.
pub fn moves_on_press(app: &PhotocraftApp, ui: &egui::Ui) -> bool {
    app.brush_resize.is_some_and(|r| !r.secondary) && ui.input(|i| i.pointer.primary_down())
}

/// End a primary-button resize whose release the drag handling did not report (a press that never
/// became a drag, or a release outside the canvas).
pub fn finish_on_release(app: &mut PhotocraftApp, ui: &egui::Ui, xf: &ViewXform, mods: egui::Modifiers) {
    if !app.brush_resize.is_some_and(|r| !r.secondary) || ui.input(|i| i.pointer.primary_down()) {
        return;
    }
    let at = ui.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()));
    let [x, y] = at.map_or(app.brush_resize.map_or([0.0; 2], |r| r.anchor), |p| xf.to_doc(p));
    tool_event(app, ToolEvent::Up { x, y }, mods);
    app.brush_resize = None;
}
