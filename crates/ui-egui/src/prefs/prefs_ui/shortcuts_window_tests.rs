//! Edit › Keyboard Shortcuts and Menus: one window size for the three tabs, recording a shortcut
//! like VS Code, a quiet selection, no dynamic menu items, the search row, the reset icon,
//! removing a row's shortcuts, and the search by modifier words.

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::PhotocraftApp;

fn harness(size: egui::Vec2, setup: impl FnOnce(&mut PhotocraftApp)) -> Harness<'static, PhotocraftApp> {
    Harness::builder().with_size(size).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        setup(&mut app);
        app
    })
}

/// The window of `command` (`edit.keyboardShortcuts`, `edit.menus`, `edit.toolbar`) with `fields` set.
fn open(h: &mut Harness<'_, PhotocraftApp>, command: &str, fields: &[(&str, Value)]) -> u64 {
    let ctx = h.ctx.clone();
    let id = crate::menus::invoke(h.state_mut(), &ctx, command, json!({})).unwrap()["dialog"].as_u64().unwrap();
    let f = &mut h.state_mut().ui.dialog_mut(id).unwrap().fields;
    for (name, v) in fields {
        f.insert((*name).into(), v.clone());
    }
    h.run_steps(6);
    id
}

fn on(command: &str, selected: &str) -> (Harness<'static, PhotocraftApp>, u64) {
    let mut h = harness(egui::vec2(1440.0, 900.0), |_| {});
    let id = open(&mut h, command, &[("selected", json!(selected))]);
    (h, id)
}

fn field(h: &Harness<'_, PhotocraftApp>, id: u64, name: &str) -> Value {
    h.state().ui.dialogs.iter().find(|d| d.id == id).unwrap().fields.get(name).cloned().unwrap_or(Value::Null)
}

fn set_field(h: &mut Harness<'_, PhotocraftApp>, id: u64, name: &str, v: Value) {
    h.state_mut().ui.dialog_mut(id).unwrap().fields.insert(name.into(), v);
}

