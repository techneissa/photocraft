use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::{Value, json};

use crate::PhotocraftApp;

fn app_harness() -> Harness<'static, PhotocraftApp> {
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
    });
    h.state_mut().run("file.new", json!({"width": 64, "height": 64})).unwrap();
    h.state_mut().sync_views();
    h.run_steps(2);
    h
}

/// The Keyboard Shortcuts dialog's field, if the dialog is open.
fn dialog_field(app: &PhotocraftApp, name: &str) -> Option<Value> {
    app.ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(Value::as_str) == Some("shortcuts"))?.fields.get(name).cloned()
}

fn assert_asked_for(h: &Harness<'_, PhotocraftApp>, command: &str) {
    assert_eq!(dialog_field(h.state(), "selected"), Some(json!(command)), "Keyboard Shortcuts opens on {command}");
    assert_eq!(dialog_field(h.state(), "capture"), Some(json!(true)), "key capture is on");
    assert_eq!(dialog_field(h.state(), "tab"), Some(json!(0)));
}

#[test]
fn right_click_on_a_palette_row_offers_assign_shortcut() {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(2);
    h.event(Event::Text("clipping mask".into()));
    h.run_steps(2);
    h.get_by_label("Create Clipping Mask").click_secondary();
    h.run_steps(2);
    assert!(h.state().ui.palette_open, "the right-click leaves the palette open until a pick");
    assert!(h.query_by_label_contains("Assign Shortcut").is_some(), "a small menu offers Assign Shortcut…");
    assert!(dialog_field(h.state(), "selected").is_none(), "the dialog waits for the pick");
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert!(!h.state().ui.palette_open, "the palette makes way for the dialog");
    assert!(h.query_by_label_contains("Assign Shortcut").is_none(), "the menu is gone");
    assert_asked_for(&h, "layer.createClippingMask");
}

#[test]
fn esc_closes_the_small_menu_before_the_palette() {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(2);
    h.event(Event::Text("clipping mask".into()));
    h.run_steps(2);
    h.get_by_label("Create Clipping Mask").click_secondary();
    h.run_steps(2);
    assert!(h.query_by_label_contains("Assign Shortcut").is_some());
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.query_by_label_contains("Assign Shortcut").is_none(), "Esc closes the small menu");
    assert!(h.state().ui.palette_open, "and only the small menu");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!h.state().ui.palette_open, "the next Esc closes the palette");
}

#[test]
fn right_click_on_a_palette_tool_row_asks_for_that_tools_shortcut_row() {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(2);
    h.event(Event::Text("zoom tool".into()));
    h.run_steps(2);
    h.get_by_label("Zoom Tool").click_secondary();
    h.run_steps(2);
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert_asked_for(&h, "tools.select.zoom");
}

#[test]
fn a_command_palette_shortcut_cmd_p_opens_the_palette_and_print_has_no_key() {
    let mut h = app_harness();
    assert!(!h.state().ui.palette_open);
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::P]);
    h.run_steps(3);
    assert!(h.state().ui.palette_open, "⌘P opens the palette");
    assert!(h.state().ui.dialogs.is_empty(), "and doesn't open Print");
    assert_eq!(crate::shortcuts::default_shortcut("file.print"), None, "Print keeps its menu item with no key");
    let menus = crate::menus::menu_items(h.state());
    assert_eq!(menus.iter().find(|m| m.id == "file.print").map(|m| m.shortcut.clone()), Some(None));
    assert_eq!(crate::shortcuts::default_shortcut("edit.search").as_deref(), Some("Cmd+F"), "⌘F still opens it");
}
