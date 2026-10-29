//! Keyboard Shortcuts: chords, When conditions and the JSON backup in the dialog.

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::PhotocraftApp;

fn key(h: &mut Harness<'_, PhotocraftApp>, key: Key, pressed: bool, modifiers: Modifiers) {
    h.event(Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
}

/// The Keyboard Shortcuts dialog with `command` selected and the list filtered to `filter`.
fn dialog(command: &str, filter: &str) -> (Harness<'static, PhotocraftApp>, u64) {
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
    });
    let ctx = h.ctx.clone();
    let id = crate::menus::invoke(h.state_mut(), &ctx, "edit.keyboardShortcuts", json!({})).unwrap()["dialog"].as_u64().unwrap();
    let f = &mut h.state_mut().ui.dialog_mut(id).unwrap().fields;
    f.insert("filter".into(), json!(filter));
    f.insert("selected".into(), json!(command));
    h.run_steps(4);
    (h, id)
}

fn field(h: &Harness<'_, PhotocraftApp>, id: u64, name: &str) -> Value {
    h.state().ui.dialogs.iter().find(|d| d.id == id).unwrap().fields.get(name).cloned().unwrap_or(Value::Null)
}

fn set_field(h: &mut Harness<'_, PhotocraftApp>, id: u64, name: &str, v: Value) {
    h.state_mut().ui.dialog_mut(id).unwrap().fields.insert(name.into(), v);
}

/// Press `first` then `second` (with `held` down for the first), one frame apart, like a keyboard.
fn chord(h: &mut Harness<'_, PhotocraftApp>, first: (Key, Modifiers), second: Key) {
    key(h, first.0, true, first.1);
    h.run_steps(2);
    key(h, first.0, false, first.1);
    h.run_steps(1);
    key(h, second, true, Modifiers::NONE);
    h.run_steps(2);
    key(h, second, false, Modifiers::NONE);
    h.run_steps(1);
}

#[test]
fn the_dialog_opens_with_a_when_column_and_no_chords() {
    let (h, id) = dialog("edit.copy", "Copy");
    assert_eq!(field(&h, id, "chords"), json!({}));
    assert_eq!(field(&h, id, "when"), json!({}));
    h.get_by_label("Condition of Copy");
    h.get_by_label("Add Chord");
    assert!(h.get_all_by_label("Add Condition").count() > 1, "one + in every row");
    h.get_by_label("Export Shortcuts as JSON…");
    h.get_by_label("Import Shortcuts from JSON…");
}

#[test]
fn the_dialog_shows_saved_chords_and_conditions() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.session.edit_prefs(|p| {
        p.chords.insert("edit.copy".into(), "Cmd+K C".into());
        p.shortcut_when.insert("edit.copy".into(), "!textFocus && tool == brush".into());
    });
    let id = crate::prefs_ui::open_shortcuts(&mut app, 0);
    let f = &app.ui.dialog_mut(id).unwrap().fields;
    assert_eq!(f["chords"]["edit.copy"], "Cmd+K C");
    assert_eq!(f["when"]["edit.copy"], "!textFocus && tool == brush");
}

#[test]
fn a_chord_is_captured_from_two_key_presses_and_saved_with_ok() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    h.get_by_label("Add Chord").click();
    h.run_steps(2);
    assert_eq!(field(&h, id, "captureChord"), json!(true));
    chord(&mut h, (Key::K, Modifiers::COMMAND), Key::J);
    assert_eq!(field(&h, id, "chords")["edit.copy"], json!("Cmd+K J"));
    assert_eq!(field(&h, id, "captureChord"), json!(false));
    assert_eq!(field(&h, id, "overrides"), json!({}), "the chord is not the command's main key");
    assert!(h.get_all_by_label_contains(&crate::keybinds::chord::text("Cmd+K J")).count() >= 1, "the chord shows as a chip and in the message");
    crate::dialogs::confirm(h.state_mut(), id).unwrap();
    assert_eq!(h.state().session.prefs().chords["edit.copy"], "Cmd+K J");
}