fn key(h: &mut Harness<'_, PhotocraftApp>, key: Key, modifiers: Modifiers) {
    h.event(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run_steps(2);
    h.event(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_steps(1);
}

fn bottom(h: &Harness<'_, PhotocraftApp>) -> f32 {
    h.get_by_label("OK").rect().bottom()
}

// ------------------------------------------------------------------ size

#[test]
fn the_three_tabs_have_one_height() {
    let heights: Vec<f32> = ["edit.keyboardShortcuts", "edit.menus", "edit.toolbar"].iter().map(|c| bottom(&on(c, "").0)).collect();
    assert!(heights.iter().all(|h| (h - heights[0]).abs() < 1.0), "OK sits at {heights:?}");
    assert!(heights[0] > 600.0, "a tall window: {heights:?}");
}

#[test]
fn switching_tabs_keeps_the_height() {
    let (mut h, id) = on("edit.keyboardShortcuts", "");
    let first = bottom(&h);
    for tab in [1, 2, 0, 2, 1] {
        set_field(&mut h, id, "tab", json!(tab));
        h.run_steps(4);
        assert!((bottom(&h) - first).abs() < 1.0, "tab {tab}: OK at {} not {first}", bottom(&h));
    }
}

#[test]
fn the_toolbar_tab_lists_every_tool_without_scrolling_at_the_default_size() {
    let (h, _) = on("edit.toolbar", "");
    let restore = h.get_by_label("Restore Defaults").rect();
    for tool in crate::state::Tool::ALL {
        let name = tool.label();
        let r = h.get_all_by_label(name).next().unwrap_or_else(|| panic!("{name} is listed")).rect();
        assert!(r.bottom() <= restore.top(), "{name} ends at {} below the Restore Defaults button at {}: it needs scrolling", r.bottom(), restore.top());
    }
}

#[test]
fn the_window_takes_the_size_the_user_gave_it_and_keeps_it() {
    let mut h = harness(egui::vec2(1440.0, 900.0), |_| {});
    let ctx = h.ctx.clone();
    let wanted = egui::vec2(900.0, 460.0);
    ctx.data_mut(|m| m.insert_persisted(crate::prefs_ui::shortcuts_size_id(), wanted));
    open(&mut h, "edit.keyboardShortcuts", &[]);
    let small = bottom(&h);
    let (default, _) = on("edit.keyboardShortcuts", "");
    // The window is centred, so a window 180 points shorter ends 90 higher.
    assert!(small < bottom(&default) - 60.0, "the remembered, smaller size is used: {small} against {}", bottom(&default));
    let kept: Option<egui::Vec2> = ctx.data_mut(|m| m.get_persisted(crate::prefs_ui::shortcuts_size_id()));
    assert_eq!(kept, Some(wanted), "opening it does not change what is kept");
}

#[test]
fn dragging_the_corner_resizes_and_the_size_is_remembered() {
    let (mut h, _) = on("edit.keyboardShortcuts", "");
    let before = bottom(&h);
    let corner = egui::pos2(h.get_by_label("Cancel").rect().right() - 3.0, h.get_by_label("OK").rect().bottom() - 1.0);
    h.event(Event::PointerMoved(corner));
    h.step();
    h.event(Event::PointerButton { pos: corner, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.event(Event::PointerMoved(corner - egui::vec2(60.0, 120.0)));
    h.step();
    h.event(Event::PointerButton { pos: corner - egui::vec2(60.0, 120.0), button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(6);
    assert!(bottom(&h) < before - 60.0, "the window got shorter: {} from {before}", bottom(&h));
    let kept: Option<egui::Vec2> = h.ctx.data_mut(|m| m.get_persisted(crate::prefs_ui::shortcuts_size_id()));
    assert!(kept.is_some_and(|s| s.y < crate::prefs_ui::SHORTCUTS_SIZE.y - 60.0), "the size is kept: {kept:?}");
}

// ------------------------------------------------------------------ recording

#[test]
fn a_click_starts_recording_and_each_key_pressed_is_appended() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    assert_eq!(field(&h, id, "capture"), json!(true));
    key(&mut h, Key::K, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "__recSteps"), json!(["Cmd+K"]));
    key(&mut h, Key::J, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "__recSteps"), json!(["Cmd+K", "Cmd+J"]));
    h.get_by_label(&crate::keybinds::chord::text("Cmd+K Cmd+J"));
    assert_eq!(field(&h, id, "chords"), json!({}), "nothing is kept before Enter");
    assert!(field(&h, id, "overrides").get("file.new").is_none());
}

#[test]
fn enter_accepts_one_key_as_the_main_key() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    key(&mut h, Key::J, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+Shift+J"), "the first combination is applied at once");
    assert_eq!(field(&h, id, "capture"), json!(true), "and the recording stays open for a second one");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+Shift+J"));
    assert_eq!(field(&h, id, "capture"), json!(false));
    assert!(field(&h, id, "__recBefore").is_null(), "nothing is left to take back");
    assert!(h.state().ui.dialogs.iter().any(|d| d.id == id), "Enter accepted the recording, it did not click OK");
}

#[test]
fn enter_accepts_two_keys_in_a_row_as_a_chord() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    key(&mut h, Key::K, Modifiers::COMMAND);
    key(&mut h, Key::J, Modifiers::COMMAND);
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(field(&h, id, "chords")["file.new"], json!("Cmd+K Cmd+J"));
    assert_eq!(field(&h, id, "overrides").get("file.new"), None, "the main key stays");
    assert_eq!(field(&h, id, "capture"), json!(false));
    h.get_by_label(&format!("{}  ×", crate::keybinds::chord::text("Cmd+K Cmd+J")));
}

#[test]
fn escape_cancels_and_changes_nothing() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    key(&mut h, Key::K, Modifiers::COMMAND);
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(field(&h, id, "capture"), json!(false));
    assert_eq!(field(&h, id, "__recSteps"), json!([]));
    assert!(field(&h, id, "overrides").get("file.new").is_none());
    assert_eq!(field(&h, id, "chords"), json!({}));
    assert!(h.state().ui.dialogs.iter().any(|d| d.id == id), "Esc ended the recording, the window stays");
}

#[test]
fn escape_puts_back_the_key_the_row_had() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    set_field(&mut h, id, "overrides", json!({"file.new": "Cmd+Shift+Y"}));
    set_field(&mut h, id, "capture", json!(true));
    h.run_steps(2);
    key(&mut h, Key::K, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+K"));
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+Shift+Y"), "Esc takes the new key back");
    assert_eq!(field(&h, id, "capture"), json!(false));
}

#[test]
fn a_second_combination_takes_the_first_back_because_the_command_gets_a_chord() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    key(&mut h, Key::K, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+K"));
    key(&mut h, Key::J, Modifiers::COMMAND);
    assert!(field(&h, id, "overrides").get("file.new").is_none(), "the main key is back as it was");
    key(&mut h, Key::L, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+L"), "a third combination starts over as a key");
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert!(field(&h, id, "overrides").get("file.new").is_none());
}

#[test]
fn clicking_away_keeps_the_applied_key() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    key(&mut h, Key::K, Modifiers::COMMAND);
    set_field(&mut h, id, "capture", json!(false));
    h.run_steps(2);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+K"));
    assert!(field(&h, id, "__recBefore").is_null(), "the recording is over");
}

#[test]
fn a_third_combination_starts_the_recording_over() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    for k in [Key::K, Key::J, Key::L] {
        key(&mut h, k, Modifiers::COMMAND);
    }
    assert_eq!(field(&h, id, "__recSteps"), json!(["Cmd+L"]));
}

