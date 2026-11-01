//! Export Settings… and Import Settings…: every setting in one `.pcsettings` file, to move to
//! another Mac. Two icons at the top right of Preferences and two commands in Edit › Preferences
//! (`edit.preferences.exportSettings`, `edit.preferences.importSettings`).
//!
//! The file, its checks and what it leaves out are the engine's
//! ([`photocraft_engine::prefs::settings_sync`]); here are the pieces only the shell can do: the
//! checks that need the shell (keys it can press, tool names, the context menus), the file
//! dialogs, the prompt that names what an import replaces, and bringing what is on screen in line
//! afterwards (the open Preferences window, panels, tool options). An import is checked whole
//! before the prompt; applying it is one step followed by a save.

use serde_json::{Map, Value, json};

use photocraft_engine::prefs::Preferences;
use photocraft_engine::prefs::settings_sync::{self, EXTENSION, MAX_BYTES, SettingsChecks};

use super::{DialogKind, PhotocraftApp, RichText, Tokens};

/// The suggested file name.
pub const FILE_NAME: &str = "PhotoCraft Settings.pcsettings";
/// Edit › Preferences › Export Settings….
pub const EXPORT: &str = "edit.preferences.exportSettings";
/// Edit › Preferences › Import Settings….
pub const IMPORT: &str = "edit.preferences.importSettings";
/// The marker of the prompt that names what an import replaces.
const PROMPT: &str = "settingsImport";
/// The Preferences window's one-line message about the last export or import.
const MESSAGE: &str = "__syncMessage";
const ICON: f32 = 22.0;

/// The checks the shell adds to the engine's.
pub fn checks() -> SettingsChecks {
    SettingsChecks {
        key_ok: |k| crate::shortcuts::parse(k).is_some(),
        tool_ok: |name| crate::state::Tool::ALL.iter().any(|t| format!("{t:?}") == name),
        context_menus: context_menus_check,
    }
}

/// The context menus value as [`crate::menu_custom`] keeps it, with the menus and items it can't
/// show named.
fn context_menus_check(v: &Value) -> Result<(Value, Vec<String>), String> {
    use crate::menu_custom::{Custom, backup};
    if v.is_null() {
        return Ok((Value::Null, Vec::new()));
    }
    if !v.is_object() {
        return Err("it must be an object or null".into());
    }
    let mut file = json!({"kind": backup::KIND, "version": backup::VERSION, "menus": v.get("menus").cloned().unwrap_or_else(|| json!({}))});
    if let Some(presets) = v.get("presets") {
        file["presets"] = presets.clone();
    }
    let parsed = backup::parse(&file.to_string())?;
    Ok((Custom { menus: parsed.menus, presets: parsed.presets.unwrap_or_default() }.to_value(), parsed.skipped))
}

/// Make the session use the shell's checks (once; a session starts without).
fn ensure_checks(app: &mut PhotocraftApp) {
    app.session.settings_checks.get_or_insert_with(checks);
}

// ------------------------------------------------------------------ the icons

/// The room the two icons take at the right of the search field.
pub(super) fn icons_width(ui: &egui::Ui) -> f32 {
    2.0 * ICON + 2.0 * ui.spacing().item_spacing.x
}

/// The two icon buttons: (Export clicked, Import clicked).
pub(super) fn icons(ui: &mut egui::Ui) -> (bool, bool) {
    let export = crate::icons::button(ui, "download", ICON, false, tl!("Export Settings…")).clicked();
    let import = crate::icons::button(ui, "upload", ICON, false, tl!("Import Settings…")).clicked();
    (export, import)
}

/// The last export or import's one line, under the search field; nothing while there is none.
pub(super) fn message_line(ui: &mut egui::Ui, f: &Map<String, Value>) {
    let Some(m) = f.get(MESSAGE).and_then(Value::as_object) else { return };
    let text = m.get("text").and_then(Value::as_str).unwrap_or("");
    if text.is_empty() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let color = if m.get("warning").and_then(Value::as_bool).unwrap_or(false) { t.warning } else { t.text_dim };
    ui.add_space(4.0);
    ui.add(egui::Label::new(RichText::new(text).color(color)).wrap());
}

/// Do what the icons asked, if anything.
pub(super) fn run(app: &mut PhotocraftApp, (export_it, import_it): (bool, bool)) {
    let result = if export_it {
        export_dialog(app)
    } else if import_it {
        import_dialog(app)
    } else {
        return;
    };
    if let Err(e) = result
        && e != crate::file_dialog::CANCELLED
    {
        report(app, &e, true);
    }
}

/// Say `text` in the status bar and in the open Preferences window.
pub(super) fn report(app: &mut PhotocraftApp, text: &str, warning: bool) {
    app.ui.status = text.to_string();
    app.ui.status_error = warning;
    if let Some(d) = app.ui.dialogs.iter_mut().find(|d| super::is_preferences(&d.fields)) {
        d.fields.insert(MESSAGE.into(), json!({"text": text, "warning": warning}));
    }
}

// ------------------------------------------------------------------ export

