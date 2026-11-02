//! The Sync Folder row of Preferences, and the shell's side of Keep in Sync: Choose…, Save to
//! Folder and Load from Folder (and `edit.preferences.saveSettingsToFolder` /
//! `loadSettingsFromFolder`), the "saved on this Mac at this time" line of the load prompt, and
//! the offer of a newer file at start. The folder, the file and the rules are the engine's
//! ([`photocraft_engine::prefs::settings_folder`]).
//!
//! Load from Folder opens the same prompt as Import Settings… ([`super::settings_sync`]); the
//! prompt's OK notes that this Mac now has the file ([`note_loaded`]).

use serde_json::{Map, Value, json};

use photocraft_engine::prefs::settings_folder::{self, FILE_NAME, FolderFile, SavedBy};

use super::{PhotocraftApp, RichText, Tokens, settings_sync};
use crate::file_dialog::{FileDialogAnswer, FileDialogRequest};

/// Edit › Preferences › Save Settings to Folder.
pub const SAVE: &str = "edit.preferences.saveSettingsToFolder";
/// Edit › Preferences › Load Settings from Folder.
pub const LOAD: &str = "edit.preferences.loadSettingsFromFolder";
/// The open Preferences window's cache of the folder the other ArtCraft apps use (read once).
const OFFER: &str = "__syncOffer";

/// What the row's buttons asked for.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Clicks {
    choose: bool,
    use_offered: bool,
    save: bool,
    load: bool,
    keep: Option<bool>,
}

/// `path` with the home folder written `~`, as the row shows it.
fn tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.strip_prefix(home.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

/// The room the checkbox takes, a little over its label.
const KEEP_WIDTH: f32 = 130.0;

/// The row under the search field: Sync Folder, the chosen folder (or the one the other ArtCraft
/// apps use, to take in one click), Choose…, Save to Folder, Load from Folder and Keep in Sync.
pub(super) fn row(ui: &mut egui::Ui, app: &PhotocraftApp, f: &mut Map<String, Value>) -> Clicks {
    let t = Tokens::get(ui.ctx());
    let sync = &app.session.prefs().sync;
    let chosen = !sync.folder.is_empty();
    let offered = if chosen {
        None
    } else {
        let cached = f.entry(OFFER).or_insert_with(|| json!(app.session.offered_sync_folder().unwrap_or_default()));
        cached.as_str().filter(|s| !s.is_empty()).map(str::to_string)
    };
    let mut clicks = Clicks::default();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Sync Folder")).color(t.text));
        let (choose, use_it, save, load) = (tl!("Choose…"), tl!("Use This Folder"), tl!("Save to Folder"), tl!("Load from Folder"));
        let mut labels = vec![choose, save, load];
        if offered.is_some() {
            labels.push(use_it);
        }
        let gap = ui.spacing().item_spacing.x;
        let reserved = labels.iter().map(|l| crate::widgets::button_width(ui, &[*l])).sum::<f32>() + KEEP_WIDTH + gap * (labels.len() as f32 + 1.0);
        let width = (ui.available_width() - reserved).max(80.0);
        let (shown, full, color) = match (&sync.folder, &offered) {
            (folder, _) if chosen => (tilde(folder), folder.clone(), t.text_dim),
            (_, Some(o)) => (tilde(o), o.clone(), t.text_faint),
            _ => (tl!("None chosen").to_string(), String::new(), t.text_faint),
        };
        let label = ui.allocate_ui_with_layout(egui::vec2(width, ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(RichText::new(shown).color(color)).truncate())
        });
        if !full.is_empty() {
            label.inner.on_hover_text(full);
        }
        if offered.is_some() && crate::widgets::secondary_button(ui, use_it, 0.0).clicked() {
            clicks.use_offered = true;
        }
        clicks.choose = crate::widgets::secondary_button(ui, choose, 0.0).clicked();
        ui.add_enabled_ui(chosen, |ui| {
            clicks.save = crate::widgets::secondary_button(ui, save, 0.0).clicked();
            clicks.load = crate::widgets::secondary_button(ui, load, 0.0).clicked();
            let mut keep = sync.keep;
            if crate::widgets::checkbox(ui, &mut keep, "Keep in Sync").changed() {
                clicks.keep = Some(keep);
            }
        });
    });
    clicks
}