#[test]
fn a_held_key_is_one_combination() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    h.event(Event::Key { key: Key::K, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
    h.run_steps(1);
    for _ in 0..3 {
        h.event(Event::Key { key: Key::K, physical_key: None, pressed: true, repeat: true, modifiers: Modifiers::COMMAND });
        h.run_steps(1);
    }
    assert_eq!(field(&h, id, "__recSteps"), json!(["Cmd+K"]));
}

#[test]
fn a_key_in_use_warns_before_enter_and_enter_still_takes_it() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    key(&mut h, Key::S, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "messageWarning"), json!(true));
    let message = field(&h, id, "message");
    assert!(message.as_str().unwrap().contains("already in use"), "{message}");
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+S"), "applied with the warning");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+S"));
    assert!(field(&h, id, "conflict").is_object(), "Accept and Go to Conflict is offered");
    h.get_by_label("Accept and Go to Conflict");
}

#[test]
fn a_chord_in_use_warns_before_enter() {
    let mut h = harness(egui::vec2(1440.0, 900.0), |app| {
        app.session.edit_prefs(|p| _ = p.chords.insert("edit.copy".into(), "Cmd+Alt+U Cmd+Alt+J".into()));
    });
    let id = open(&mut h, "edit.keyboardShortcuts", &[("selected", json!("file.new")), ("capture", json!(true))]);
    key(&mut h, Key::U, Modifiers::COMMAND | Modifiers::ALT);
    assert_eq!(field(&h, id, "messageWarning"), json!(false), "one key is no clash");
    key(&mut h, Key::J, Modifiers::COMMAND | Modifiers::ALT);
    assert_eq!(field(&h, id, "messageWarning"), json!(true));
    assert!(field(&h, id, "message").as_str().unwrap().contains("already in use"));
}

#[test]
fn add_shortcut_records_a_further_key() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label("Add Shortcut").click();
    h.run_steps(2);
    assert_eq!(field(&h, id, "captureAlt"), json!(true));
    key(&mut h, Key::J, Modifiers::COMMAND | Modifiers::ALT);
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(field(&h, id, "alternates")["file.new"], json!(["Cmd+Alt+J"]));
    assert!(field(&h, id, "overrides").get("file.new").is_none(), "the main key stays");
}

