//! Export and Import of the user's keyboard shortcuts as a JSON file, a backup to keep or move to
//! another computer. It lists every customised command with the parts the user changed: its main
//! key (`""` = removed), further keys, chord and When condition; what is absent stays at its
//! default. A file that is not ours, or newer than this build, is refused with a clear message
//! and changes nothing; entries this build can't use (an unknown command, a key, chord or
//! condition that doesn't parse) are skipped with a notice and the rest is taken.
//!
//! `edit.keyboardShortcuts` takes `exportJson` (true or `{path}`) and `importJson` (the file's
//! text, or `{path}`); the Keyboard Shortcuts dialog calls the same functions.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::prefs::{
    MAX_ALTERNATES, Preferences, alternate_keys, bindable_items, check_chord, check_when, default_alternates, normalize_shortcut, reserved_shortcut,
};

/// What the file says it is.
pub const KIND: &str = "photocraft-shortcuts";
/// The version this build writes and the newest it reads.
pub const VERSION: u64 = 1;
/// The biggest file read (a keymap is a few kilobytes).
pub const MAX_BYTES: usize = 4_000_000;

fn is_bindable(id: &str) -> bool {
    bindable_items().any(|i| i.0 == id)
}

/// The label of a command for messages: `Copy (edit.copy)`.
fn name(id: &str) -> String {
    bindable_items().find(|i| i.0 == id).map_or(id.to_string(), |i| format!("{} ({id})", i.1))
}

/// The customised bindings of `p` as the file: one entry per command the user changed.
pub fn to_json(p: &Preferences) -> Value {
    let mut commands: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for (id, sc) in &p.shortcuts {
        if is_bindable(id) {
            commands.entry(id.clone()).or_default().insert("shortcut".into(), json!(sc));
        }
    }
    for (id, keys) in &p.alternate_shortcuts {
        if is_bindable(id) || default_alternates(id).next().is_some() {
            commands.entry(id.clone()).or_default().insert("alternates".into(), json!(keys));
        }
    }
    for (id, chord) in &p.chords {
        if is_bindable(id) {
            commands.entry(id.clone()).or_default().insert("chord".into(), json!(chord));
        }
    }
    for (id, when) in &p.shortcut_when {
        if is_bindable(id) {
            commands.entry(id.clone()).or_default().insert("when".into(), json!(when));
        }
    }
    json!({"kind": KIND, "version": VERSION, "app": "PhotoCraft", "shortcuts": commands})
}

/// What an import did: how many commands it set and what it skipped.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub commands: usize,
    pub notices: Vec<String>,
}

impl Report {
    pub fn to_json(&self) -> Value {
        json!({"imported": self.commands, "notices": self.notices})
    }
}

/// Read the file text, checking only that it is ours: its `shortcuts` object.
fn header(text: &str) -> Result<Map<String, Value>, String> {
    if text.len() > MAX_BYTES {
        return Err("This file is too big to be a shortcuts file".into());
    }
    let doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("This isn't a PhotoCraft shortcuts file: it isn't readable JSON ({e})"))?;
    match doc.get("kind").and_then(Value::as_str) {
        Some(KIND) => {}
        Some(other) => return Err(format!("This isn't a PhotoCraft shortcuts file (its kind is “{other}”)")),
        None => return Err("This isn't a PhotoCraft shortcuts file (it has no \"kind\")".into()),
    }
    match doc.get("version").and_then(Value::as_u64) {
        Some(v) if (1..=VERSION).contains(&v) => {}
        Some(v) if v > VERSION => {
            return Err(format!(
                "This shortcuts file was made by a newer PhotoCraft (file version {v}; this PhotoCraft reads up to {VERSION}). Update PhotoCraft to read it."
            ));
        }
        _ => return Err("This shortcuts file has no readable \"version\"".into()),
    }
    doc.get("shortcuts").and_then(Value::as_object).cloned().ok_or_else(|| "This shortcuts file has no \"shortcuts\" list".to_string())
}

