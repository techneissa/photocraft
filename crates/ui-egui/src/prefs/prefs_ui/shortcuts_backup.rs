//! Export and Import of the keyboard shortcuts as a JSON file (two icon buttons in the Set row of
//! Keyboard Shortcuts). The file, its checks and what it keeps are the engine's
//! ([`photocraft_engine::shortcut_backup`], also behind `edit.keyboardShortcuts {"exportJson",
//! "importJson"}`); here the dialog's own fields are what is written, and what is read lands in
//! them, so OK applies an import like hand-made changes and Cancel discards it.

use serde_json::{Map, Value, json};

use photocraft_engine::prefs::Preferences;
use photocraft_engine::shortcut_backup;

use super::shortcuts::save_text;
use super::{PhotocraftApp, shortcuts_chord, shortcuts_when};

/// The dialog's bindings as preferences, the shape the engine's backup reads and writes.
pub(super) fn staged(f: &Map<String, Value>) -> Preferences {
    Preferences {
        shortcuts: f.get("overrides").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        alternate_shortcuts: f.get("alternates").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        chords: shortcuts_chord::load_map(f),
        shortcut_when: shortcuts_when::load_map(f),
        ..Default::default()
    }
}

/// Write the staged bindings back into the dialog's fields.
fn unstage(f: &mut Map<String, Value>, p: &Preferences) {
    f.insert("overrides".into(), json!(p.shortcuts));
    f.insert("alternates".into(), json!(p.alternate_shortcuts));
    f.insert("chords".into(), json!(p.chords));
    f.insert("when".into(), json!(p.shortcut_when));
    f.insert("selected".into(), json!(""));
    f.insert("capture".into(), json!(false));
    f.insert("captureChord".into(), json!(false));
    f.insert("conflict".into(), Value::Null);
    f.insert("chordConflict".into(), Value::Null);
    f.insert("whenEdit".into(), Value::Null);
}

/// The two icon buttons: (Export clicked, Import clicked).
pub(super) fn icons(ui: &mut egui::Ui) -> (bool, bool) {
    let export = crate::icons::button(ui, "download", 22.0, false, tl!("Export Shortcuts as JSON…")).clicked();
    let import = crate::icons::button(ui, "upload", 22.0, false, tl!("Import Shortcuts from JSON…")).clicked();
    (export, import)
}

/// Do what the icons asked, if anything; a failure is the dialog's message.
pub(super) fn run(app: &mut PhotocraftApp, f: &Map<String, Value>, (export_it, import_it): (bool, bool), (message, warning): (&mut String, &mut bool)) {
    let result = if export_it {
        export(app, f)
    } else if import_it {
        import_dialog(app)
    } else {
        return;
    };
    if let Err(e) = result {
        *message = e;
        *warning = true;
    }
}

/// Export…: the bindings as the dialog stands, saved through the save panel.
pub(super) fn export(app: &mut PhotocraftApp, f: &Map<String, Value>) -> Result<Value, String> {
    let mut text = serde_json::to_string_pretty(&shortcut_backup::to_json(&staged(f))).map_err(|e| e.to_string())?;
    text.push('\n');
    save_text(app, "PhotoCraft Shortcuts.json", text)
}

/// Import…: ask for the file, then fill the dialog ([`import_bytes`]).
pub(super) fn import_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    app.pick_file_bytes_limited(&["json"], shortcut_backup::MAX_BYTES, |app, name, bytes| import_bytes(app, &name, &bytes))
}

/// A shortcuts file's bytes: refused with the engine's reason, or taken into the open dialog.
pub(crate) fn import_bytes(app: &mut PhotocraftApp, file_name: &str, bytes: &[u8]) -> Result<Value, String> {
    let id = match app.ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(Value::as_str) == Some("shortcuts")) {
        Some(d) => d.id,
        None => crate::prefs_ui::open_shortcuts(app, 0),
    };
    let shown = std::path::Path::new(file_name).file_name().map_or_else(|| file_name.to_string(), |s| s.to_string_lossy().to_string());
    let text = if bytes.len() > shortcut_backup::MAX_BYTES {
        Err("This file is too big to be a shortcuts file".to_string())
    } else {
        std::str::from_utf8(bytes).map_err(|_| "This isn't a PhotoCraft shortcuts file: it isn't text".to_string())
    };
    let result = text.and_then(|text| {
        let f = &app.ui.dialog_mut(id).ok_or("the Keyboard Shortcuts dialog is gone")?.fields;
        let mut p = staged(f);
        let key_ok = |k: &str| crate::shortcuts::parse(k).is_some();
        let report = shortcut_backup::import_text_with(&mut p, text, &key_ok)?;
        Ok((p, report))
    });
    let f = &mut app.ui.dialog_mut(id).ok_or("the Keyboard Shortcuts dialog is gone")?.fields;
    f.insert("tab".into(), json!(0));
    match result {
        Ok((p, report)) => {
            unstage(f, &p);
            let n = report.commands.to_string();
            let mut message = crate::i18n::fmt(tl!("Imported {n} commands from {file}. Click OK to keep them."), &[("n", &n), ("file", &shown)]);
            if let Some(first) = report.notices.first() {
                let more = report.notices.len().saturating_sub(1).to_string();
                message.push(' ');
                message.push_str(&crate::i18n::fmt(
                    tl!("{count} entries were skipped: {first} (and {more} more)"),
                    &[("count", &report.notices.len().to_string()), ("first", first), ("more", &more)],
                ));
            }
            f.insert("message".into(), json!(message));
            f.insert("messageWarning".into(), json!(!report.notices.is_empty()));
            Ok(report.to_json())
        }
        Err(e) => {
            f.insert("message".into(), json!(e));
            f.insert("messageWarning".into(), json!(true));
            Err(e)
        }
    }
}
