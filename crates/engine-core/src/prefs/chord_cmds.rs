//! The chord, When and JSON-backup parts of `edit.keyboardShortcuts`: `chords` and `when` change
//! single commands (`null` or `""` removes), `importJson` replaces the customisations with a
//! backup file's, `exportJson` makes the file. The reply lists `chords`, `when` and each
//! command's own.

use super::chords;
use super::cmds::bad;
use super::files::{file_target, read_text_file};
use super::shortcuts::bindable;
use super::{Preferences, Result, Value, json};

/// Apply `chords`, `when` and `importJson` of the call to `next`. The import goes first, so the
/// call's own `chords` and `when` apply on top of it. Returns whether anything changed and the
/// import report.
pub(super) fn apply(next: &mut Preferences, p: &Value, cmd: &str) -> Result<(bool, Option<Value>)> {
    let mut changed = false;
    let mut report = None;
    if let Some(v) = p.get("importJson").filter(|v| !v.is_null()) {
        let text = match v {
            Value::String(s) => s.clone(),
            Value::Object(o) => {
                let path =
                    o.get("path").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).ok_or_else(|| bad(cmd, "`importJson.path` is a shortcuts file"))?;
                read_text_file(path, crate::shortcut_backup::MAX_BYTES).map_err(|e| bad(cmd, format!("{path}: {e}")))?
            }
            _ => return Err(bad(cmd, "`importJson` is the text of a shortcuts file, or {\"path\": …}")),
        };
        // a refused file is an error and changes nothing: the import works on a copy
        let mut staged = next.clone();
        let r = crate::shortcut_backup::import_text(&mut staged, &text).map_err(|e| bad(cmd, e))?;
        *next = staged;
        report = Some(r.to_json());
        changed = true;
    }
    let allow_unknown = p.get("allowUnknown").and_then(Value::as_bool).unwrap_or(false);
    for (field, is_chord) in [("chords", true), ("when", false)] {
        let Some(v) = p.get(field).filter(|v| !v.is_null()) else { continue };
        let map = v.as_object().ok_or_else(|| bad(cmd, format!("`{field}` maps command ids to text")))?;
        // all of it is checked before any of it lands
        let mut staged = next.clone();
        for (id, text) in map {
            if !allow_unknown && !bindable().any(|(b, _)| b == id) {
                return Err(bad(cmd, format!("unknown command `{id}`")));
            }
            let path = format!("{}.{id}", if is_chord { "chords" } else { "shortcutWhen" });
            staged.set(&path, text.clone()).map_err(|e| bad(cmd, e))?;
        }
        *next = staged;
        changed |= !map.is_empty();
    }
    Ok((changed, report))
}

/// `exportJson`'s target: absent → `None`; true → the text in the reply; `{path}` → a file.
pub(super) fn export_target(cmd: &str, p: &Value) -> Result<Option<Option<String>>> {
    file_target(cmd, p.get("exportJson"))
}

/// The backup file's text for `prefs`.
pub(super) fn export_text(prefs: &Preferences) -> String {
    let mut text = serde_json::to_string_pretty(&crate::shortcut_backup::to_json(prefs)).unwrap_or_default();
    text.push('\n');
    text
}

/// Add a command's chord and When to its entry in the reply's `commands` list.
pub(super) fn describe(prefs: &Preferences, id: &str, entry: &mut Value) {
    if let Some(c) = prefs.chords.get(id) {
        entry["chord"] = json!(c);
    }
    if let Some(w) = prefs.shortcut_when.get(id) {
        entry["when"] = json!(w);
    }
}

/// Whether the command has a chord or a condition of its own (it is listed without `list: true`).
pub(super) fn customised(prefs: &Preferences, id: &str) -> bool {
    prefs.chords.contains_key(id) || prefs.shortcut_when.contains_key(id)
}

/// The conditions in the settings that can't be read, for the reply: `[{id, when, error}]`.
pub(super) fn damaged(prefs: &Preferences) -> Vec<Value> {
    chords::damaged(&prefs.shortcut_when).into_iter().map(|(id, when, error)| json!({"id": id, "when": when, "error": error})).collect()
}

#[cfg(test)]
mod tests;
