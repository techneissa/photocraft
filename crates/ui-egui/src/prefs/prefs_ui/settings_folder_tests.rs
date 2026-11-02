//! The Sync Folder row in the Preferences window, Load from Folder's prompt, the two commands'
//! shell front end and Keep in Sync at start and on quit. Every folder is a temporary one; the
//! real ArtCraft folder and iCloud Drive are never touched (the session's `sync_env` points
//! inside the temporary folders).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::{Value, json};

use photocraft_engine::Session;
use photocraft_engine::prefs::settings_folder::{FILE_NAME, SHARED_NAME, SavedBy, SyncEnv};

use super::settings_folder::{LOAD, SAVE};
use super::*;
use crate::PhotocraftApp;
use crate::file_dialog::FileDialogAnswer;

/// A new empty temporary folder, removed at the end.
struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("pc-sync-ui-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn s(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Store = Arc<Mutex<Option<String>>>;

/// A session on the Mac `mac` whose shared file is `home/ArtCraft/sync-folder.json`.
fn session_on(mac: &str, home: &Dir) -> Session {
    let mut s = Session::new();
    s.sync_env = SyncEnv { shared_file: Some(home.path().join("ArtCraft").join(SHARED_NAME)), computer: Some(mac.into()) };
    s
}

/// An app on `session` whose file dialogs give `answers` and whose preferences live in the returned store.
fn app_on(session: Session, saved: Option<String>, answers: Vec<Option<FileDialogAnswer>>) -> (PhotocraftApp, Store) {
    let store: Store = Arc::new(Mutex::new(saved));
    let (a, b) = (store.clone(), store.clone());
    let (picker, _) = crate::file_dialog::fake(answers);
    let services = crate::Services {
        file_dialog: Some(picker),
        load_prefs: Some(Box::new(move || a.lock().unwrap().clone())),
        save_prefs: Some(Box::new(move |s: &str| {
            *b.lock().unwrap() = Some(s.to_string());
            Ok(())
        })),
        ..Default::default()
    };
    (PhotocraftApp::new(session, services), store)
}

fn harness(app: PhotocraftApp) -> Harness<'static, PhotocraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1280.0, 860.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        app
    });
    h.run_steps(4);
    h
}

fn prompt(app: &PhotocraftApp) -> Option<(u64, serde_json::Map<String, Value>)> {
    app.ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(Value::as_str) == Some("settingsImport")).map(|d| (d.id, d.fields.clone()))
}

fn message(app: &PhotocraftApp) -> Value {
    app.ui.dialogs.iter().find(|d| is_preferences(&d.fields)).and_then(|d| d.fields.get("__syncMessage").cloned()).unwrap_or(Value::Null)
}

fn file_json(folder: &Dir) -> Value {
    serde_json::from_str(&std::fs::read_to_string(folder.path().join(FILE_NAME)).unwrap()).unwrap()
}

/// The prefs file text of a Mac with `folder` chosen and Keep in Sync as `keep`.
fn saved_prefs(home: &Dir, folder: &Dir, keep: bool, last: &str) -> String {
    let mut s = session_on("Laptop", home);
    s.choose_sync_folder(&folder.s()).unwrap();
    s.prefs.edit(|p| {
        p.sync.keep = keep;
        p.sync.last = last.into();
    });
    serde_json::to_string(&s.prefs_value()).unwrap()
}

/// Put a settings file from the Mac `mac` saved at `at`, with JPEG quality `quality`, in `folder`.
fn put_file_from(folder: &Dir, mac: &str, at: &str, quality: u32) {
    let mut other = Session::new();
    other.execute("prefs.set", json!({"path": "export.jpegQuality", "value": quality})).unwrap();
    let text = photocraft_engine::prefs::settings_folder::stamped_text(other.prefs(), &other.color.settings, &SavedBy { computer: mac.into(), at: at.into() });
    std::fs::write(folder.path().join(FILE_NAME), text).unwrap();
}

#[test]
fn the_row_has_its_buttons_and_save_and_load_wait_for_a_folder() {
    let home = Dir::new("home");
    let (mut app, _) = app_on(session_on("Studio Mac", &home), None, vec![]);
    open_preferences(&mut app, "general");
    let h = harness(app);
    h.get_by_label("Sync Folder");
    h.get_by_label("None chosen");
    h.get_by_label("Choose…");
    assert!(h.get_by_label("Save to Folder").accesskit_node().is_disabled());
    assert!(h.get_by_label("Load from Folder").accesskit_node().is_disabled());
    h.get_by_label("Export Settings…");
}

