//! Settings sync in the shell: Export Settings…, Import Settings…, the prompt before an import, the sync folder and
//! the open Settings window following along (fork/settings-sync.md). The file's own rules are the engine's tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use photocraft_engine::Session;
use photocraft_engine::color_cmds::ColorExt;
use photocraft_engine::prefs::settings_folder::{FILE_NAME, SHARED_NAME, SavedBy, SyncEnv};
use serde_json::{Value, json};

use super::settings_sync::{self, ImportAsk, Request};
use super::*;
use crate::PhotocraftApp;
use crate::dialogs::alerts::prompt_tag;

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

/// A settings file of an app whose JPEG quality is `quality`.
fn file_with(quality: u32) -> String {
    let mut other = Session::new();
    other.execute("prefs.set", json!({"path": "export.jpegQuality", "value": quality})).unwrap();
    other.settings_text()
}

/// An app on the Mac `mac` whose shared file is `home/ArtCraft/sync-folder.json`.
fn app_on(mac: &str, home: &Dir) -> Box<PhotocraftApp> {
    let mut session = Session::new();
    session.sync_env = SyncEnv { shared_file: Some(home.path().join("ArtCraft").join(SHARED_NAME)), computer: Some(mac.into()) };
    PhotocraftApp::new(session, crate::Services::default())
}

fn plain_app() -> Box<PhotocraftApp> {
    PhotocraftApp::new(Session::new(), crate::Services::default())
}

fn ask_of(app: &PhotocraftApp) -> Option<ImportAsk> {
    prompt_tag::<ImportAsk>(app).cloned()
}

