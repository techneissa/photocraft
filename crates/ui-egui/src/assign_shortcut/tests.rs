use egui::{Event, Modifiers, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::{Value, json};

use crate::PhotocraftApp;

fn app_harness() -> Harness<'static, PhotocraftApp> {
    app_harness_sized(vec2(1400.0, 900.0))
}

fn app_harness_sized(size: egui::Vec2) -> Harness<'static, PhotocraftApp> {
    let mut h = Harness::builder().with_size(size).with_max_steps(64).build_eframe(|cc| {
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
fn right_click_on_an_in_window_menu_row_offers_assign_shortcut() {
    // Tall enough that the Edit menu's last rows are on screen.
    let mut h = app_harness_sized(vec2(1400.0, 1800.0));
    h.get_by_label("Edit").click();
    h.run_steps(3);
    h.get_by_label_contains("Keyboard Shortcuts").click_secondary();
    h.run_steps(3);
    assert!(h.query_by_label_contains("Assign Shortcut").is_some(), "the menu closes and the small menu opens");
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert_asked_for(&h, "edit.keyboardShortcuts");
}

#[test]
fn right_click_on_a_layers_footer_button_offers_assign_shortcut() {
    let mut h = app_harness();
    h.get_by_label_contains("Create a new layer").click_secondary();
    h.run_steps(2);
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert_asked_for(&h, "layer.new.layer");
}

#[test]
fn right_click_on_a_lone_toolbar_tool_offers_assign_shortcut() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let mut h = Harness::builder().with_size(vec2(800.0, 1400.0)).build_ui_state(
        |ui, app: &mut PhotocraftApp| {
            if ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                crate::panels::toolbar(app, ui);
            }
            super::show(app, &ui.ctx().clone());
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, Default::default());
    h.run_steps(2);
    let tool = h.state().ui.tool;
    h.get_by_label_contains("Zoom Tool").click_secondary();
    h.run_steps(2);
    assert_eq!(h.state().ui.tool, tool, "a right-click doesn't pick the tool");
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert_asked_for(&h, "tools.select.zoom");
}

#[test]
fn the_small_menu_shows_the_commands_current_shortcut_and_closes_on_an_outside_click() {
    let mut h = app_harness();
    super::offer(&h.ctx.clone(), "edit.undo", egui::pos2(300.0, 300.0));
    h.run_steps(2);
    assert!(super::is_open(&h.ctx));
    h.get_by_label_contains("Assign Shortcut").hover();
    h.event(Event::PointerMoved(egui::pos2(900.0, 700.0)));
    h.event(Event::PointerButton { pos: egui::pos2(900.0, 700.0), button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert!(!super::is_open(&h.ctx), "a press outside closes it");
    assert!(dialog_field(h.state(), "selected").is_none());
}

#[test]
fn open_shortcuts_for_reuses_the_open_dialog() {
    let mut h = app_harness();
    let a = crate::prefs_ui::open_shortcuts_for(h.state_mut(), "edit.undo");
    let b = crate::prefs_ui::open_shortcuts_for(h.state_mut(), "tools.select.zoom");
    assert_eq!(a, b, "one Keyboard Shortcuts dialog");
    assert_asked_for(&h, "tools.select.zoom");
    assert_eq!(dialog_field(h.state(), "filter"), Some(json!("Zoom Tool")), "its row is in view");
}

#[test]
fn right_click_on_a_wrapped_icon_button_offers_assign_shortcut_and_a_left_click_still_clicks() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let mut h = Harness::builder().with_size(vec2(400.0, 300.0)).build_ui_state(
        |ui, (app, clicks): &mut (PhotocraftApp, u32)| {
            if super::button(ui, "search", 28.0, false, "Search commands", "edit.search").clicked() {
                *clicks += 1;
            }
            super::show(app, &ui.ctx().clone());
        },
        (app, 0u32),
    );
    PhotocraftApp::setup_context(&h.ctx, Default::default());
    h.run_steps(2);
    h.get_by_label("Search commands").click();
    h.run_steps(2);
    assert_eq!(h.state().1, 1, "a left click is the button's own");
    assert!(!super::is_open(&h.ctx));
    h.get_by_label("Search commands").click_secondary();
    h.run_steps(2);
    assert_eq!(h.state().1, 1, "a right-click is not a click");
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert_asked_for_in(&h.state().0, "edit.search");
}

fn assert_asked_for_in(app: &PhotocraftApp, command: &str) {
    assert_eq!(dialog_field(app, "selected"), Some(json!(command)));
    assert_eq!(dialog_field(app, "capture"), Some(json!(true)));
}