#[test]
fn choose_asks_for_a_folder_and_remembers_it_here_and_for_the_other_apps() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let (mut app, store) = app_on(session_on("Studio Mac", &home), None, vec![Some(FileDialogAnswer::FolderTo(folder.s()))]);
    open_preferences(&mut app, "general");
    let mut h = harness(app);
    h.get_by_label("Choose…").click();
    h.run_steps(8);
    assert_eq!(h.state().session.prefs().sync.folder, folder.s());
    let shared: Value = serde_json::from_str(&std::fs::read_to_string(home.path().join("ArtCraft").join(SHARED_NAME)).unwrap()).unwrap();
    assert_eq!(shared, json!({"path": folder.s()}));
    h.run_steps(4);
    assert!(!h.get_by_label("Save to Folder").accesskit_node().is_disabled(), "a folder is chosen");
    assert_eq!(message(h.state())["warning"], false);
    // It is kept with the other preferences.
    let ctx = egui::Context::default();
    tick(h.state_mut(), &ctx);
    let stored: Value = serde_json::from_str(store.lock().unwrap().as_deref().unwrap()).unwrap();
    assert_eq!(stored["sync"]["folder"], json!(folder.s()));
    // The Preferences window still has nothing unsaved from this.
    assert!(!preferences_changed(h.state(), &h.state().ui.dialogs.iter().find(|d| is_preferences(&d.fields)).unwrap().fields));
}

#[test]
fn a_folder_the_other_apps_use_is_offered_and_taken_in_one_click() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    std::fs::create_dir_all(home.path().join("ArtCraft")).unwrap();
    std::fs::write(home.path().join("ArtCraft").join(SHARED_NAME), json!({"path": folder.s(), "other": 1}).to_string()).unwrap();
    let (mut app, _) = app_on(session_on("Studio Mac", &home), None, vec![]);
    open_preferences(&mut app, "general");
    let mut h = harness(app);
    h.get_by_label("Use This Folder").click();
    h.run_steps(6);
    assert_eq!(h.state().session.prefs().sync.folder, folder.s());
    h.run_steps(4);
    assert!(h.query_by_label("Use This Folder").is_none(), "nothing left to offer");
}

#[test]
fn a_missing_or_bad_shared_file_offers_nothing_and_is_no_error() {
    let home = Dir::new("home");
    for text in [None, Some("garbage"), Some(r#"{"path": 5}"#)] {
        if let Some(t) = text {
            std::fs::create_dir_all(home.path().join("ArtCraft")).unwrap();
            std::fs::write(home.path().join("ArtCraft").join(SHARED_NAME), t).unwrap();
        }
        let (mut app, _) = app_on(session_on("Studio Mac", &home), None, vec![]);
        open_preferences(&mut app, "general");
        let h = harness(app);
        assert!(h.query_by_label("Use This Folder").is_none());
        h.get_by_label("None chosen");
    }
}

#[test]
fn save_to_folder_writes_what_the_window_shows_and_load_asks_naming_the_mac_and_time() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut studio = session_on("Studio Mac", &home);
    studio.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, _) = app_on(studio, None, vec![]);
    let id = open_preferences(&mut app, "general");
    app.ui.dialog_mut(id).unwrap().fields.get_mut("values").unwrap()["export"]["jpegQuality"] = json!(44);
    let mut h = harness(app);
    h.get_by_label("Save to Folder").click();
    h.run_steps(6);
    let file = file_json(&folder);
    assert_eq!(file["preferences"]["export"]["jpegQuality"], 44, "the window's value");
    assert_eq!(file["savedBy"]["computer"], "Studio Mac");
    assert_eq!(h.state().session.prefs().export.jpeg_quality, 85, "saving to the folder changes nothing here");
    assert_eq!(message(h.state())["warning"], false);
    assert!(message(h.state())["text"].as_str().unwrap().starts_with("Saved settings to "));
    assert_eq!(h.state().session.prefs().sync.last, file["savedBy"]["at"].as_str().unwrap());

    // Another Mac loads it.
    let mut laptop = session_on("Laptop", &home);
    laptop.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, _) = app_on(laptop, None, vec![]);
    open_preferences(&mut app, "general");
    let mut h = harness(app);
    h.get_by_label("Load from Folder").click();
    h.run_steps(8);
    let (_, f) = prompt(h.state()).expect("the prompt");
    assert_eq!(f["savedBy"]["computer"], "Studio Mac");
    assert_eq!(f["__label"], "Load Settings from Folder");
    h.get_by_label_contains("Saved on Studio Mac,");
    h.get_by_label_contains(&format!("those in {FILE_NAME}"));
    assert_eq!(h.state().session.prefs().export.jpeg_quality, 85, "nothing changed before OK");
    let (_, f) = prompt(h.state()).unwrap();
    confirm(h.state_mut(), &f).unwrap();
    assert_eq!(h.state().session.prefs().export.jpeg_quality, 44);
    assert_eq!(h.state().session.prefs().sync.last, file["savedBy"]["at"].as_str().unwrap(), "this Mac now has that file");
    assert_eq!(h.state().session.prefs().sync.folder, folder.s(), "its own folder stays");
}

