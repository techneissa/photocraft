//! Export Settings… and Import Settings… in the shell: the icons, the checks only the shell can
//! make, the prompt before an import, and the open Preferences window following the import.

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use super::settings_sync::{self, EXPORT, IMPORT};
use super::*;
use crate::PhotocraftApp;
use crate::file_dialog::FileDialogAnswer;

type Written = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// An app whose file dialogs answer `open` (a file's bytes) or `save`, and whose writes are recorded.
fn app_with_dialogs(answers: Vec<Option<FileDialogAnswer>>) -> (PhotocraftApp, Written) {
    let written: Written = Rc::default();
    let log = written.clone();
    let (picker, _) = crate::file_dialog::fake(answers);
    let services = crate::Services {
        file_dialog: Some(picker),
        write: Some(Box::new(move |path: &str, bytes: &[u8]| {
            log.borrow_mut().push((path.to_string(), bytes.to_vec()));
            Ok(())
        })),
        ..Default::default()
    };
    (PhotocraftApp::new(photocraft_engine::Session::new(), services), written)
}

/// The settings file of an app whose JPEG quality is `quality`.
fn file_with(quality: u32) -> String {
    let mut other = super::tests::app_with_store().0;
    other.run("prefs.set", json!({"path": "export.jpegQuality", "value": quality})).unwrap();
    other.session.settings_text()
}

fn prompt(app: &PhotocraftApp) -> Option<(u64, serde_json::Map<String, Value>)> {
    app.ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(Value::as_str) == Some("settingsImport")).map(|d| (d.id, d.fields.clone()))
}

fn message(app: &PhotocraftApp) -> Value {
    app.ui.dialogs.iter().find(|d| is_preferences(&d.fields)).and_then(|d| d.fields.get("__syncMessage").cloned()).unwrap_or(Value::Null)
}

#[test]
fn the_shell_checks_keys_tools_and_context_menus() {
    let c = settings_sync::checks();
    assert!((c.key_ok)("Cmd+K") && !(c.key_ok)("Banana+Q"));
    assert!((c.tool_ok)("Brush") && !(c.tool_ok)("NoSuchTool"));
    let (kept, left) = (c.context_menus)(&json!({"menus": {"no.such.menu": {"items": ["---"], "hidden": []}}})).unwrap();
    assert!(left.iter().any(|s| s.contains("no.such.menu")), "{left:?}");
    assert!(kept.get("menus").is_none_or(|m| m.get("no.such.menu").is_none()));
    assert_eq!((c.context_menus)(&Value::Null).unwrap().0, Value::Null);
    assert!((c.context_menus)(&json!(5)).is_err());
}