#[test]
fn escape_cancels_a_chord_and_a_reserved_key_is_refused() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    h.get_by_label("Add Chord").click();
    h.run_steps(2);
    key(&mut h, Key::K, true, Modifiers::COMMAND);
    h.run_steps(2);
    assert_eq!(field(&h, id, "chordFirst"), json!("Cmd+K"));
    key(&mut h, Key::Escape, true, Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(field(&h, id, "captureChord"), json!(false));
    assert_eq!(field(&h, id, "chords"), json!({}));
    // a step the system takes
    h.get_by_label("Add Chord").click();
    h.run_steps(2);
    let reserved = if cfg!(target_os = "macos") { (Key::Tab, Modifiers::COMMAND) } else { (Key::Tab, Modifiers::ALT) };
    chord(&mut h, reserved, Key::J);
    assert_eq!(field(&h, id, "chords"), json!({}));
    assert_eq!(field(&h, id, "messageWarning"), json!(true));
}

#[test]
fn a_chord_already_in_use_is_taken_when_ok_is_clicked() {
    let (mut h, id) = dialog("edit.cut", "Cut");
    set_field(&mut h, id, "chords", json!({"edit.copy": "Cmd+K J"}));
    h.run_steps(2);
    h.get_by_label("Add Chord").click();
    h.run_steps(2);
    chord(&mut h, (Key::K, Modifiers::COMMAND), Key::J);
    assert_eq!(field(&h, id, "chords")["edit.cut"], json!("Cmd+K J"));
    assert!(field(&h, id, "message").as_str().unwrap().contains("already in use"), "{}", field(&h, id, "message"));
    assert_eq!(field(&h, id, "chordConflict")["holders"], json!(["edit.copy"]));
    h.get_by_label("Accept and Go to Conflict");
    h.get_by_label("Undo Changes");
    crate::dialogs::confirm(h.state_mut(), id).unwrap();
    let chords = &h.state().session.prefs().chords;
    assert_eq!(chords.get("edit.cut").map(String::as_str), Some("Cmd+K J"));
    assert!(!chords.contains_key("edit.copy"), "the old owner lost it: {chords:?}");
}

#[test]
fn undo_puts_the_commands_old_chord_back() {
    let (mut h, id) = dialog("edit.cut", "Cut");
    set_field(&mut h, id, "chords", json!({"edit.copy": "Cmd+K J", "edit.cut": "Cmd+K X"}));
    h.run_steps(2);
    h.get_by_label("Add Chord").click();
    h.run_steps(2);
    chord(&mut h, (Key::K, Modifiers::COMMAND), Key::J);
    h.get_by_label("Undo Changes").click();
    h.run_steps(2);
    assert_eq!(field(&h, id, "chords"), json!({"edit.copy": "Cmd+K J", "edit.cut": "Cmd+K X"}));
    assert_eq!(field(&h, id, "chordConflict"), Value::Null);
}

#[test]
fn a_chord_is_deleted_with_its_cross_and_use_default() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    set_field(&mut h, id, "chords", json!({"edit.copy": "Cmd+K J"}));
    set_field(&mut h, id, "when", json!({"edit.copy": "hasDocument"}));
    h.run_steps(2);
    h.get_by_label("Use Default").click();
    h.run_steps(2);
    assert_eq!(field(&h, id, "chords"), json!({}));
    assert_eq!(field(&h, id, "when"), json!({}));
}

#[test]
fn a_condition_is_typed_with_autocomplete_and_saved_on_return() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    h.get_by_label("Condition of Copy").click();
    h.run_steps(3);
    let edit = field(&h, id, "whenEdit");
    assert_eq!(edit["id"], "edit.copy");
    for n in 0..6 {
        eprintln!("step {n} focus {} focused {:?}", h.ctx.memory(|m| m.has_focus(egui::Id::unique("pc-shortcut-when-field"))), h.ctx.memory(|m| m.focused()));
        eprintln!("inputs {}", h.get_all_by_role(egui::accesskit::Role::TextInput).count());
        h.run_steps(1);
    }
    h.get_by_label("tool");
    set_field(&mut h, id, "whenEdit", json!({"id": "edit.copy", "text": "tool==brush&&!dialogOpen", "fresh": true}));
    h.run_steps(3);
    key(&mut h, Key::Enter, true, Modifiers::NONE);
    h.run_steps(3);
    assert_eq!(field(&h, id, "when")["edit.copy"], json!("tool == brush && !dialogOpen"));
    assert_eq!(field(&h, id, "whenEdit"), Value::Null);
    crate::dialogs::confirm(h.state_mut(), id).unwrap();
    assert_eq!(h.state().session.prefs().shortcut_when["edit.copy"], "tool == brush && !dialogOpen");
}

