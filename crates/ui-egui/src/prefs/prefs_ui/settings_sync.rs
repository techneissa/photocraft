//! Export Settings… and Import Settings…: every setting in one `.pcsettings` file, to move to
//! another Mac. Two buttons in Settings › General and two commands in Edit › Preferences
//! (`edit.preferences.exportSettings`, `edit.preferences.importSettings`).
//!
//! The file, its checks and what it leaves out are the engine's
//! ([`photocraft_engine::prefs::settings_sync`]); here are the pieces only the shell can do: the
//! checks that need the shell (keys it can press, tool names), the file dialogs, the prompt that
//! names what an import replaces, and bringing what is on screen in line afterwards (panels, tool
//! options). An import is checked whole before the prompt; applying it is one step followed by a
//! save.
//!
//! The Settings window runs inside the dialog host and has no app: its buttons file a [`Request`]
//! in the [`Bridge`] and `dialogs::take` does the work after the pass. The prompt shows inside the
//! window while it is open (the window is app-modal, so a prompt in the main window could not be
//! answered) and on the host otherwise.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use craft::theme::variant::Tone;
use craft::ui::dialog::{PromptAnswer, PromptChoice, StandardPrompt};
use serde_json::{Value, json};

use photocraft_engine::prefs::Preferences;
use photocraft_engine::prefs::settings_sync::{self, EXTENSION, MAX_BYTES, SettingsChecks};

use photocraft_engine::color_cmds::ColorExt;

use super::PhotocraftApp;
use super::settings_folder::SavedBy;

/// The suggested file name.
pub const FILE_NAME: &str = "PhotoCraft Settings.pcsettings";
/// Edit › Preferences › Export Settings….
pub const EXPORT: &str = "edit.preferences.exportSettings";
/// Edit › Preferences › Import Settings….
pub const IMPORT: &str = "edit.preferences.importSettings";

/// What the Settings window's buttons and prompts ask the app to do, after the pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Request {
    Export,
    Import,
    Choose,
    UseOffered,
    Save,
    Load,
    Keep(bool),
    CheckUpdate,
    Update,
}

/// The answer to the "save before closing?" prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseAnswer {
    Save,
    Discard,
}

/// What an import will do, as the prompt names it.
#[derive(Clone, Debug)]
pub(crate) struct ImportAsk {
    pub(super) file: String,
    text: String,
    parts: Vec<String>,
    notes: Vec<String>,
    unknown: Vec<String>,
    unknown_total: usize,
    /// Who saved the file and when, for a file from the sync folder.
    pub(super) saved_by: Option<SavedBy>,
}

/// What the window and the app say to each other (see the module docs).
#[derive(Default)]
pub(super) struct Bridge {
    pub(super) requests: Vec<Request>,
    /// The last export, import or sync's one line, and whether it is a warning.
    pub(super) message: Option<(String, bool)>,
    /// An import waiting for its answer inside the open window.
    pub(super) import: Option<ImportAsk>,
    /// The answer to an import prompt: the import to do, once.
    pub(super) import_confirmed: Option<ImportAsk>,
    /// The "save before closing?" prompt is up.
    pub(super) close_asking: bool,
    pub(super) close_answer: Option<CloseAnswer>,
}

/// The bridge, shared by the app and the open window.
pub(super) type Shared = Arc<Mutex<Bridge>>;

pub(super) fn lock(shared: &Shared) -> MutexGuard<'_, Bridge> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The checks the shell adds to the engine's.
pub fn checks() -> SettingsChecks {
    SettingsChecks {
        key_ok: |k| crate::shortcuts::parse(k).is_some(),
        tool_ok: |name| crate::state::Tool::ALL.iter().any(|t| format!("{t:?}") == name),
        ..SettingsChecks::default()
    }
}

/// Make the session use the shell's checks (once; a session starts without).
pub(super) fn ensure_checks(app: &mut PhotocraftApp) {
    app.session.settings_checks.get_or_insert_with(checks);
}

/// Do what the Settings window asked for, if anything.
pub(super) fn run(app: &mut PhotocraftApp, request: Request) {
    let result = match request {
        Request::Export => export_dialog(app),
        Request::Import => import_dialog(app),
        Request::Keep(on) => {
            app.session.prefs.edit(|p| p.sync.keep = on);
            return;
        }
        Request::CheckUpdate => {
            #[cfg(not(target_arch = "wasm32"))]
            crate::fork_update::run_settings(app, crate::fork_update::Click::Check);
            return;
        }
        Request::Update => {
            #[cfg(not(target_arch = "wasm32"))]
            crate::fork_update::run_settings(app, crate::fork_update::Click::Update);
            return;
        }
        other => super::settings_folder::run(app, other),
    };
    if let Err(e) = result
        && e != crate::file_dialog::CANCELLED
    {
        report(app, &e, true);
    }
}