#[test]
fn an_import_asks_first_and_changes_nothing_until_it_is_answered() {
    let (mut app, store) = super::tests::app_with_store();
    settings_sync::import_bytes(&mut app, "/Users/a/Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    let (_, f) = prompt(&app).expect("the prompt");
    assert_eq!(f["file"], "Mine.pcsettings", "the file's name, not its folder");
    assert!(f["parts"].as_array().unwrap().iter().any(|p| p == "preferences"));
    assert_eq!(app.session.prefs().export.jpeg_quality, 85, "nothing changed yet");
    confirm(&mut app, &f).unwrap();
    assert_eq!(app.session.prefs().export.jpeg_quality, 33);
    assert_eq!(store.lock().unwrap().as_deref().map(|t| serde_json::from_str::<Value>(t).unwrap()["export"]["jpegQuality"].clone()), Some(json!(33)), "saved");
}

#[test]
fn cancelling_the_prompt_leaves_everything_as_it_was() {
    let (mut app, _) = super::tests::app_with_store();
    settings_sync::import_bytes(&mut app, "Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    let (id, _) = prompt(&app).unwrap();
    crate::dialogs::cancel(&mut app, id).unwrap();
    assert!(prompt(&app).is_none());
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
}

#[test]
fn a_refused_file_says_why_and_opens_no_prompt() {
    let (mut app, _) = super::tests::app_with_store();
    open_preferences(&mut app, "general");
    for bytes in [&b"not json"[..], &[0xff, 0xfe, 0x00][..], br#"{"kind":"photocraft-settings","version":1,"preferences":{"export":{"jpegQuality":0}}}"#] {
        let e = settings_sync::import_bytes(&mut app, "Bad.pcsettings", bytes).unwrap_err();
        settings_sync::report(&mut app, &e, true);
        assert!(prompt(&app).is_none(), "no prompt for a refused file");
        assert_eq!(message(&app)["warning"], true);
        assert_eq!(message(&app)["text"], e.as_str());
    }
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
}

#[test]
fn an_import_brings_the_open_preferences_window_and_other_dialogs_in_line() {
    let (mut app, _) = super::tests::app_with_store();
    let id = open_preferences(&mut app, "general");
    let shortcuts = open_shortcuts(&mut app, 0);
    // An unsaved edit in the window.
    app.ui.dialog_mut(id).unwrap().fields.get_mut("values").unwrap()["cursors"]["painting"] = json!("fullSizeTip");
    assert!(preferences_changed(&app, &app.ui.dialogs.iter().find(|d| d.id == id).unwrap().fields));
    settings_sync::import_bytes(&mut app, "Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    let (_, f) = prompt(&app).unwrap();
    confirm(&mut app, &f).unwrap();
    let fields = &app.ui.dialogs.iter().find(|d| d.id == id).unwrap().fields;
    assert_eq!(fields["values"]["export"]["jpegQuality"], 33, "the window shows the imported value");
    assert_eq!(fields["values"]["cursors"]["painting"], json!(app.session.prefs().cursors.painting.name()), "and not the discarded edit");
    assert!(!preferences_changed(&app, fields), "nothing left to save in the window");
    assert!(app.ui.dialogs.iter().all(|d| d.id != shortcuts), "a Keyboard Shortcuts window would put its old keys back");
    assert_eq!(message(&app)["warning"], false);
}

#[test]
fn the_export_writes_what_the_preferences_window_shows() {
    let (mut app, _) = super::tests::app_with_store();
    let id = open_preferences(&mut app, "general");
    app.ui.dialog_mut(id).unwrap().fields.get_mut("values").unwrap()["export"]["jpegQuality"] = json!(44);
    let (prefs, color) = settings_sync::settings_now(&app);
    assert_eq!(prefs.export.jpeg_quality, 44, "the window's value, not the saved 85");
    let text = photocraft_engine::prefs::settings_sync::to_text(&prefs, &color);
    assert!(text.contains("\"jpegQuality\": 44"));
    assert_eq!(app.session.prefs().export.jpeg_quality, 85, "exporting saves nothing");
}

#[test]
fn the_icons_ask_for_a_file_and_write_it() {
    let (mut app, written) = app_with_dialogs(vec![Some(FileDialogAnswer::SaveTo("Mine.pcsettings".into()))]);
    app.run("prefs.set", json!({"path": "export.jpegQuality", "value": 55})).unwrap();
    let (mut h, _) = {
        let id = open_preferences(&mut app, "general");
        (
            Harness::builder().with_size(egui::vec2(1280.0, 860.0)).with_max_steps(64).build_eframe(move |cc| {
                PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
                app
            }),
            id,
        )
    };
    h.run_steps(4);
    h.get_by_label("Import Settings…");
    h.get_by_label("Export Settings…").click();
    h.run_steps(6);
    let written = written.borrow();
    let (path, bytes) = written.first().expect("the file was written");
    assert!(path.ends_with("Mine.pcsettings"), "{path}");
    let file: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!((file["kind"].as_str(), file["version"].as_u64()), (Some("photocraft-settings"), Some(1)));
    assert_eq!(file["preferences"]["export"]["jpegQuality"], 55);
    assert!(h.state().ui.dialogs.iter().filter(|d| is_preferences(&d.fields)).count() == 1);
}

#[test]
fn the_import_icon_reads_a_file_and_shows_the_prompt() {
    let bytes = file_with(33).into_bytes();
    let (mut app, _) = app_with_dialogs(vec![Some(FileDialogAnswer::Contents("Mine.pcsettings".into(), bytes))]);
    open_preferences(&mut app, "general");
    let mut h = Harness::builder().with_size(egui::vec2(1280.0, 860.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        app
    });
    h.run_steps(4);
    h.get_by_label("Import Settings…").click();
    h.run_steps(8);
    assert!(prompt(h.state()).is_some(), "the prompt names what is replaced");
    assert_eq!(h.state().session.prefs().export.jpeg_quality, 85);
    h.get_by_label("Preferences");
    h.get_by_label("Import");
}

/// A settings file of quality `quality` with settings this build doesn't have.
fn file_with_unknowns(quality: u32, extra: usize) -> String {
    let mut f: Value = serde_json::from_str(&file_with(quality)).unwrap();
    f["preferences"]["interface"]["fromTheFuture"] = json!(true);
    f["futurePart"] = json!({});
    for n in 0..extra {
        f["preferences"]["general"][format!("later{n}")] = json!(n);
    }
    f.to_string()
}

#[test]
fn the_prompt_names_the_skipped_settings_and_ok_imports_the_rest() {
    let (mut app, _) = super::tests::app_with_store();
    let reply = settings_sync::import_bytes(&mut app, "Newer.pcsettings", file_with_unknowns(33, 0).as_bytes()).unwrap();
    assert_eq!(reply["unknownCount"], 2);
    let (_, f) = prompt(&app).expect("an unknown setting does not refuse the file");
    let names: Vec<&str> = f["unknown"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(names.contains(&"interface.fromTheFuture") && names.contains(&"futurePart"), "{names:?}");
    assert_eq!(app.session.prefs().export.jpeg_quality, 85, "nothing changed yet");
    confirm(&mut app, &f).unwrap();
    assert_eq!(app.session.prefs().export.jpeg_quality, 33, "the known settings are imported");
    assert_eq!(app.ui.status, "Imported settings from Newer.pcsettings (2 skipped)");
}

#[test]
fn the_prompt_line_is_short_and_cuts_a_long_list() {
    assert_eq!(settings_sync::unknown_line(&["a.b", "c"], 2), "Not in this PhotoCraft, skipped: a.b, c");
    let names = ["a", "b", "c", "d", "e"];
    assert_eq!(settings_sync::unknown_line(&names, 5), "Not in this PhotoCraft, skipped: a, b, c, d, e");
    assert_eq!(settings_sync::unknown_line(&names, 12), "Not in this PhotoCraft, skipped: a, b, c, d, e and 7 more");
}

#[test]
fn the_prompt_shows_the_skipped_settings() {
    let bytes = file_with_unknowns(33, 8).into_bytes();
    let (mut app, _) = app_with_dialogs(vec![Some(FileDialogAnswer::Contents("Newer.pcsettings".into(), bytes))]);
    open_preferences(&mut app, "general");
    let mut h = Harness::builder().with_size(egui::vec2(1280.0, 860.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        app
    });
    h.run_steps(4);
    h.get_by_label("Import Settings…").click();
    h.run_steps(8);
    assert!(prompt(h.state()).is_some(), "the prompt opens for a file with settings this build doesn't have");
    h.get_by_label_contains("Not in this PhotoCraft, skipped:");
    h.get_by_label_contains("and 5 more");
}

#[test]
fn the_commands_do_not_open_a_preferences_section() {
    let (mut app, _) = super::tests::app_with_store();
    let ctx = egui::Context::default();
    // Without a file dialog there is nothing to ask: the ids are not mistaken for a section.
    for id in [EXPORT, IMPORT] {
        let r = invoke(&mut app, &ctx, id, &json!({})).expect("handled");
        assert!(r.is_err(), "{id}");
        assert!(app.ui.dialogs.is_empty(), "{id} opened a dialog");
    }
}

#[test]
fn an_agent_calls_the_commands_with_parameters_and_the_shell_follows() {
    let (mut app, store) = super::tests::app_with_store();
    let ctx = egui::Context::default();
    let id = open_preferences(&mut app, "general");
    let dry = invoke(&mut app, &ctx, IMPORT, &json!({"text": file_with(33), "dryRun": true})).unwrap().unwrap();
    assert_eq!(dry["applied"], false);
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
    let done = invoke(&mut app, &ctx, IMPORT, &json!({"text": file_with(33)})).unwrap().unwrap();
    assert_eq!(done["applied"], true);
    assert_eq!(app.session.prefs().export.jpeg_quality, 33);
    let fields = &app.ui.dialogs.iter().find(|d| d.id == id).unwrap().fields;
    assert_eq!(fields["values"]["export"]["jpegQuality"], 33);
    assert!(store.lock().unwrap().as_deref().is_some_and(|t| t.contains("33")), "saved");
    let text = invoke(&mut app, &ctx, EXPORT, &json!({"path": null})).unwrap().unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("\"jpegQuality\": 33"));
    // A bad file is an error and nothing changes.
    let e = invoke(&mut app, &ctx, IMPORT, &json!({"text": file_with(33).replace("33", "0")})).unwrap().unwrap_err();
    assert!(e.contains("Nothing was changed"), "{e}");
    assert_eq!(app.session.prefs().export.jpeg_quality, 33);
}

#[test]
fn an_import_brings_panels_and_tool_options_in_line_like_a_launch() {
    let (mut app, _) = super::tests::app_with_store();
    let mut other = super::tests::app_with_store().0;
    other.run("prefs.set", json!({"path": "workspace.rememberWorkspaceChanges", "value": true})).unwrap();
    other.session.prefs.edit(|p| p.tool_options = json!({"tolerance": 77.0}));
    let text = other.session.settings_text();
    assert!((app.ui.tool_options.tolerance - 77.0).abs() > 1.0, "the starting point");
    settings_sync::import_bytes(&mut app, "Mine.pcsettings", text.as_bytes()).unwrap();
    let (_, f) = prompt(&app).unwrap();
    confirm(&mut app, &f).unwrap();
    assert!((app.ui.tool_options.tolerance - 77.0).abs() < 0.01, "the tool options follow the import at once");
}

#[test]
fn automation_cannot_confirm_the_prompt() {
    let (mut app, _) = super::tests::app_with_store();
    settings_sync::import_bytes(&mut app, "Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    let (_, f) = prompt(&app).unwrap();
    app.automation_input = true;
    assert!(confirm(&mut app, &f).is_err());
    app.automation_input = false;
    app.session.authorize = Some(|_, _| Ok(()));
    assert!(confirm(&mut app, &f).is_err());
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
}
