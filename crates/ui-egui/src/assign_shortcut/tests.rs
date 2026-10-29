use egui::{Event, Key, Modifiers, vec2};
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
    assert_eq!(dialog_field(h.state(), "reveal"), Some(json!(true)), "its row is scrolled into view");
}

#[test]
fn right_click_on_an_icon_button_marked_with_next_offers_assign_shortcut_and_a_left_click_still_clicks() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let mut h = Harness::builder().with_size(vec2(400.0, 300.0)).build_ui_state(
        |ui, (app, clicks): &mut (PhotocraftApp, u32)| {
            super::next(ui, "edit.search");
            if crate::icons::button(ui, "search", 28.0, false, "Search commands").clicked() {
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

fn toolbar_harness() -> Harness<'static, PhotocraftApp> {
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
    h
}

#[test]
fn right_click_on_the_toolbars_edit_and_quick_mask_buttons_offers_assign_shortcut() {
    for (label, command) in [("Edit Toolbar…", "edit.toolbar"), ("Edit in Quick Mask Mode", "select.editInQuickMaskMode")] {
        let mut h = toolbar_harness();
        h.get_by_label_contains(label).click_secondary();
        h.run_steps(2);
        h.get_by_label_contains("Assign Shortcut").click();
        h.run_steps(3);
        assert_asked_for(&h, command);
    }
}

#[test]
fn right_click_on_a_flyout_tool_row_offers_assign_shortcut_and_closes_the_flyout() {
    let mut h = toolbar_harness();
    // Right-click on a tool that shares its slot opens the flyout, not the small menu.
    let button = h.get_by_label("Brush Tool  (B)").rect();
    h.get_by_label("Brush Tool  (B)").click_secondary();
    h.run_steps(3);
    assert!(!super::is_open(&h.ctx), "the flyout takes the right-click on the tool's own button");
    // The second row of the flyout (Pencil), beside the button.
    let margin = egui::Frame::popup(&h.ctx.global_style()).inner_margin.top as f32;
    let row = egui::pos2(button.right() + 6.0 + margin + 60.0, button.top() + margin + 26.0 + 13.0);
    h.event(Event::PointerMoved(row));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: row, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
    assert!(super::is_open(&h.ctx), "a right-click on a flyout row offers the small menu");
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    assert_asked_for(&h, "tools.select.pencil");
}

#[test]
fn the_options_bar_tool_icons_menu_has_assign_shortcut() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let mut h = Harness::builder().with_size(vec2(1200.0, 300.0)).build_ui_state(
        |ui, app: &mut PhotocraftApp| {
            if ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                crate::panels::options_bar(app, ui);
            }
            super::show(app, &ui.ctx().clone());
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, Default::default());
    h.run_steps(2);
    let tool = h.state().ui.tool;
    h.get_by_label(crate::i18n::t(tool.label())).click_secondary();
    h.run_steps(3);
    assert!(h.query_by_label("Reset Tool").is_some(), "its own menu still opens");
    h.get_by_label("Assign Shortcut…").click();
    h.run_steps(3);
    assert_asked_for(&h, &crate::tool_shortcuts::id(tool));
}

#[test]
fn esc_and_tool_keys_belong_to_the_small_menu() {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    h.state_mut().run("select.all", json!({})).unwrap();
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    h.run_steps(2);
    super::offer(&ctx, "edit.undo", egui::pos2(300.0, 300.0));
    h.run_steps(2);
    h.key_press(Key::V);
    h.run_steps(2);
    assert_eq!(h.state().ui.tool, crate::state::Tool::Brush, "V doesn't switch tools under the menu");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!super::is_open(&ctx), "Esc closes the menu");
    assert!(h.state().session.active().is_some_and(|s| s.doc.selection.is_some()), "and deselects nothing");
    h.key_press(Key::V);
    h.run_steps(2);
    assert_eq!(h.state().ui.tool, crate::state::Tool::Move, "with the menu gone V is the Move tool again");
}

#[test]
fn a_click_outside_closes_the_menu_and_goes_no_further() {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    h.state_mut().run("layer.new.layer", json!({})).unwrap();
    h.state_mut().sync_views();
    h.run_steps(2);
    let layers = h.state().session.active().unwrap().doc.layer_count();
    super::offer(&ctx, "edit.undo", egui::pos2(300.0, 300.0));
    h.run_steps(2);
    h.get_by_label("Delete layer").click();
    h.run_steps(3);
    assert!(!super::is_open(&ctx), "the click closes the menu");
    assert_eq!(h.state().session.active().unwrap().doc.layer_count(), layers, "and doesn't press Delete Layer");
    assert!(h.state().ui.dialogs.is_empty() && h.state().ui.layer_menu.is_none());
    // With the menu gone the same button works.
    h.get_by_label("Delete layer").click();
    h.run_steps(3);
    assert!(h.state().session.active().unwrap().doc.layer_count() < layers || !h.state().ui.dialogs.is_empty());
}

#[test]
fn a_menu_from_a_palette_row_goes_with_the_palette() {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(2);
    h.event(Event::Text("zoom tool".into()));
    h.run_steps(2);
    h.get_by_label("Zoom Tool").click_secondary();
    h.run_steps(2);
    assert!(super::is_open(&ctx));
    // Enter and the arrows are the menu's, not the palette's: nothing runs, the highlight stays.
    h.key_press(Key::Enter);
    h.key_press(Key::ArrowDown);
    h.run_steps(2);
    assert!(h.state().ui.palette_open && super::is_open(&ctx), "Enter does not run the highlighted row");
    h.state_mut().ui.palette_open = false;
    h.run_steps(2);
    assert!(!super::is_open(&ctx), "the palette closing takes the menu with it");
}

#[test]
fn open_shortcuts_for_a_command_the_dialog_does_not_list_captures_nothing() {
    let mut h = app_harness();
    crate::prefs_ui::open_shortcuts_for(h.state_mut(), "no.such.command");
    assert_eq!(dialog_field(h.state(), "capture"), Some(json!(false)));
    assert_eq!(dialog_field(h.state(), "selected"), Some(json!("")));
    crate::prefs_ui::open_shortcuts_for(h.state_mut(), "edit.undo");
    assert_eq!(dialog_field(h.state(), "capture"), Some(json!(true)));
}

#[test]
fn the_add_mask_button_offers_the_command_it_runs() {
    let mut h = app_harness();
    h.state_mut().run("select.all", json!({})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Add a mask").click_secondary();
    h.run_steps(2);
    h.get_by_label_contains("Assign Shortcut").click();
    h.run_steps(3);
    // A selection makes it the mask from the selection, not Reveal All.
    assert_asked_for(&h, "layer.layerMask.revealSelection");
}