#[test]
fn a_bad_condition_is_not_saved_and_says_why() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    set_field(&mut h, id, "whenEdit", json!({"id": "edit.copy", "text": "tol == brush", "fresh": true}));
    h.run_steps(3);
    h.get_by_label_contains("Unknown key");
    key(&mut h, Key::Enter, true, Modifiers::NONE);
    h.run_steps(3);
    assert_eq!(field(&h, id, "when"), json!({}), "nothing invalid is kept");
    assert!(field(&h, id, "whenEdit").is_object(), "the box stays open");
}

#[test]
fn the_builder_adds_a_part_to_a_condition() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    h.get_all_by_label("Add Condition").next().unwrap().click();
    h.run_steps(2);
    h.get_all_by_label("Typing").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("Is Not").click();
    h.run_steps(2);
    h.get_all_by_label("Typing").last().unwrap().click();
    h.run_steps(2);
    // the first row's + was used: one command now has the part
    let when = field(&h, id, "when");
    assert_eq!(when.as_object().map(|m| m.len()), Some(1), "{when}");
    assert_eq!(when.as_object().and_then(|m| m.values().next()), Some(&json!("!textFocus")));
    // a second part joins the first with &&
    h.get_all_by_label("Add Condition").next().unwrap().click();
    h.run_steps(2);
    h.get_all_by_label("Document").next().unwrap().click();
    h.run_steps(2);
    h.get_all_by_label("Document Open").last().unwrap().click();
    h.run_steps(2);
    let first = field(&h, id, "when").as_object().and_then(|m| m.values().next().cloned()).unwrap();
    assert_eq!(first, json!("!textFocus && hasDocument"));
}