#[test]
fn an_import_asks_first_and_changes_nothing_until_it_is_confirmed() {
    let mut app = plain_app();
    let reply = settings_sync::import_bytes(&mut app, "/Users/a/Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    assert!(reply["parts"].as_array().is_some_and(|p| !p.is_empty()), "the reply names what it replaces: {reply}");
    let ask = ask_of(&app).expect("the prompt is open");
    assert_eq!(app.session.prefs().export.jpeg_quality, 85, "nothing changed before OK");
    settings_sync::confirm(&mut app, &ask).unwrap();
    assert_eq!(app.session.prefs().export.jpeg_quality, 33);
}

#[test]
fn a_refused_file_says_why_and_opens_no_prompt() {
    let mut app = plain_app();
    let bad: [&[u8]; 3] = [b"not json", &[0xff, 0xfe, 0x00], br#"{"kind":"photocraft-settings","version":1,"preferences":{"export":{"jpegQuality":"high"}}}"#];
    for bytes in bad {
        assert!(settings_sync::import_bytes(&mut app, "Bad.pcsettings", bytes).is_err());
        assert!(ask_of(&app).is_none());
    }
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
}

#[test]
fn an_agent_cannot_confirm_the_prompt_and_calls_the_command_with_the_text_instead() {
    let mut app = plain_app();
    settings_sync::import_bytes(&mut app, "Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    let ask = ask_of(&app).unwrap();
    app.automation_input = true;
    assert!(settings_sync::confirm(&mut app, &ask).unwrap_err().contains("confirmed by a person"));
    app.automation_input = false;
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
    let reply = settings_sync::invoke(&mut app, settings_sync::IMPORT, &json!({"text": file_with(44)})).unwrap().unwrap();
    assert_eq!(reply["applied"], true);
    assert_eq!(app.session.prefs().export.jpeg_quality, 44);
}

#[test]
fn the_prompt_cuts_a_long_list_of_skipped_settings() {
    assert_eq!(settings_sync::unknown_line(&["a.b", "c"], 2), "Not in this PhotoCraft, skipped: a.b, c");
    let names = ["a", "b", "c", "d", "e", "f"];
    assert_eq!(settings_sync::unknown_line(&names, 12), "Not in this PhotoCraft, skipped: a, b, c, d, e and 7 more");
}

#[test]
fn with_settings_open_the_prompt_is_inside_the_window_and_the_export_writes_its_draft() {
    let mut app = plain_app();
    super::tests::open_prefs(&mut app, "general");
    super::tests::edit(&mut app, "export.jpegQuality", craft::ui::patterns::preferences::PrefValue::Int(44));
    let (prefs, _) = settings_sync::settings_now(&app);
    assert_eq!(prefs.export.jpeg_quality, 44, "the export takes what the window shows");
    assert_eq!(app.session.prefs().export.jpeg_quality, 85);
    settings_sync::import_bytes(&mut app, "Mine.pcsettings", file_with(33).as_bytes()).unwrap();
    assert!(ask_of(&app).is_none(), "the app-modal window would block a prompt in the main window");
    assert!(settings_sync::lock(&app.prefs_rt.bridge).import.is_some());
}

#[test]
fn the_buttons_file_requests_the_app_runs() {
    let mut app = plain_app();
    super::tests::open_prefs(&mut app, "general");
    settings_sync::lock(&app.prefs_rt.bridge).requests.push(Request::Keep(true));
    super::dialogs::take(&mut app, &egui::Context::default());
    assert!(app.session.prefs().sync.keep);
}

#[test]
fn save_to_folder_writes_the_window_and_another_mac_loads_it_naming_who_saved_it() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut studio = app_on("Studio Mac", &home);
    studio.session.choose_sync_folder(&folder.s()).unwrap();
    super::tests::open_prefs(&mut studio, "general");
    super::tests::edit(&mut studio, "export.jpegQuality", craft::ui::patterns::preferences::PrefValue::Int(44));
    settings_folder::save_now(&mut studio).unwrap();
    let file: Value = serde_json::from_str(&std::fs::read_to_string(folder.path().join(FILE_NAME)).unwrap()).unwrap();
    assert_eq!(file["preferences"]["export"]["jpegQuality"], 44, "the window's value");
    assert_eq!(file["savedBy"]["computer"], "Studio Mac");
    assert_eq!(studio.session.prefs().export.jpeg_quality, 85, "saving to the folder changes nothing here");

    let mut laptop = app_on("Laptop", &home);
    laptop.session.choose_sync_folder(&folder.s()).unwrap();
    settings_folder::load_dialog(&mut laptop).unwrap();
    let ask = ask_of(&laptop).expect("the prompt");
    assert_eq!(ask.saved_by.as_ref().map(|by| by.computer.as_str()), Some("Studio Mac"));
    assert_eq!(laptop.session.prefs().export.jpeg_quality, 85, "nothing changed before OK");
    settings_sync::confirm(&mut laptop, &ask).unwrap();
    assert_eq!(laptop.session.prefs().export.jpeg_quality, 44);
    assert_eq!(laptop.session.prefs().sync.last, file["savedBy"]["at"].as_str().unwrap(), "this Mac now has that file");
    assert_eq!(laptop.session.prefs().sync.folder, folder.s(), "its own folder stays");
}

#[test]
fn keep_in_sync_offers_a_newer_file_from_another_mac_at_start_and_never_its_own() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut other = Session::new();
    other.execute("prefs.set", json!({"path": "export.jpegQuality", "value": 33})).unwrap();
    let by = SavedBy { computer: "Studio Mac".into(), at: "2026-11-02T10:00:00+01:00".into() };
    let text = photocraft_engine::prefs::settings_folder::stamped_text(other.prefs(), &other.color().settings, &by);
    std::fs::write(folder.path().join(FILE_NAME), text).unwrap();

    let keeping = |mac: &str| {
        let mut app = app_on(mac, &home);
        app.session.choose_sync_folder(&folder.s()).unwrap();
        app.session.prefs.edit(|p| p.sync.keep = true);
        app
    };
    let mut laptop = keeping("Laptop");
    settings_folder::offer_newer_file(&mut laptop);
    assert!(ask_of(&laptop).is_some(), "offered at start");
    let mut studio = keeping("Studio Mac");
    settings_folder::offer_newer_file(&mut studio);
    assert!(ask_of(&studio).is_none(), "the Mac's own file is never offered to it");
    let mut off = keeping("Laptop");
    off.session.prefs.edit(|p| p.sync.keep = false);
    settings_folder::offer_newer_file(&mut off);
    assert!(ask_of(&off).is_none(), "Keep in Sync off: nothing is asked");
}

#[test]
fn the_menus_list_the_settings_sync_items_and_remove_subject() {
    let app = plain_app();
    let rows = crate::menus::menu_rows(&app);
    let path_of = |id: &str| rows.iter().find(|r| r.command.id == id).map(|r| r.path.join(" > "));
    assert_eq!(path_of("edit.removeSubject").as_deref(), Some("Edit"));
    for id in [settings_sync::EXPORT, settings_sync::IMPORT, "edit.preferences.saveSettingsToFolder", "edit.preferences.loadSettingsFromFolder"] {
        assert_eq!(path_of(id).as_deref(), Some("Edit > Preferences"), "{id}");
    }
}
