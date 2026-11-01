//! Chords in Keyboard Shortcuts: a chord's chip beside a command's other keys, and the Accept /
//! Undo choice when a chord is already in use. A chord is recorded like a key (two combinations in
//! a row, see `shortcuts_record`). Chords live in the dialog's `chords` field (command id →
//! `Cmd+K C`) until OK, like the keys.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::{RichText, Tokens};
use crate::keybinds::chord;

pub(super) fn load_map(f: &Map<String, Value>) -> BTreeMap<String, String> {
    f.get("chords").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// A command's chord as a chip beside its other keys; a click on its × deletes it.
pub(super) fn chip(ui: &mut egui::Ui, t: &Tokens, id: &str, chords: &mut BTreeMap<String, String>, selected: &mut String) {
    let Some(c) = chords.get(id) else { return };
    let b = egui::Button::new(RichText::new(format!("{}  ×", chord::text(c))).size(12.0).color(t.accent_text)).min_size(egui::vec2(64.0, 18.0));
    if ui.add(b).on_hover_text(tl!("Remove Chord")).clicked() {
        chords.remove(id);
        *selected = id.to_string();
    }
}

/// Accept and Go to Conflict / Undo Changes, for a chord assigned that another command holds.
/// Drawn after the dialog's own conflict row; at most one of the two shows. The dialog's own
/// locals are passed in, since the dialog writes them back after this.
pub(super) fn conflict_row(
    ui: &mut egui::Ui,
    f: &mut Map<String, Value>,
    chords: &mut BTreeMap<String, String>,
    (selected, filter): (&mut String, &mut String),
    (message, warning): (&mut String, &mut bool),
) {
    let Some(c) = f.get("chordConflict").filter(|c| c.is_object()).cloned() else { return };
    let command = c["command"].as_str().unwrap_or_default().to_string();
    let stale = f.get("conflict").is_some_and(|x| x.is_object()) || chords.get(&command) != c["chord"].as_str().map(str::to_string).as_ref();
    if stale {
        f.insert("chordConflict".into(), Value::Null);
        return;
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Accept and Go to Conflict")).on_hover_text(tl!("Take the chord from the other command and select it")).clicked() {
            let holders: Vec<String> = serde_json::from_value(c["holders"].clone()).unwrap_or_default();
            holders.iter().for_each(|h| _ = chords.remove(h));
            *selected = holders.first().cloned().unwrap_or_default();
            filter.clear();
            f.insert("chordConflict".into(), Value::Null);
            *message = tl!("Choose a new shortcut for this command, or leave it without one.").to_string();
            *warning = false;
        } else if ui.button(tl!("Undo Changes")).on_hover_text(tl!("Put this command's chord back as it was")).clicked() {
            match c["before"].as_str() {
                Some(b) => _ = chords.insert(command.clone(), b.to_string()),
                None => _ = chords.remove(&command),
            }
            *selected = command.clone();
            f.insert("chordConflict".into(), Value::Null);
            message.clear();
            *warning = false;
        }
    });
}

/// The chords and conditions OK sends: a chord given to a command that another still held in the
/// same context is taken from the other (as a key moved to a command is taken from its old owner).
pub(super) fn at_ok(f: &Map<String, Value>) -> (Value, Value) {
    let mut chords = load_map(f);
    let losers: Vec<String> = f.get("chordLosers").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    for loser in &losers {
        let held = chords.get(loser).cloned();
        if held.is_some_and(|c| chords.iter().any(|(id, x)| id != loser && *x == c && !losers.contains(id))) {
            chords.remove(loser);
        }
    }
    (json!(chords), json!(super::shortcuts_when::load_map(f)))
}

/// A key or a chord as menus show it (`⌘K C`).
pub(super) fn shown(key: &str) -> String {
    chord::text(key)
}