#[test]
fn a_key_may_be_shared_when_the_conditions_exclude_each_other() {
    let (mut h, id) = dialog("edit.cut", "Cut");
    // Copy has F9; Cut takes F9 too, and the two differ by When
    set_field(&mut h, id, "overrides", json!({"edit.copy": "F9"}));
    set_field(&mut h, id, "when", json!({"edit.copy": "tool == brush", "edit.cut": "tool != brush"}));
    h.run_steps(2);
    h.get_by_label(&crate::shortcuts::pretty("Cmd+X")).click();
    h.run_steps(2);
    key(&mut h, Key::F9, true, Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(field(&h, id, "overrides")["edit.cut"], json!("F9"));
    assert_eq!(field(&h, id, "conflict"), Value::Null, "no clash: they never hold together");
    crate::dialogs::confirm(h.state_mut(), id).unwrap();
    let p = h.state().session.prefs();
    assert_eq!(p.shortcuts.get("edit.copy").map(String::as_str), Some("F9"), "Copy keeps F9");
    assert_eq!(p.shortcuts.get("edit.cut").map(String::as_str), Some("F9"));
}

#[test]
fn the_same_key_clashes_when_the_conditions_overlap() {
    let (mut h, id) = dialog("edit.cut", "Cut");
    set_field(&mut h, id, "overrides", json!({"edit.copy": "F9"}));
    set_field(&mut h, id, "when", json!({"edit.copy": "tool == brush", "edit.cut": "hasDocument"}));
    h.run_steps(2);
    h.get_by_label(&crate::shortcuts::pretty("Cmd+X")).click();
    h.run_steps(2);
    key(&mut h, Key::F9, true, Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(field(&h, id, "conflict")["with"], json!("edit.copy"));
}

#[test]
fn a_backup_round_trips_through_the_dialog() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    set_field(&mut h, id, "overrides", json!({"edit.copy": "Cmd+J"}));
    set_field(&mut h, id, "chords", json!({"edit.cut": "Cmd+K X"}));
    set_field(&mut h, id, "when", json!({"edit.cut": "hasSelection"}));
    let f = h.state().ui.dialogs.iter().find(|d| d.id == id).unwrap().fields.clone();
    let text = serde_json::to_string(&photocraft_engine::shortcut_backup::to_json(&super::shortcuts_backup::staged(&f))).unwrap();
    set_field(&mut h, id, "overrides", json!({}));
    set_field(&mut h, id, "chords", json!({}));
    set_field(&mut h, id, "when", json!({}));
    let r = super::shortcuts_backup::import_bytes(h.state_mut(), "keys.json", text.as_bytes()).unwrap();
    assert_eq!(r["imported"], 2);
    assert_eq!(field(&h, id, "overrides"), json!({"edit.copy": "Cmd+J"}));
    assert_eq!(field(&h, id, "chords"), json!({"edit.cut": "Cmd+K X"}));
    assert_eq!(field(&h, id, "when"), json!({"edit.cut": "hasSelection"}));
    assert!(field(&h, id, "message").as_str().unwrap().contains("Imported 2 commands from keys.json"));
    // nothing is applied until OK
    assert!(h.state().session.prefs().chords.is_empty());
}

#[test]
fn a_wrong_newer_or_broken_file_is_refused_and_changes_nothing() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    set_field(&mut h, id, "chords", json!({"edit.cut": "Cmd+K X"}));
    for (name, bytes, why) in [
        ("keys.json", b"{broken".to_vec(), "isn't readable JSON"),
        ("keys.json", br#"{"kind": "lightcraft-shortcuts", "version": 1, "shortcuts": {}}"#.to_vec(), "isn't a PhotoCraft shortcuts file"),
        ("keys.json", br#"{"kind": "photocraft-shortcuts", "version": 99, "shortcuts": {}}"#.to_vec(), "newer PhotoCraft"),
        ("keys.json", vec![0xff, 0xfe, 0x00], "isn't text"),
        ("keys.json", vec![b' '; photocraft_engine::shortcut_backup::MAX_BYTES + 1], "too big"),
    ] {
        let e = super::shortcuts_backup::import_bytes(h.state_mut(), name, &bytes).unwrap_err();
        assert!(e.contains(why), "{e}");
        assert_eq!(field(&h, id, "chords"), json!({"edit.cut": "Cmd+K X"}), "{why}");
        assert_eq!(field(&h, id, "messageWarning"), json!(true));
    }
}

#[test]
fn unknown_ids_in_a_backup_are_skipped_with_a_note() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    let text = json!({"kind": "photocraft-shortcuts", "version": 1, "shortcuts": {
        "no.such.command": {"shortcut": "Cmd+J"},
        "edit.copy": {"chord": "Cmd+K C"},
        "edit.cut": {"chord": "Cmd+K Nope"},
    }})
    .to_string();
    let r = super::shortcuts_backup::import_bytes(h.state_mut(), "keys.json", text.as_bytes()).unwrap();
    assert_eq!(r["imported"], 1);
    assert_eq!(r["notices"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(field(&h, id, "chords"), json!({"edit.copy": "Cmd+K C"}));
    assert_eq!(field(&h, id, "messageWarning"), json!(true));
}

#[test]
fn a_dialog_made_by_an_older_build_still_works() {
    let (mut h, id) = dialog("edit.copy", "Copy");
    for name in ["chords", "when", "whenEdit", "captureChord", "chordFirst", "chordConflict"] {
        h.state_mut().ui.dialog_mut(id).unwrap().fields.remove(name);
    }
    h.run_steps(3);
    assert_eq!(field(&h, id, "chords"), json!({}));
    crate::dialogs::confirm(h.state_mut(), id).unwrap();
}

#[test]
fn a_clash_gets_a_warning_note_only_when_the_conditions_can_hold_together() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let items = crate::prefs_ui::shortcut_items(&app);
    let overrides: std::collections::BTreeMap<String, String> = [("edit.copy", "F9"), ("edit.cut", "F9")].map(|(a, b)| (a.to_string(), b.to_string())).into();
    let alts = Default::default();
    let notes = |chords: &std::collections::BTreeMap<String, String>, whens: &std::collections::BTreeMap<String, String>| {
        super::shortcuts_when::clash_notes(&items, (&overrides, &alts), chords, whens)
    };
    let none = Default::default();
    assert!(notes(&none, &none).is_empty(), "no condition or chord, no note: a shared key is not this feature's business");
    let mut whens: std::collections::BTreeMap<String, String> = [("edit.copy", "tool == brush")].map(|(a, b)| (a.to_string(), b.to_string())).into();
    let n = notes(&none, &whens);
    assert!(n["edit.copy"].contains("also used by Cut") && n["edit.cut"].contains("also used by Copy"), "{n:?}");
    whens.insert("edit.cut".into(), "tool != brush".into());
    assert!(notes(&none, &whens).is_empty(), "they never hold together");
    // a chord whose first key is another command's key
    let chords: std::collections::BTreeMap<String, String> = [("edit.paste", "F9 V")].map(|(a, b)| (a.to_string(), b.to_string())).into();
    let n = notes(&chords, &none);
    assert!(n["edit.paste"].contains("waits for the chord's second key"), "{n:?}");
}

#[test]
fn the_when_header_says_it_is_for_keys_only_and_suggestions_are_short() {
    let (h, _) = dialog("edit.copy", "Copy");
    h.get_all_by_label("When").next().expect("a When header");
    assert_eq!(super::shortcuts_when::short_for_test("A dialog is open (shortcuts normally wait for it)"), "A dialog is open");
    assert!(super::shortcuts_when::short_for_test(&"x".repeat(80)).chars().count() <= 36);
}

/// Opt-in pictures for review (`PHOTOCRAFT_SHORTCUT_SNAPSHOTS` names the folder): the dialog with
/// a chord, conditions and the JSON icons; the When autocomplete open; the keycap hint of a chord
/// waiting on its second key.
#[test]
fn shortcut_upgrade_snapshots() {
    let Some(directory) = std::env::var_os("PHOTOCRAFT_SHORTCUT_SNAPSHOTS") else {
        eprintln!("set PHOTOCRAFT_SHORTCUT_SNAPSHOTS to capture the shortcut dialog, the When autocomplete and the chord hint");
        return;
    };
    let _gpu = photocraft_testkit::gpu_lock();
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let build = || {
        Harness::builder().with_size(egui::vec2(1100.0, 700.0)).with_pixels_per_point(2.0).with_max_steps(64).wgpu().build_eframe(|cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
            app.session.edit_prefs(|p| {
                p.chords.insert("edit.copy".into(), "Cmd+K C".into());
                p.chords.insert("edit.copyMerged".into(), "Cmd+K M".into());
                p.chords.insert("tools.select.brush".into(), "Cmd+K B".into());
                p.shortcut_when.insert("edit.copy".into(), "!textFocus && tool == brush".into());
                p.shortcut_when.insert("edit.copyMerged".into(), "hasDocument && !(layerKind == group)".into());
                p.shortcuts.insert("edit.copyMerged".into(), "Cmd+Alt+C".into());
            });
            app
        })
    };
    let mut h = build();
    let ctx = h.ctx.clone();
    let id = crate::menus::invoke(h.state_mut(), &ctx, "edit.keyboardShortcuts", json!({})).unwrap()["dialog"].as_u64().unwrap();
    set_field(&mut h, id, "filter", json!("Copy"));
    set_field(&mut h, id, "selected", json!("edit.copy"));
    h.run_steps(6);
    h.render().expect("render the dialog").save(directory.join("after-dialog.png")).unwrap();
    set_field(&mut h, id, "whenEdit", json!({"id": "edit.copy", "text": "tool == brush && !d", "fresh": true}));
    h.run_steps(6);
    h.render().expect("render the autocomplete").save(directory.join("after-when-autocomplete.png")).unwrap();
    let mut h = build();
    h.run_steps(3);
    let [first, _] = crate::keybinds::chord::steps("Cmd+K B").unwrap();
    let now = h.ctx.input(|i| i.time);
    h.state_mut().keybinds.pending = Some(crate::keybinds::runtime::Pending { first, since: now });
    h.run_steps(4);
    h.render().expect("render the chord hint").save(directory.join("after-chord-hint.png")).unwrap();
}

/// The right edge of the dialog's button row: it follows the window's width.
fn right_edge(h: &Harness<'_, PhotocraftApp>) -> f32 {
    ["OK", "Cancel"].iter().map(|l| h.get_by_label(l).rect().right()).fold(f32::MIN, f32::max)
}

#[test]
fn the_window_keeps_its_width_while_searching_and_clearing() {
    let (mut h, id) = dialog("edit.copy", "");
    h.run_steps(4);
    let full = right_edge(&h);
    for query in ["Copy", "", "zzz-no-such-command", ""] {
        set_field(&mut h, id, "filter", json!(query));
        // Every frame after the change, not only the settled one: the jump lasts a frame or two.
        for _ in 0..4 {
            h.step();
            let now = right_edge(&h);
            assert!((now - full).abs() < 1.0, "query {query:?}: the window's right edge is {now}, steady is {full}");
        }
    }
}