/// A single key this build accepts, canonical, or why not. `key_ok` is the shell's own say on
/// whether it can press that key (the engine only knows the spelling).
fn checked_key(sc: &str, key_ok: &dyn Fn(&str) -> bool) -> Result<String, String> {
    let n = normalize_shortcut(sc).filter(|n| !sc.contains(char::is_whitespace) && key_ok(n)).ok_or("not a shortcut")?;
    if reserved_shortcut(&n) { Err("it is reserved by the system".into()) } else { Ok(n) }
}

/// A chord whose steps are both keys `key_ok` accepts.
fn checked_chord(text: &str, key_ok: &dyn Fn(&str) -> bool) -> Result<String, String> {
    let chord = check_chord(text)?;
    if chord.split(' ').all(key_ok) { Ok(chord) } else { Err(format!("`{text}` has a step that is not a key")) }
}

/// Read a shortcuts file over `p`: every customisation of a command this build knows is replaced
/// by the file's (a command the file doesn't list is back at its defaults); keys of things that
/// aren't commands (the actions' function keys) are kept. A refused file changes nothing.
pub fn import_text(p: &mut Preferences, text: &str) -> Result<Report, String> {
    import_text_with(p, text, &|_| true)
}

/// [`import_text`], with `key_ok` (given a canonical key like `Cmd+K`) vetoing keys the shell
/// can't press.
pub fn import_text_with(p: &mut Preferences, text: &str, key_ok: &dyn Fn(&str) -> bool) -> Result<Report, String> {
    let list = header(text)?;
    let known = |id: &str| is_bindable(id);
    p.shortcuts.retain(|id, _| !known(id));
    p.alternate_shortcuts.retain(|id, _| !known(id) && default_alternates(id).next().is_none());
    p.chords.retain(|id, _| !known(id));
    p.shortcut_when.retain(|id, _| !known(id));
    let mut report = Report::default();
    for (id, entry) in &list {
        if !known(id) {
            report.notices.push(format!("Skipped {id}: not a command of this PhotoCraft"));
            continue;
        }
        let Some(entry) = entry.as_object() else {
            report.notices.push(format!("Skipped {}: its entry isn't an object", name(id)));
            continue;
        };
        let mut used = false;
        if let Some(v) = entry.get("shortcut") {
            match v.as_str() {
                Some("") => {
                    p.shortcuts.insert(id.clone(), String::new());
                    used = true;
                }
                Some(sc) => match checked_key(sc, key_ok) {
                    Ok(n) => {
                        p.shortcuts.insert(id.clone(), n);
                        used = true;
                    }
                    Err(why) => report.notices.push(format!("Skipped the shortcut “{sc}” of {}: {why}", name(id))),
                },
                None => report.notices.push(format!("Skipped the shortcut of {}: it isn't text", name(id))),
            }
        }
        if let Some(v) = entry.get("alternates") {
            let keys = alternate_keys(v).map_err(|e| e.to_string());
            match keys.and_then(|k| k.into_iter().map(|s| checked_key(&s, key_ok)).collect::<Result<Vec<_>, _>>()) {
                Ok(keys) if keys.len() <= MAX_ALTERNATES => {
                    p.alternate_shortcuts.insert(id.clone(), keys);
                    used = true;
                }
                Ok(_) => report.notices.push(format!("Skipped the further shortcuts of {}: at most {MAX_ALTERNATES}", name(id))),
                Err(why) => report.notices.push(format!("Skipped the further shortcuts of {}: {why}", name(id))),
            }
        }
        for (field, is_chord) in [("chord", true), ("when", false)] {
            let Some(v) = entry.get(field) else { continue };
            let Some(text) = v.as_str() else {
                report.notices.push(format!("Skipped the {field} of {}: it isn't text", name(id)));
                continue;
            };
            let checked = if is_chord { checked_chord(text, key_ok) } else { check_when(text) };
            match checked {
                Ok(t) if t.is_empty() => {}
                Ok(t) => {
                    let map = if is_chord { &mut p.chords } else { &mut p.shortcut_when };
                    map.insert(id.clone(), t);
                    used = true;
                }
                Err(why) => report.notices.push(format!("Skipped the {field} of {}: {why}", name(id))),
            }
        }
        report.commands += usize::from(used);
    }
    Ok(report)
}

#[cfg(test)]
mod tests;