/// Do what the row asked for, if anything. The Preferences window is drawn without its entry in the dialog list, so a message the action
/// leaves in the status bar is put in the window's own fields (`f`) here.
pub(super) fn run(app: &mut PhotocraftApp, f: &mut Map<String, Value>, clicks: Clicks) {
    let before = (app.ui.status.clone(), app.ui.status_error);
    if let Some(on) = clicks.keep {
        app.session.prefs.edit(|p| p.sync.keep = on);
    }
    let result = if clicks.choose {
        choose_dialog(app)
    } else if clicks.use_offered {
        match app.session.offered_sync_folder() {
            Some(folder) => choose_folder(app, &folder),
            None => return,
        }
    } else if clicks.save {
        save_now(app)
    } else if clicks.load {
        load_dialog(app)
    } else {
        return;
    };
    if let Err(e) = result
        && e != crate::file_dialog::CANCELLED
    {
        settings_sync::report(app, &e, true);
    }
    if (app.ui.status.clone(), app.ui.status_error) != before {
        f.insert(settings_sync::MESSAGE.into(), json!({"text": app.ui.status, "warning": app.ui.status_error}));
    }
}

/// Choose…: ask for the folder, starting in the chosen one, else in iCloud Drive when this Mac has it.
fn choose_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    let chosen = app.session.prefs().sync.folder.clone();
    let icloud = std::env::var("HOME").ok().map(|h| format!("{h}/Library/Mobile Documents/com~apple~CloudDocs"));
    let initial_dir = Some(chosen).filter(|c| std::path::Path::new(c).is_dir()).or_else(|| icloud.filter(|p| std::path::Path::new(p).is_dir()));
    app.ask_file(FileDialogRequest::Folder { initial_dir }, |app, answer| match answer {
        FileDialogAnswer::FolderTo(path) => choose_folder(app, &path),
        _ => Err("the file dialog gave an unexpected answer".into()),
    })
}

/// Use `folder`: remember it here and for the other ArtCraft apps.
pub(super) fn choose_folder(app: &mut PhotocraftApp, folder: &str) -> Result<Value, String> {
    match app.session.choose_sync_folder(folder) {
        Ok(()) => {
            let message = crate::i18n::fmt(tl!("Sync folder: {path}"), &[("path", &tilde(folder))]);
            settings_sync::report(app, &message, false);
        }
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
    let reply = settings_sync::import_bytes(app, FILE_NAME, file.text.as_bytes()).inspect_err(|e| settings_sync::report(app, e, true))?;
    if let Some(d) = reply["dialog"].as_u64().and_then(|id| app.ui.dialog_mut(id)) {
        d.fields.insert("__label".into(), json!(tl!("Load Settings from Folder")));
        if let Some(by) = &file.by {
            d.fields.insert("savedBy".into(), json!(by));
        }
    }
    Ok(reply)
}

/// The prompt's line "Saved on <Mac>, <time>"; nothing for a file that doesn't say.
pub(super) fn saved_line(ui: &mut egui::Ui, f: &Map<String, Value>) {
    let Some(by) = f.get("savedBy").and_then(|v| serde_json::from_value::<SavedBy>(v.clone()).ok()) else { return };
    let t = Tokens::get(ui.ctx());
    let line = crate::i18n::fmt(tl!("Saved on {computer}, {when}"), &[("computer", &by.computer), ("when", &by.when_text())]);
    ui.add_space(2.0);
    ui.add(egui::Label::new(RichText::new(line).color(t.text_dim)).wrap());
}

/// OK on the prompt of a file from the folder: this Mac now has that file.
pub(super) fn note_loaded(app: &mut PhotocraftApp, f: &Map<String, Value>) {
    if let Some(by) = f.get("savedBy").and_then(|v| serde_json::from_value::<SavedBy>(v.clone()).ok()) {
        app.session.note_synced(&by);
    }
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
