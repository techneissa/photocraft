//! The Sync Folder row of Preferences, and the shell's side of Keep in Sync: Choose…, Save to
//! Folder and Load from Folder (and `edit.preferences.saveSettingsToFolder` /
//! `loadSettingsFromFolder`), the "saved on this Mac at this time" line of the load prompt, and
//! the offer of a newer file at start. The folder, the file and the rules are the engine's
//! ([`photocraft_engine::prefs::settings_folder`]).
//!
//! Load from Folder opens the same prompt as Import Settings… ([`super::settings_sync`]); the
//! prompt's OK notes that this Mac now has the file ([`note_loaded`]).

use serde_json::{Map, Value, json};

pub(super) use photocraft_engine::prefs::settings_folder::SavedBy;
use photocraft_engine::prefs::settings_folder::{self, FILE_NAME, FolderFile};

use super::PhotocraftApp;
use super::settings_sync::{self, Request};
use craft::host::{PickRequest, Picked};

/// Edit › Preferences › Save Settings to Folder.
pub const SAVE: &str = "edit.preferences.saveSettingsToFolder";
/// Edit › Preferences › Load Settings from Folder.
pub const LOAD: &str = "edit.preferences.loadSettingsFromFolder";

/// `path` with the home folder written `~`, as the row shows it.
pub(super) fn tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.strip_prefix(home.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

/// What the Sync Folder block's buttons asked for.
pub(super) fn run(app: &mut PhotocraftApp, request: Request) -> Result<Value, String> {
    match request {
        Request::Choose => choose_dialog(app),
        Request::UseOffered => match app.session.offered_sync_folder() {
            Some(folder) => choose_folder(app, &folder),
            None => Ok(Value::Null),
        },
        Request::Save => save_now(app),
        Request::Load => load_dialog(app),
        _ => Ok(Value::Null),
    }
}

/// Choose…: ask for the folder, starting in the chosen one, else in iCloud Drive when this Mac has it.
fn choose_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    let chosen = app.session.prefs().sync.folder.clone();
    let icloud = std::env::var("HOME").ok().map(|h| format!("{h}/Library/Mobile Documents/com~apple~CloudDocs"));
    let initial_dir = Some(chosen).filter(|c| std::path::Path::new(c).is_dir()).or_else(|| icloud.filter(|p| std::path::Path::new(p).is_dir()));
    let mut request = PickRequest::folder("sync");
    if let Some(dir) = initial_dir {
        request = request.directory(dir);
    }
    app.ask_file(request, |app, picked| match picked {
        Picked::Paths(paths) => match paths.first() {
            Some(path) => choose_folder(app, &path.to_string_lossy()),
            None => Err(crate::file_dialog::UNEXPECTED.into()),
        },
        _ => Err(crate::file_dialog::UNEXPECTED.into()),
    })
}

/// Use `folder`: remember it here and for the other ArtCraft apps.
pub(super) fn choose_folder(app: &mut PhotocraftApp, folder: &str) -> Result<Value, String> {
    match app.session.choose_sync_folder(folder) {
        // The row shows the folder; no message repeating it under the search field.
        Ok(()) => {}
        Err(e) => settings_sync::report(app, &e, true),
    }
    Ok(json!({"folder": folder}))
}

/// Save to Folder: the settings as the Preferences window shows them, in the folder.
pub(super) fn save_now(app: &mut PhotocraftApp) -> Result<Value, String> {
    let (prefs, color) = settings_sync::settings_now(app);
    let saved = settings_folder::save(&prefs.sync.folder, &prefs, &color, app.session.sync_env.saved_by())?;
    app.session.note_synced(&saved.by);
    let shown = saved.path.to_string_lossy().into_owned();
    let message = crate::i18n::fmt(tl!("Saved settings to {path}"), &[("path", &tilde(&shown))]);
    settings_sync::report(app, &message, false);
    Ok(json!({"file": shown, "savedBy": saved.by}))
}

/// Load from Folder: read the file and ask before replacing anything.
pub(super) fn load_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    let file = settings_folder::read(&app.session.prefs().sync.folder)?;
    open_prompt(app, &file)
}

/// The prompt of Import Settings… for the sync folder's file, naming who saved it and when.
pub(crate) fn open_prompt(app: &mut PhotocraftApp, file: &FolderFile) -> Result<Value, String> {
    let reply = settings_sync::import_bytes_by(app, FILE_NAME, file.text.as_bytes(), file.by.clone()).inspect_err(|e| settings_sync::report(app, e, true))?;
    Ok(reply)
}

// ------------------------------------------------------------------ the commands

/// The shell's front end of the two commands. Called by a person (the menu) with no parameters
/// they save the settings as the window shows them, or ask before loading; with parameters, or
/// from an agent, the engine does the work and the shell follows.
pub(super) fn invoke(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if id != SAVE && id != LOAD {
        return None;
    }
    settings_sync::ensure_checks(app);
    let by_a_person = params.as_object().is_none_or(Map::is_empty) && !app.automation_input && app.session.authorize.is_none();
    if by_a_person {
        return Some(if id == SAVE { save_now(app) } else { load_dialog(app) });
    }
    let result = app.session.execute(id, params.clone()).map_err(|e| e.to_string());
    if id == LOAD && result.as_ref().is_ok_and(|r| r["applied"] == true) {
        settings_sync::after_import(app);
        let _ = super::save_preferences(app);
    }
    Some(result)
}

// ------------------------------------------------------------------ Keep in Sync

/// At start, with Keep in Sync on: the folder's file, when another Mac saved a newer one than
/// this Mac last saved or loaded, put to the user as the load prompt. Nothing changes before
/// OK. A folder that can't be read is a message in the status bar.
pub fn offer_newer_file(app: &mut PhotocraftApp) {
    match app.session.newer_sync_file() {
        Ok(Some(file)) => {
            settings_sync::ensure_checks(app);
            if let Err(e) = open_prompt(app, &file) {
                log::warn!("Settings sync: {e}");
            }
        }
        Ok(None) => {}
        Err(e) => {
            app.ui.status = e;
            app.ui.status_error = true;
        }
    }
}

/// On quit, with Keep in Sync on: save to the folder (see the engine's `save_settings_on_quit`
/// for when it does not), then store that this Mac saved.
pub fn save_on_exit(app: &mut PhotocraftApp) {
    match app.session.save_settings_on_quit() {
        Ok(Some(_)) => {
            if let Err(e) = super::save_preferences(app) {
                log::warn!("Settings sync: could not store the time of the last save: {e}");
            }
        }
        Ok(None) => {}
        Err(e) => log::warn!("Settings sync: nothing was saved to the sync folder on quit: {e}"),
    }
}