#[test]
fn a_reserved_key_is_refused_with_a_reason() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    let (reserved, modifiers) = if cfg!(target_os = "macos") { (Key::Tab, Modifiers::COMMAND) } else { (Key::Tab, Modifiers::ALT) };
    key(&mut h, reserved, modifiers);
    assert_eq!(field(&h, id, "capture"), json!(false));
    assert_eq!(field(&h, id, "messageWarning"), json!(true));
    assert!(field(&h, id, "message").as_str().unwrap().contains("reserved by the system"));
}

// ------------------------------------------------------------------ rows

#[test]
fn the_top_row_is_a_full_width_search_with_add_shortcut_at_its_right() {
    let (h, _) = on("edit.keyboardShortcuts", "file.new");
    let add = h.get_by_label("Add Shortcut").rect();
    let search = h.get_by_label("Search").rect();
    assert!(search.right() <= add.left(), "the search is left of the button");
    assert!((search.center().y - add.center().y).abs() < 3.0, "one row: {search:?} {add:?}");
    let ok = h.get_by_label("OK").rect();
    assert!(add.right() > ok.right() - 40.0, "the button sits at the window's right edge: {} against {}", add.right(), ok.right());
    assert!(search.width() > 500.0, "the search is wide: {}", search.width());
}

#[test]
fn the_removed_buttons_and_texts_are_gone() {
    let mut h = harness(egui::vec2(1440.0, 900.0), |app| {
        app.session.edit_prefs(|p| _ = p.shortcuts.insert("file.new".into(), "Cmd+Alt+N".into()));
    });
    let id = open(&mut h, "edit.keyboardShortcuts", &[("selected", json!("file.new"))]);
    for gone in ["Delete Shortcut", "Use Default", "Import Shortcuts…", "modified", "Add Chord"] {
        assert!(h.query_by_label(gone).is_none(), "{gone} is gone");
    }
    // Export and import icons stay at the top.
    h.get_by_label("Export Shortcuts as JSON…");
    h.get_by_label("Import Shortcuts from JSON…");
    // Recording and accepting leaves no "assigned." status line.
    h.get_by_label(&crate::shortcuts::pretty("Cmd+Alt+N")).click();
    h.run_steps(2);
    key(&mut h, Key::U, Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT);
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!("Cmd+Alt+Shift+U"));
    assert!(h.query_by_label_contains("assigned").is_none());
    assert_eq!(field(&h, id, "message"), json!(""));
}

#[test]
fn a_changed_row_has_a_reset_icon_that_says_what_it_goes_back_to() {
    let mut h = harness(egui::vec2(1440.0, 900.0), |app| {
        app.session.edit_prefs(|p| _ = p.shortcuts.insert("file.new".into(), "Cmd+Alt+N".into()));
    });
    let id = open(&mut h, "edit.keyboardShortcuts", &[("selected", json!("file.new"))]);
    let tip = format!("Reset to {}", crate::shortcuts::pretty("Cmd+N"));
    assert_eq!(h.query_all_by_label_contains("Reset to ").count(), 1, "one changed row, one icon (and Reset All to Defaults is another label)");
    h.get_by_label(&tip).click();
    h.run_steps(3);
    assert!(field(&h, id, "overrides").get("file.new").is_none(), "the row is back to its default");
    assert!(h.query_by_label(&tip).is_none(), "an unchanged row has no icon");
}

#[test]
fn the_reset_icon_of_a_command_without_a_default_says_none() {
    let mut h = harness(egui::vec2(1440.0, 900.0), |app| {
        app.session.edit_prefs(|p| _ = p.shortcuts.insert("file.newFromClipboard".into(), "Cmd+Alt+J".into()));
    });
    open(&mut h, "edit.keyboardShortcuts", &[("selected", json!("file.newFromClipboard"))]);
    h.get_by_label("Reset to none");
}

