//! The Control+Option drag through the real canvas holds the brush at its limits: turning back after dragging past
//! one changes the brush at once (fork/brush-resize-drag.md).

use egui::Modifiers;
use serde_json::json;

use super::*;
use crate::canvas::testing;

fn ctrl_alt() -> Modifiers {
    Modifiers { alt: true, ctrl: true, ..Default::default() }
}

/// A 300 × 200 document with a 40 px brush at 50 % hardness.
fn app() -> Box<PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 300, "height": 200, "background": "white"})).unwrap();
    app.ui.tool = Tool::Brush;
    crate::paint_mouse::sync_tool_brush(&mut app);
    app.run("tools.setBrush", json!({"brush": {"size": 40, "hardness": 0.5}})).unwrap();
    app.sync_views();
    testing::on_a_mac(&app);
    app
}

/// Dragging far left past the 1 px limit and back 25 points right changes the diameter at once: the pointer is still
/// left of the press, where the framework alone (distance from the press) would still hold 1 px.
#[test]
fn turning_back_after_passing_the_diameter_limit_changes_the_brush_at_once() {
    let mut a = app();
    testing::press(&mut a, [150.0, 100.0], ctrl_alt());
    testing::move_to(&mut a, [60.0, 100.0], ctrl_alt());
    assert_eq!(a.session.tools.brush.size, 1.0, "dragged left to the smallest diameter");
    testing::move_to(&mut a, [85.0, 100.0], ctrl_alt());
    let size = a.session.tools.brush.size;
    testing::release(&mut a, [85.0, 100.0], ctrl_alt());
    assert!(size > 1.0, "turning back from far past the limit grows the brush at once, got {size}");
}

/// The same for the hardness: down past 100 % and back up.
#[test]
fn turning_back_after_passing_the_hardness_limit_changes_the_brush_at_once() {
    let mut a = app();
    // 90 % hardness: the limit is 38 screen points below the press, however the view is zoomed the drag passes it.
    a.run("tools.setBrush", json!({"brush": {"hardness": 0.9}})).unwrap();
    testing::press(&mut a, [150.0, 40.0], ctrl_alt());
    testing::move_to(&mut a, [150.0, 190.0], ctrl_alt());
    assert_eq!(a.session.tools.brush.hardness, 1.0, "dragged down to full hardness");
    testing::move_to(&mut a, [150.0, 170.0], ctrl_alt());
    let hardness = a.session.tools.brush.hardness;
    testing::release(&mut a, [150.0, 170.0], ctrl_alt());
    assert!(hardness < 1.0, "turning back from past the limit softens the brush at once, got {hardness}");
}