/// The settings as they stand, with what the open Preferences window shows in place of what is
/// saved (its sections and its context menus draft), the way the Keyboard Shortcuts export writes
/// the dialog's own fields.
pub(super) fn settings_now(app: &PhotocraftApp) -> (Preferences, photocraft_engine::color_cmds::ColorSettings) {
    let mut prefs = app.session.prefs().clone();
    if let Some(d) = app.ui.dialogs.iter().find(|d| super::is_preferences(&d.fields)) {
        let mut root = prefs.to_json();
        if let (Some(values), Some(obj)) = (d.fields.get("values").and_then(Value::as_object), root.as_object_mut()) {
            for (section, v) in values {
                obj.insert(section.clone(), v.clone());
            }
        }
        if let Ok(shown) = serde_json::from_value::<Preferences>(root) {
            prefs = shown;
        }
        if let Some(draft) = d.fields.get(crate::menu_custom::DRAFT) {
            prefs.context_menus = crate::menu_custom::Custom::from_value(draft).to_value();
        }
    }
    (prefs, app.session.color.settings.clone())
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
    app.pick_file_bytes_limited(&[EXTENSION, "json"], MAX_BYTES, |app, name, bytes| import_bytes(app, &name, &bytes).inspect_err(|e| report(app, e, true)))
}

/// A settings file's bytes: refused with the engine's reason, or checked whole and put to the user
/// as a prompt naming what it replaces. Nothing changes before the prompt is answered.
pub(crate) fn import_bytes(app: &mut PhotocraftApp, file_name: &str, bytes: &[u8]) -> Result<Value, String> {
    ensure_checks(app);
    let text = std::str::from_utf8(bytes).map_err(|_| tl!("This isn't a PhotoCraft settings file: it isn't text").to_string())?;
    let plan = app.session.plan_settings_import(text)?;
    let shown = std::path::Path::new(file_name).file_name().map_or_else(|| file_name.to_string(), |s| s.to_string_lossy().to_string());
    let mut f = Map::new();
    f.insert("__prefsui".into(), json!(PROMPT));
    f.insert("__label".into(), json!(tl!("Import Settings")));
    f.insert("file".into(), json!(shown));
    f.insert("text".into(), json!(text));
    f.insert("parts".into(), json!(plan.parts));
    f.insert("notes".into(), json!(plan.notes));
    let id = app.ui.open_dialog(DialogKind::Command, f);
    Ok(json!({"dialog": id, "parts": plan.parts, "skipped": plan.notes}))
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

/// The prompt's body: what the file replaces, and what it skips.
pub(super) fn prompt_body(app: &PhotocraftApp, ui: &mut egui::Ui, f: &Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let file = f.get("file").and_then(Value::as_str).unwrap_or("");
    ui.label(RichText::new(crate::i18n::fmt(tl!("Replace these settings with those in {file}?"), &[("file", file)])).color(t.text));
    ui.add_space(4.0);
    let parts: Vec<&str> = f.get("parts").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    for part in parts {
        ui.label(RichText::new(format!("•  {}", part_label(part))).color(t.text_dim));
    }
    let notes: Vec<&str> = f.get("notes").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    if let Some(first) = notes.first() {
        ui.add_space(6.0);
        let n = notes.len().to_string();
        let line = crate::i18n::fmt(tl!("{n} skipped: {first}"), &[("n", &n), ("first", first)]);
        ui.add(egui::Label::new(RichText::new(line).color(t.warning)).wrap()).on_hover_text(notes.join("\n"));
    }
    let discards = app.ui.dialogs.iter().any(|d| super::is_preferences(&d.fields) && super::preferences_changed(app, &d.fields));
    if discards {
        ui.add_space(6.0);
        ui.label(RichText::new(tl!("Unsaved changes in Preferences are discarded.")).color(t.text_faint));
    }
}

/// OK on the prompt: check the file again (the same answer as before the prompt), apply it in one
/// step, bring the screen in line and save.
pub(super) fn confirm(app: &mut PhotocraftApp, f: &Map<String, Value>) -> Result<Value, String> {
    // A person answers the prompt. An agent calls `edit.preferences.importSettings` with the text,
    // which the session's gate judges setting by setting.
    if app.automation_input || app.session.authorize.is_some() {
        return Err("Import Settings is confirmed by a person; call edit.preferences.importSettings with the file's text instead".into());
    }
    ensure_checks(app);
    let text = f.get("text").and_then(Value::as_str).unwrap_or("");
    let file = f.get("file").and_then(Value::as_str).unwrap_or("").to_string();
    let plan = app.session.plan_settings_import(text).inspect_err(|e| report(app, e, true))?;
    let report_json = plan.report(true);
    let skipped = plan.notes.len();
    app.session.apply_settings_plan(plan);
    after_import(app);
    let mut message = crate::i18n::fmt(tl!("Imported settings from {file}"), &[("file", &file)]);
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
    crate::color_panel_ui::restore(app);
    crate::brush_picker::restore(app);
    super::startup::sync_rulers(app);
    crate::tool_memory::restore(app);
    crate::view_cmds::restore(app);
    app.sync_recent();
    // A Keyboard Shortcuts window would put its old bindings back on OK.
    app.ui.dialogs.retain(|d| d.fields.get("__prefsui").and_then(Value::as_str) != Some("shortcuts"));
    let values = super::dialogs::preference_values(app.session.prefs());
    let draft = app.session.prefs().context_menus.clone();
    for d in app.ui.dialogs.iter_mut().filter(|d| super::is_preferences(&d.fields)) {
        d.fields.insert("values".into(), Value::Object(values.clone()));
        d.fields.insert(crate::menu_custom::DRAFT.into(), draft.clone());
    }
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
    let empty = params.as_object().is_none_or(Map::is_empty);
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