#[test]
fn the_load_prompt_names_unknown_settings_and_a_bad_value_refuses() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    put_file_from(&folder, "Studio Mac", "2026-11-02T10:00:00+01:00", 33);
    let mut doc = file_json(&folder);
    doc["preferences"]["export"]["fromTheFuture"] = json!(true);
    std::fs::write(folder.path().join(FILE_NAME), doc.to_string()).unwrap();
    let mut s = session_on("Laptop", &home);
    s.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, _) = app_on(s, None, vec![]);
    settings_folder::load_dialog(&mut app).unwrap();
    let (_, f) = prompt(&app).unwrap();
    assert_eq!(f["unknown"], json!(["export.fromTheFuture"]));
    // A bad known value: an error, no prompt.
    doc["preferences"]["export"]["jpegQuality"] = json!(0);
    std::fs::write(folder.path().join(FILE_NAME), doc.to_string()).unwrap();
    let (mut app, _) = app_on(
        {
            let mut s = session_on("Laptop", &home);
            s.choose_sync_folder(&folder.s()).unwrap();
            s
        },
        None,
        vec![],
    );
    let e = settings_folder::load_dialog(&mut app).unwrap_err();
    assert!(e.contains("Nothing was changed"), "{e}");
    assert!(prompt(&app).is_none());
}

#[test]
fn a_folder_that_is_gone_or_has_no_file_says_what_to_do_and_changes_nothing() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut s = session_on("Laptop", &home);
    s.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, _) = app_on(s, None, vec![]);
    open_preferences(&mut app, "general");
    let mut h = harness(app);
    h.get_by_label("Load from Folder").click();
    h.run_steps(6);
    assert!(prompt(h.state()).is_none());
    assert_eq!(message(h.state())["warning"], true);
    assert!(message(h.state())["text"].as_str().unwrap().contains("There is no photocraft.pcsettings"), "{}", message(h.state()));
    drop(folder);
    h.get_by_label("Save to Folder").click();
    h.run_steps(6);
    let text = message(h.state())["text"].as_str().unwrap().to_string();
    assert!(text.contains("isn't there") && text.contains("Choose the sync folder again"), "{text}");
    assert_eq!(message(h.state())["warning"], true);
}

#[test]
fn keep_in_sync_is_a_checkbox_that_is_remembered() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut s = session_on("Laptop", &home);
    s.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, _) = app_on(s, None, vec![]);
    open_preferences(&mut app, "general");
    let mut h = harness(app);
    assert!(!h.state().session.prefs().sync.keep);
    h.get_by_label("Keep in Sync").click();
    h.run_steps(4);
    assert!(h.state().session.prefs().sync.keep);
    h.get_by_label("Keep in Sync").click();
    h.run_steps(4);
    assert!(!h.state().session.prefs().sync.keep);
}

#[test]
fn the_menu_commands_save_and_ask_and_an_agent_gets_the_engines_answer() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut s = session_on("Studio Mac", &home);
    s.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, store) = app_on(s, None, vec![]);
    let ctx = egui::Context::default();
    // A person: no parameters.
    let saved = invoke(&mut app, &ctx, SAVE, &json!({})).expect("handled").unwrap();
    assert!(saved["file"].as_str().unwrap().ends_with(FILE_NAME));
    assert_eq!(file_json(&folder)["savedBy"]["computer"], "Studio Mac");
    put_file_from(&folder, "Laptop", "2026-11-02T10:00:00+01:00", 33);
    invoke(&mut app, &ctx, LOAD, &json!({})).expect("handled").unwrap();
    assert!(prompt(&app).is_some(), "a person is asked first");
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
    // An agent: parameters, or an agent's session.
    let dry = invoke(&mut app, &ctx, LOAD, &json!({"dryRun": true})).unwrap().unwrap();
    assert_eq!((dry["applied"].clone(), dry["savedBy"]["computer"].clone()), (json!(false), json!("Laptop")));
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
    app.automation_input = true;
    let done = invoke(&mut app, &ctx, LOAD, &json!({})).unwrap().unwrap();
    assert_eq!(done["applied"], true);
    assert_eq!(app.session.prefs().export.jpeg_quality, 33);
    let stored: Value = serde_json::from_str(store.lock().unwrap().as_deref().unwrap()).unwrap();
    assert_eq!(stored["export"]["jpegQuality"], 33, "saved");
    // The ids are not mistaken for a Preferences section.
    assert!(invoke(&mut app, &ctx, "edit.preferences.general", &json!({})).is_none() || app.ui.dialogs.iter().any(|d| is_preferences(&d.fields)));
}