/// Say `text` in the status bar and in the open Settings window.
pub(super) fn report(app: &mut PhotocraftApp, text: &str, warning: bool) {
    app.ui.status = text.to_string();
    app.ui.status_error = warning;
    lock(&app.prefs_rt.bridge).message = Some((text.to_string(), warning));
}

// ------------------------------------------------------------------ export

/// The settings as they stand, with what the open Settings window shows in place of what is saved
/// (its draft), the way the Keyboard Shortcuts export writes the dialog's own fields.
pub(super) fn settings_now(app: &PhotocraftApp) -> (Preferences, photocraft_engine::color_cmds::ColorSettings) {
    let mut prefs = app.session.prefs().clone();
    if let Some(Value::Object(shown)) = super::dialogs::draft_values(app) {
        let mut root = prefs.to_json();
        if let Some(obj) = root.as_object_mut() {
            for (section, v) in shown {
                obj.insert(section, v);
            }
        }
        if let Ok(shown) = serde_json::from_value::<Preferences>(root) {
            prefs = shown;
        }
    }
    (prefs, app.session.color().settings.clone())
}

/// Export Settings…: ask where, then write the file.
pub(super) fn export_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    let (prefs, color) = settings_now(app);
    let text = settings_sync::to_text(&prefs, &color);
    app.pick_save(FILE_NAME, move |app, path| {
        let write = app.services.write.as_mut().ok_or("no writer configured")?;
        write(&path, text.as_bytes()).map_err(|e| crate::i18n::fmt(tl!("Could not write {path}: {e}"), &[("path", &path), ("e", &e)]))?;
        let message = crate::i18n::fmt(tl!("Exported settings to {path}"), &[("path", &path)]);
        report(app, &message, false);
        Ok(json!({"path": path}))
    })
}

// ------------------------------------------------------------------ import

/// Import Settings…: ask which file, check it, then ask before replacing anything.
pub(super) fn import_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    app.pick_file_bytes_filtered(&[EXTENSION, "json"], |app, name, bytes| {
        if bytes.len() > MAX_BYTES {
            return Err(tl!("This isn't a PhotoCraft settings file: it is too big").to_string());
        }
        import_bytes(app, &name, &bytes).inspect_err(|e| report(app, e, true))
    })
}

/// A settings file's bytes: refused with the engine's reason, or checked whole and put to the user
/// as a prompt naming what it replaces. Nothing changes before the prompt is answered.
pub(crate) fn import_bytes(app: &mut PhotocraftApp, file_name: &str, bytes: &[u8]) -> Result<Value, String> {
    import_bytes_by(app, file_name, bytes, None)
}

/// [`import_bytes`] for a file the sync folder holds: the prompt names who saved it and when.
pub(super) fn import_bytes_by(app: &mut PhotocraftApp, file_name: &str, bytes: &[u8], saved_by: Option<SavedBy>) -> Result<Value, String> {
    ensure_checks(app);
    let text = std::str::from_utf8(bytes).map_err(|_| tl!("This isn't a PhotoCraft settings file: it isn't text").to_string())?;
    let plan = app.session.plan_settings_import(text)?;
    let shown = std::path::Path::new(file_name).file_name().map_or_else(|| file_name.to_string(), |s| s.to_string_lossy().to_string());
    let ask = ImportAsk {
        file: shown,
        text: text.to_string(),
        parts: plan.parts.iter().map(|p| part_label(p).to_string()).collect(),
        notes: plan.notes.clone(),
        unknown: plan.unknown.clone(),
        unknown_total: plan.unknown_total,
        saved_by,
    };
    let reply = json!({"parts": plan.parts, "skipped": plan.notes, "unknown": plan.unknown, "unknownCount": plan.unknown_total});
    open_ask(app, ask);
    Ok(reply)
}

/// Puts the prompt of `ask` where it can be answered: inside the open Settings window, else on the host.
pub(super) fn open_ask(app: &mut PhotocraftApp, ask: ImportAsk) {
    if super::window_open(app) {
        lock(&app.prefs_rt.bridge).import = Some(ask);
    } else {
        let prompt = prompt_of(&ask).tag(ask);
        let _ = crate::dialogs::alerts::open_prompt(app, prompt);
    }
}

/// What a part of the file is called to the user.
fn part_label(part: &str) -> &'static str {
    match part {
        "preferences" => tl!("Preferences"),
        "colorSettings" => tl!("Color Settings"),
        "shortcuts" => tl!("Keyboard Shortcuts"),
        "menus" => tl!("Menus"),
        "toolbar" => tl!("Toolbar"),
        "contextMenus" => tl!("Context Menus"),
        "workspaces" => tl!("Workspaces and Panels"),
        "remembered" => tl!("Tool Options and View Settings"),
        "dictionary" => tl!("Spelling Dictionary"),
        _ => "",
    }
}

/// How many unknown settings the prompt names before "and N more".
const NAMED: usize = 5;

