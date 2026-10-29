//! Chords in Keyboard Shortcuts: the Add Chord button, the key capture (the first key, then the
//! second), a chord's chip beside a command's other keys, and the Accept / Undo choice when a
//! chord is already in use. Chords live in the dialog's `chords` field (command id → `Cmd+K C`)
//! until OK, like the keys.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::shortcuts::{capture_key_press, capture_mods_field, capture_mods_json, dialog_keys, shortcut_items, shortcut_text};
use super::{PhotocraftApp, RichText, Tokens, prefs};
use crate::keybinds::chord;

pub(super) fn load_map(f: &Map<String, Value>) -> BTreeMap<String, String> {
    f.get("chords").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

fn save_map(f: &mut Map<String, Value>, chords: &BTreeMap<String, String>) {
    f.insert("chords".into(), json!(chords));
}

fn str_field(f: &Map<String, Value>, name: &str) -> String {
    f.get(name).and_then(Value::as_str).unwrap_or("").to_string()
}

fn say(f: &mut Map<String, Value>, text: String, warning: bool) {
    f.insert("message".into(), json!(text));
    f.insert("messageWarning".into(), json!(warning));
}

fn stop(f: &mut Map<String, Value>) {
    f.insert("captureChord".into(), json!(false));
    f.insert("chordFirst".into(), Value::Null);
    f.remove("captureMods");
}

/// The Add Chord button. True when it was clicked (key capture for a single key must stop).
pub(super) fn add_button(ui: &mut egui::Ui, f: &mut Map<String, Value>, enabled: bool) -> bool {
    let clicked = ui
        .add_enabled(enabled, egui::Button::new(tl!("Add Chord")))
        .on_hover_text(tl!("Give the selected command a two-key shortcut, such as ⌘K then C"))
        .clicked();
    if clicked {
        f.insert("captureChord".into(), json!(true));
        f.insert("chordFirst".into(), Value::Null);
    }
    clicked
}

/// A command's chord as a chip beside its other keys; a click on its × deletes it.
pub(super) fn chip(ui: &mut egui::Ui, t: &Tokens, id: &str, chords: &mut BTreeMap<String, String>, selected: &mut String) {
    let Some(c) = chords.get(id) else { return };
    let b = egui::Button::new(RichText::new(format!("{}  ×", chord::text(c))).size(12.0).color(t.accent_text)).min_size(egui::vec2(64.0, 18.0));
    if ui.add(b).on_hover_text(tl!("Delete Chord")).clicked() {
        chords.remove(id);
        *selected = id.to_string();
    }
}

type Item = (String, String, Vec<String>, Option<String>);

fn name_of(items: &[Item], id: &str) -> String {
    let label = items.iter().find(|i| i.0 == id).map_or(id, |i| i.1.as_str());
    crate::i18n::tr_id(crate::i18n::current(), id, label).trim_end_matches('…').to_string()
}

/// Key capture for a chord (while `captureChord` is on): the first key, then the second. Esc
/// cancels. Runs before the dialog reads its own fields, so its messages stand.
pub(super) fn capture(app: &mut PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>) {
    if f.get("captureChord").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let selected = str_field(f, "selected");
    if selected.is_empty() || f.get("tab").and_then(Value::as_u64) != Some(0) {
        return stop(f);
    }
    let first = f.get("chordFirst").and_then(Value::as_str).map(str::to_string);
    let (mods, pressed) = ui.input(|i| capture_key_press(capture_mods_field(f.get("captureMods")), i));
    f.insert("captureMods".into(), capture_mods_json(mods));
    let Some((key, m)) = pressed else {
        let prompt = match &first {
            Some(step) => crate::i18n::fmt(tl!("{key} … press the second key (Esc cancels)"), &[("key", &crate::shortcuts::pretty(step))]),
            None => tl!("Press the first key of the chord, then the second (Esc cancels).").to_string(),
        };
        return say(f, prompt, false);
    };
    ui.input_mut(|i| i.consume_key(m, key));
    if key == egui::Key::Escape {
        stop(f);
        return say(f, String::new(), false);
    }
    let Some(step) = shortcut_text(key, m) else { return };
    match first {
        None if matches!(key, egui::Key::Backspace | egui::Key::Delete) && m.is_none() => {
            let mut chords = load_map(f);
            chords.remove(&selected);
            save_map(f, &chords);
            stop(f);
            say(f, tl!("Chord removed.").to_string(), false);
        }
        None => {
            f.insert("chordFirst".into(), json!(step));
            f.remove("captureMods");
        }
        Some(first) => finish(app, f, &selected, &first, &step),
    }
}

/// Both keys are in: check the chord, store it, and say what it clashes with.
fn finish(app: &mut PhotocraftApp, f: &mut Map<String, Value>, selected: &str, first: &str, second: &str) {
    stop(f);
    let chord = match prefs::check_chord(&format!("{first} {second}")) {
        Ok(c) => c,
        Err(_) => {
            return say(
                f,
                crate::i18n::fmt(
                    tl!("{chord} can't be used: a key in it is reserved by the system."),
                    &[("chord", &chord::text(&format!("{first} {second}")))],
                ),
                true,
            );
        }
    };
    let items = shortcut_items(app);
    let mut chords = load_map(f);
    let whens = super::shortcuts_when::load_map(f);
    let overrides: BTreeMap<String, String> = f.get("overrides").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let alternates: BTreeMap<String, Vec<String>> = f.get("alternates").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let others = |keep: &dyn Fn(&Item) -> bool| -> Vec<String> {
        items.iter().filter(|i| i.0 != selected && prefs::can_overlap(&whens, &i.0, selected) && keep(i)).map(|i| i.0.clone()).collect()
    };
    let same: Vec<String> = others(&|i| chords.get(&i.0) == Some(&chord));
    let leads: Vec<String> = others(&|i| dialog_keys(&overrides, &alternates, &i.0, i.3.as_deref()).iter().any(|k| k == first));
    let before = chords.insert(selected.to_string(), chord.clone());
    save_map(f, &chords);
    // who loses their chord to this one when OK is clicked (the latest assignment wins)
    let mut losers: Vec<String> = f.get("chordLosers").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    losers.retain(|l| l != selected);
    losers.extend(same.iter().cloned());
    f.insert("chordLosers".into(), json!(losers));
    let shown = chord::text(&chord);
    let names = |ids: &[String]| ids.iter().map(|id| name_of(&items, id)).collect::<Vec<_>>().join(", ");
    let mut text = if same.is_empty() {
        crate::i18n::fmt(tl!("{chord} assigned."), &[("chord", &shown)])
    } else {
        crate::i18n::fmt(
            tl!("{chord} is already in use by {commands} and will be removed from it when you click OK."),
            &[("chord", &shown), ("commands", &names(&same))],
        )
    };
    if !leads.is_empty() {
        text.push(' ');
        text.push_str(&crate::i18n::fmt(
            tl!("{key} is also the shortcut of {commands}: it will wait for the chord's second key."),
            &[("key", &crate::shortcuts::pretty(first)), ("commands", &names(&leads))],
        ));
    }
    let clash = !same.is_empty() || !leads.is_empty();
    say(f, text, clash);
    let conflict = if same.is_empty() { Value::Null } else { json!({"chord": chord, "command": selected, "holders": same, "before": before}) };
    f.insert("chordConflict".into(), conflict);
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
            *message = tl!("Changes undone.").to_string();
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