#[test]
fn keep_in_sync_offers_a_newer_file_from_another_mac_at_start() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    put_file_from(&folder, "Studio Mac", "2026-11-02T10:00:00+01:00", 33);
    // Off: nothing is asked.
    let (app, _) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &folder, false, "")), vec![]);
    assert!(prompt(&app).is_none());
    // On, never loaded here: the prompt names the Mac and the time, and nothing changes before OK.
    let (mut app, store) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &folder, true, "")), vec![]);
    let (_, f) = prompt(&app).expect("offered at start");
    assert_eq!(f["savedBy"]["computer"], "Studio Mac");
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
    confirm(&mut app, &f).unwrap();
    assert_eq!(app.session.prefs().export.jpeg_quality, 33);
    assert_eq!(app.session.prefs().sync.last, "2026-11-02T10:00:00+01:00");
    assert!(store.lock().unwrap().as_deref().is_some_and(|t| t.contains("2026-11-02T10:00:00+01:00")), "stored");
    // Already loaded here (or newer here): not offered again.
    let (app, _) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &folder, true, "2026-11-02T10:00:00+01:00")), vec![]);
    assert!(prompt(&app).is_none());
    // The Mac's own file is never offered to it.
    let (app, _) = app_on(session_on("Studio Mac", &home), Some(saved_prefs(&home, &folder, true, "")), vec![]);
    assert!(prompt(&app).is_none());
    // Cancel leaves everything as it was.
    let (mut app, _) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &folder, true, "")), vec![]);
    let (id, _) = prompt(&app).unwrap();
    crate::dialogs::cancel(&mut app, id).unwrap();
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
}

#[test]
fn keep_in_sync_with_a_folder_that_is_gone_says_so_at_start_and_never_crashes() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let text = saved_prefs(&home, &folder, true, "");
    drop(folder);
    let (app, _) = app_on(session_on("Laptop", &home), Some(text), vec![]);
    assert!(prompt(&app).is_none());
    assert!(app.ui.status.contains("isn't there"), "{}", app.ui.status);
    assert!(app.ui.status_error);
}

#[test]
fn keep_in_sync_saves_on_quit_and_stores_the_time() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let (mut app, store) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &folder, true, "")), vec![]);
    app.run("prefs.set", json!({"path": "export.jpegQuality", "value": 61})).unwrap();
    save_sync_on_exit(&mut app);
    let file = file_json(&folder);
    assert_eq!(file["preferences"]["export"]["jpegQuality"], 61);
    assert_eq!(file["savedBy"]["computer"], "Laptop");
    let stored: Value = serde_json::from_str(store.lock().unwrap().as_deref().unwrap()).unwrap();
    assert_eq!(stored["sync"]["last"], file["savedBy"]["at"], "the save is stored with the preferences");
    // Off: nothing is written.
    let other = Dir::new("other");
    let (mut app, _) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &other, false, "")), vec![]);
    save_sync_on_exit(&mut app);
    assert!(!other.path().join(FILE_NAME).exists());
    // A newer file from another Mac that was never loaded is left alone.
    put_file_from(&other, "Studio Mac", "2999-01-01T00:00:00+00:00", 20);
    let (mut app, _) = app_on(session_on("Laptop", &home), Some(saved_prefs(&home, &other, true, "")), vec![]);
    let id = prompt(&app).unwrap().0;
    crate::dialogs::cancel(&mut app, id).unwrap();
    save_sync_on_exit(&mut app);
    assert_eq!(file_json(&other)["savedBy"]["computer"], "Studio Mac");
    // A folder that went away does not crash the quit.
    let gone = Dir::new("gone");
    let text = saved_prefs(&home, &gone, true, "");
    drop(gone);
    let (mut app, _) = app_on(session_on("Laptop", &home), Some(text), vec![]);
    save_sync_on_exit(&mut app);
}

#[test]
fn another_apps_file_is_not_touched_by_save_or_load_in_the_shell() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let theirs = folder.path().join("lightcraft.lcsettings");
    std::fs::write(&theirs, b"their settings").unwrap();
    let mut s = session_on("Studio Mac", &home);
    s.choose_sync_folder(&folder.s()).unwrap();
    let (mut app, _) = app_on(s, None, vec![]);
    assert!(settings_folder::load_dialog(&mut app).is_err(), "only their file is there");
    settings_folder::save_now(&mut app).unwrap();
    settings_folder::load_dialog(&mut app).unwrap();
    assert_eq!(std::fs::read(&theirs).unwrap(), b"their settings");
}