/// The prompt's line for settings this build doesn't have: the first few names, then "and N more".
pub(super) fn unknown_line(names: &[&str], total: usize) -> String {
    let shown = names.iter().take(NAMED).copied().collect::<Vec<_>>().join(", ");
    let more = total.saturating_sub(names.len().min(NAMED));
    if more == 0 {
        crate::i18n::fmt(tl!("Not in this PhotoCraft, skipped: {names}"), &[("names", &shown)])
    } else {
        crate::i18n::fmt(tl!("Not in this PhotoCraft, skipped: {names} and {n} more"), &[("names", &shown), ("n", &more.to_string())])
    }
}

/// The prompt that names what the import replaces and what it skips.
pub(super) fn prompt_of(ask: &ImportAsk) -> StandardPrompt {
    let mut lines = vec![crate::i18n::fmt(tl!("Replace these settings with those in {file}?"), &[("file", &ask.file)])];
    if let Some(by) = &ask.saved_by {
        lines.push(crate::i18n::fmt(tl!("Saved on {computer}, {when}"), &[("computer", &by.computer), ("when", &by.when_text())]));
    }
    lines.extend(ask.parts.iter().map(|part| format!("•  {part}")));
    let mut prompt = StandardPrompt::custom(lines.remove(0)).title(tl!("Import Settings")).instance("settings-import");
    for line in lines {
        prompt = prompt.paragraph(line);
    }
    let unknown: Vec<&str> = ask.unknown.iter().map(String::as_str).collect();
    if !unknown.is_empty() {
        let total = ask.unknown_total.max(unknown.len());
        prompt = prompt.tone(Tone::Warn, unknown_line(&unknown, total));
    } else if let Some(first) = ask.notes.first() {
        let n = ask.notes.len().to_string();
        prompt = prompt.tone(Tone::Warn, crate::i18n::fmt(tl!("{n} skipped: {first}"), &[("n", &n), ("first", first)]));
    }
    prompt.button(PromptChoice::Proceed, tl!("OK")).button(PromptChoice::Cancel, tl!("Cancel"))
}

/// The `PROMPTS` entry: OK on the import prompt (the window was closed). `false` for another family's answer.
pub(crate) fn answer(app: &mut PhotocraftApp, _ctx: &egui::Context, answer: &mut PromptAnswer) -> bool {
    let Some(ask) = answer.take_tag::<ImportAsk>() else { return false };
    if answer.choice == PromptChoice::Proceed {
        let result = confirm(app, &ask);
        crate::dialogs::alerts::report(app, result);
    }
    true
}

/// OK on the prompt: check the file again (the same answer as before the prompt), apply it in one
/// step, bring the screen in line and save.
pub(super) fn confirm(app: &mut PhotocraftApp, ask: &ImportAsk) -> Result<Value, String> {
    // A person answers the prompt. An agent calls `edit.preferences.importSettings` with the text,
    // which the session's gate judges setting by setting.
    if app.automation_input || app.session.authorize.is_some() {
        return Err("Import Settings is confirmed by a person; call edit.preferences.importSettings with the file's text instead".into());
    }
    ensure_checks(app);
    let plan = app.session.plan_settings_import(&ask.text).inspect_err(|e| report(app, e, true))?;
    let report_json = plan.report(true);
    let skipped = plan.notes.len() + plan.unknown_total;
    app.session.apply_settings_plan(plan);
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(by) = &ask.saved_by {
        app.session.note_synced(by);
    }
    after_import(app);
    let mut message = crate::i18n::fmt(tl!("Imported settings from {file}"), &[("file", &ask.file)]);
    if skipped > 0 {
        message.push_str(&crate::i18n::fmt(tl!(" ({n} skipped)"), &[("n", &skipped.to_string())]));
    }
    // The save is part of the import; applying it stays even when the disk says no.
    if let Err(e) = super::save_preferences(app) {
        message = crate::i18n::fmt(tl!("Imported settings, but could not save them: {e}"), &[("e", &e)]);
        report(app, &message, true);
    } else {
        report(app, &message, skipped > 0);
    }
    Ok(report_json)
}

/// The settings are new in the session: bring what is on screen in line, the way a launch does.
pub(crate) fn after_import(app: &mut PhotocraftApp) {
    crate::dock::restore(app);
    crate::tool_memory::restore(app);
    super::startup::sync_rulers(app);
    app.sync_recent();
}

// ------------------------------------------------------------------ the commands

/// The shell's front end of the two commands. Without parameters they ask for a file (and the
/// prompt); with parameters the engine does the work (an agent's call: nothing to ask) and the
/// shell follows.
pub(super) fn invoke(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if id != EXPORT && id != IMPORT {
        return None;
    }
    ensure_checks(app);
    let empty = params.as_object().is_none_or(serde_json::Map::is_empty);
    if !empty {
        let result = app.session.execute(id, params.clone()).map_err(|e| e.to_string());
        if id == IMPORT && result.as_ref().is_ok_and(|r| r["applied"] == true) {
            after_import(app);
            let _ = super::save_preferences(app);
        }
        return Some(result);
    }
    Some(if id == EXPORT { export_dialog(app) } else { import_dialog(app) })
}
