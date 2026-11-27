//! A right click on a toolbar tool offers Assign Shortcut…, which opens Keyboard Shortcuts (fork/assign-shortcut-palette.md).

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;

use crate::PhotocraftApp;

fn harness() -> Harness<'static, Box<PhotocraftApp>> {
    // Tall, so the whole rail shows: a tool the rail clips takes no pointer.
    let mut h = Harness::builder().with_size(vec2(1400.0, 1400.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::with_context(cc.egui_ctx.clone(), photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width": 64, "height": 48})).unwrap();
        app
    });
    h.run_steps(8);
    h
}

/// The Zoom tool sits alone in its toolbar slot, so its right click is free for the item.
#[test]
fn right_click_on_a_toolbar_tool_offers_assign_shortcut_and_opens_the_dialog() {
    let mut h = harness();
    assert!(!crate::prefs_ui::shortcuts_open(h.state()));
    h.get_by_label_contains("Zoom Tool").click_secondary();
    h.run_steps(4);
    h.get_by_role_and_label(Role::MenuItem, "Assign Shortcut…").click();
    h.run_steps(6);
    assert!(crate::prefs_ui::shortcuts_open(h.state()), "Keyboard Shortcuts opened");
    // The rail names the tool `toolSelect.zoom`; Keyboard Shortcuts lists it by the id of its saved keys.
    let id = crate::menu_custom::shortcut::dialog_id("toolSelect.zoom");
    assert!(crate::prefs_ui::open_shortcuts_for(h.state_mut(), &id), "{id} is a row of Keyboard Shortcuts");
}

/// A tool in a flyout: the right click of the slot opens the flyout, and a right click of a row offers the item.
#[test]
fn right_click_on_a_flyout_row_offers_assign_shortcut_too() {
    let mut h = harness();
    h.get_by_label_contains("Rectangular Marquee Tool").click_secondary();
    h.run_steps(4);
    h.get_by_label_contains("Elliptical Marquee Tool").click_secondary();
    h.run_steps(4);
    h.get_by_role_and_label(Role::MenuItem, "Assign Shortcut…").click();
    h.run_steps(6);
    assert!(crate::prefs_ui::shortcuts_open(h.state()), "Keyboard Shortcuts opened");
}