#[test]
fn remove_shortcut_is_on_the_right_click_menu_of_a_row() {
    let (mut h, id) = on("edit.keyboardShortcuts", "");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click_secondary();
    h.run_steps(3);
    h.get_by_label("Remove Shortcut").click();
    h.run_steps(3);
    assert_eq!(field(&h, id, "overrides")["file.new"], json!(""));
}

#[test]
fn delete_and_backspace_remove_the_selected_rows_shortcut() {
    for removing in [Key::Delete, Key::Backspace] {
        let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
        key(&mut h, removing, Modifiers::NONE);
        assert_eq!(field(&h, id, "overrides")["file.new"], json!(""), "{removing:?}");
    }
}

#[test]
fn delete_while_typing_in_the_search_removes_nothing() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label("Search").click();
    h.run_steps(2);
    key(&mut h, Key::Backspace, Modifiers::NONE);
    assert!(field(&h, id, "overrides").get("file.new").is_none());
}

#[test]
fn the_open_recent_files_are_not_listed_but_clear_recent_files_is() {
    let mut h = harness(egui::vec2(1440.0, 900.0), |app| {
        app.ui.recent_files = ["sample-1.png", "sample-2.png"].map(String::from).to_vec();
    });
    for command in ["edit.keyboardShortcuts", "edit.menus"] {
        open(&mut h, command, &[]);
        assert!(h.query_by_label_contains("sample-1.png").is_none(), "{command}: no recent file");
        assert!(h.query_by_label_contains("sample-2.png").is_none());
        assert!(h.query_all_by_label_contains("Clear Recent Files").count() >= 1, "{command}: Clear Recent Files stays");
    }
}

#[test]
fn no_dynamic_item_is_a_command_of_the_window() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.ui.recent_files = ["sample-1.png".to_string()].to_vec();
    app.run("file.new", json!({"width": 64, "height": 48})).unwrap();
    app.sync_views();
    let ids: Vec<String> = crate::prefs_ui::shortcut_items(&app).into_iter().map(|i| i.0).collect();
    assert!(!ids.iter().any(|i| i.starts_with("file.openRecent.") || i.starts_with("window.document.") || i.starts_with("window.workspace.saved.")), "{ids:?}");
    assert!(ids.iter().any(|i| i == "file.clearRecent"));
    assert!(ids.iter().any(|i| i == "file.open"));
}

// ------------------------------------------------------------------ search

#[test]
fn the_search_understands_modifier_words_and_symbols() {
    let (mut h, id) = on("edit.keyboardShortcuts", "");
    let listed = |h: &mut Harness<'_, PhotocraftApp>, q: &str| {
        set_field(h, id, "filter", json!(q));
        h.run_steps(4);
        (
            h.query_all_by_label_contains("Save As").count() > 0,
            h.query_all_by_label_contains("New from Clipboard").count() > 0,
            h.query_all_by_label_contains("Close All").count() > 0,
        )
    };
    // Save As is ⌘⇧S, Close All is ⌘⌥W, New from Clipboard has no modifier at all.
    assert_eq!(listed(&mut h, "cmd shift"), (true, false, false), "both modifiers");
    assert_eq!(listed(&mut h, "command"), (true, false, true));
    assert_eq!(listed(&mut h, "shift"), (true, false, false));
    assert_eq!(listed(&mut h, "option"), (false, false, true));
    assert_eq!(listed(&mut h, "alt"), (false, false, true));
    assert_eq!(listed(&mut h, "⌘⇧"), (true, false, false));
    assert_eq!(listed(&mut h, "⌘⌥"), (false, false, true));
    assert_eq!(listed(&mut h, "save"), (true, false, false), "plain words still match command names");
}

// ------------------------------------------------------------------ recording and typing

#[test]
fn clicking_into_the_search_ends_the_recording() {
    let (mut h, id) = on("edit.keyboardShortcuts", "file.new");
    h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click();
    h.run_steps(2);
    assert_eq!(field(&h, id, "capture"), json!(true));
    h.get_by_label("Search").click();
    h.run_steps(3);
    assert_eq!(field(&h, id, "capture"), json!(false), "typing in the search is not recording");
    key(&mut h, Key::J, Modifiers::COMMAND);
    assert_eq!(field(&h, id, "__recSteps"), json!([]));
}

// ------------------------------------------------------------------ Menus tab

#[test]
fn a_menu_row_has_a_visible_box_and_a_colour_dot_and_no_text_controls() {
    let (h, _) = on("edit.menus", "file.new");
    assert!(h.query_all_by_label("none").next().is_none(), "no text dropdown any more");
    assert!(h.query_all_by_label_contains("Color: None").count() > 10, "a colour dot in every row");
    assert!(h.query_all_by_role_and_label(egui::accesskit::Role::CheckBox, "Visible").count() > 10, "a Visible box in every row");
    // Visible and Color are column headers, once per section.
    assert!(h.query_all_by_label("Visible").count() > 10);
}

#[test]
fn the_menu_controls_are_pinned_right_in_columns() {
    let (h, _) = on("edit.menus", "");
    let dots: Vec<egui::Rect> = h.query_all_by_label_contains("Color: None").take(8).map(|n| n.rect()).collect();
    let boxes: Vec<egui::Rect> = h.query_all_by_role_and_label(egui::accesskit::Role::CheckBox, "Visible").take(8).map(|n| n.rect()).collect();
    assert!(dots.windows(2).all(|w| (w[0].center().x - w[1].center().x).abs() < 0.5), "one column of dots: {dots:?}");
    assert!(boxes.windows(2).all(|w| (w[0].center().x - w[1].center().x).abs() < 0.5), "one column of boxes");
    let cancel = h.get_by_label("Cancel").rect();
    assert!(dots[0].right() > cancel.right() - 60.0, "the dots sit at the right edge: {} against {}", dots[0].right(), cancel.right());
    assert!(boxes[0].right() < dots[0].left(), "Visible comes before Color");
    let name = h.get_all_by_label_contains("New…").next().unwrap().rect();
    assert!(name.left() < 260.0 && boxes[0].left() > 700.0, "the name is at the left, the controls far right");
}

#[test]
fn the_colour_dot_opens_a_popover_of_dots_and_sets_the_colour() {
    let (mut h, id) = on("edit.menus", "");
    h.query_all_by_label_contains("Color: None").next().unwrap().click();
    h.run_steps(3);
    for name in ["None", "Red", "Orange", "Yellow", "Green", "Blue", "Violet", "Gray"] {
        assert!(h.query_all_by_label(name).count() >= 1, "{name} dot in the popover");
    }
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Blue").click();
    h.run_steps(3);
    assert_eq!(field(&h, id, "colors")["file.new"], json!("blue"));
    assert!(h.query_all_by_label_contains("Color: Blue").count() == 1, "the dot now names its colour");
    // Back to none removes it.
    h.get_by_label("Color: Blue").click();
    h.run_steps(3);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "None").click();
    h.run_steps(3);
    assert_eq!(field(&h, id, "colors"), json!({}));
}

#[test]
fn the_visible_box_hides_an_item_and_always_visible_items_are_inert() {
    let (mut h, id) = on("edit.menus", "");
    let clipboard = h.query_all_by_role_and_label(egui::accesskit::Role::CheckBox, "Visible").nth(1).unwrap();
    clipboard.click();
    h.run_steps(3);
    assert_eq!(field(&h, id, "hidden"), json!(["file.newFromClipboard"]));
    h.get_by_label("Show All Menu Items").click();
    h.run_steps(3);
    assert_eq!(field(&h, id, "hidden"), json!([]));
}
