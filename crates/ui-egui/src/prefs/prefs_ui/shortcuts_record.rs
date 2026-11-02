//! Recording a shortcut in Keyboard Shortcuts, the way VS Code does it. A click on a shortcut (or
//! Add Shortcut) starts recording; each key combination pressed is added to what is recorded,
//! two at most, so a chord is two combinations in a row (⌘K then ⌘S). The field shows the
//! recording as it grows and the clash warning shows with it. One combination is a key; two are a
//! chord. The first combination is applied to the command at once (the official dialog did, and its
//! tests read the result without an Enter); the recording stays open for a second one. A second
//! combination takes the key back, because the command is getting a chord; Esc takes it back too.
//! Enter ends the recording and keeps what is there; a click elsewhere ends it the same way.

use std::collections::BTreeMap;

use egui::{Key, Modifiers};
use serde_json::{Map, Value, json};

use super::shortcuts::{capture_key_press, reserved_shortcut, shortcut_text};
use super::{prefs, shortcuts_chord};
use crate::keybinds::chord;

pub(super) type Item = (String, String, Vec<String>, Option<String>);

/// The combinations a recording holds: a key, or a chord's two.
pub(super) const MAX_STEPS: usize = 2;

/// A command's main key as the dialog stands: its edited key (`""` removes it) or its default.
pub(super) fn effective(overrides: &BTreeMap<String, String>, id: &str, default: &Option<String>) -> Option<String> {
    match overrides.get(id) {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s.clone()),
        None => default.clone(),
    }
}

/// A command's further keys as the dialog stands: its edited list, else its defaults.
pub(super) fn effective_alts(alternates: &BTreeMap<String, Vec<String>>, id: &str, main: Option<&str>) -> Vec<String> {
    let mut p = prefs::Preferences::default();
    if let Some(list) = alternates.get(id) {
        p.alternate_shortcuts.insert(id.to_string(), list.clone());
    }
    p.alternates(id, main)
}

/// What has been recorded so far (the dialog's `__recSteps` field).
pub(super) fn load_steps(f: &Map<String, Value>) -> Vec<String> {
    f.get("__recSteps").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// The recording as the field shows it: `⌘K ⌘S`.
pub(super) fn shown(steps: &[String]) -> String {
    chord::text(&steps.join(" "))
}

/// What one frame of recording found.
pub(super) enum Poll {
    /// Nothing, or a modifier key on its own.
    Idle,
    Cancel,
    Accept,
    /// A key the system takes before the app sees it.
    Reserved(String),
}

/// The dialog field holding what a recording replaced.
const BEFORE: &str = "__recBefore";

/// What the selected command held when the recording started: its main key and its further keys as
/// the dialog's maps had them (none = no entry, the default applies).
struct Before {
    command: String,
    main: Option<String>,
    alternates: Option<Vec<String>>,
}

impl Before {
    fn load(f: &Map<String, Value>, command: &str) -> Option<Self> {
        let v = f.get(BEFORE)?;
        if v.get("command")?.as_str()? != command {
            return None;
        }
        let main = v.get("main").and_then(Value::as_str).map(str::to_string);
        let alternates = v.get("alternates").and_then(|a| serde_json::from_value(a.clone()).ok());
        Some(Self { command: command.to_string(), main, alternates })
    }

    fn store(&self, f: &mut Map<String, Value>) {
        f.insert(BEFORE.into(), json!({"command": self.command, "main": self.main, "alternates": self.alternates}));
    }

    /// The command's keys as they were.
    fn restore(&self, st: &mut Staged) {
        match &self.main {
            Some(main) => st.overrides.insert(self.command.clone(), main.clone()),
            None => st.overrides.remove(&self.command),
        };
        match &self.alternates {
            Some(list) => st.alternates.insert(self.command.clone(), list.clone()),
            None => st.alternates.remove(&self.command),
        };
    }
}

/// Read this frame's key press. A combination is added to `steps`; a third one starts over.
/// `mods` holds the modifiers pressed since the last combination.
pub(super) fn poll(ui: &egui::Ui, mods: &mut Modifiers, steps: &mut Vec<String>) -> Poll {
    let (held, pressed) = ui.input(|i| capture_key_press(*mods, i));
    *mods = held;
    let Some((key, m)) = pressed else { return Poll::Idle };
    ui.input_mut(|i| i.consume_key(m, key));
    if key == Key::Escape {
        return Poll::Cancel;
    }
    if key == Key::Enter && m.is_none() {
        return Poll::Accept;
    }
    let Some(step) = shortcut_text(key, m) else { return Poll::Idle };
    if reserved_shortcut(&step) {
        return Poll::Reserved(step);
    }
    if steps.len() >= MAX_STEPS {
        steps.clear();
    }
    steps.push(step);
    // The next combination starts from the modifiers held then, not from these.
    *mods = Modifiers::NONE;
    Poll::Idle
}

/// The dialog's bindings a recording reads and changes.
pub(super) struct Staged<'a> {
    pub overrides: &'a mut BTreeMap<String, String>,
    pub alternates: &'a mut BTreeMap<String, Vec<String>>,
    pub chords: &'a mut BTreeMap<String, String>,
    pub whens: &'a BTreeMap<String, String>,
}

/// The recording and the dialog's message line.
pub(super) struct Recording<'a> {
    /// Add Shortcut: a further key, where a click on the shortcut replaces the main key.
    pub add: bool,
    pub selected: &'a str,
    pub mods: &'a mut Modifiers,
    pub steps: &'a mut Vec<String>,
    pub conflict: &'a mut Option<Value>,
    pub message: &'a mut String,
    pub warning: &'a mut bool,
}

/// One frame of recording for the selected command. False once it has ended (accepted, cancelled
/// or refused).
pub(super) fn run(ui: &egui::Ui, f: &mut Map<String, Value>, items: &[Item], st: &mut Staged, rec: &mut Recording) -> bool {
    let steps_before = rec.steps.clone();
    let before = Before::load(f, rec.selected);
    match poll(ui, rec.mods, rec.steps) {
        Poll::Idle => {}
        Poll::Cancel => {
            if let Some(before) = before {
                before.restore(st);
            }
            f.remove(BEFORE);
            rec.steps.clear();
            *rec.conflict = None;
            rec.message.clear();
            *rec.warning = false;
            return false;
        }
        Poll::Accept => {
            let steps = std::mem::take(rec.steps);
            f.remove(BEFORE);
            // One combination is already applied; two are a chord, committed here.
            if let [first, second] = steps.as_slice() {
                commit_chord(f, items, st, rec, (first, second));
            }
            return false;
        }
        Poll::Reserved(step) => {
            if let Some(before) = before {
                before.restore(st);
            }
            f.remove(BEFORE);
            *rec.message =
                crate::i18n::fmt(tl!("{shortcut} is reserved by the system and can't be assigned."), &[("shortcut", &crate::shortcuts::pretty(&step))]);
            *rec.warning = true;
            rec.steps.clear();
            return false;
        }
    }
    if *rec.steps != steps_before {
        // The command's keys as they were before this recording touched them.
        let before = before.unwrap_or_else(|| Before {
            command: rec.selected.to_string(),
            main: st.overrides.get(rec.selected).cloned(),
            alternates: st.alternates.get(rec.selected).cloned(),
        });
        before.store(f);
        let steps = rec.steps.clone();
        match steps.as_slice() {
            [key] => apply_key(f, items, st, rec, (key, &before)),
            // A chord in the making: the first combination is not the command's key after all.
            _ => before.restore(st),
        }
    }
    if !rec.steps.is_empty() {
        let ids = holders(items, st, rec.selected, rec.steps);
        *rec.warning = !ids.is_empty();
        *rec.message = if ids.is_empty() { String::new() } else { clash_sentence(items, &ids, &shown(rec.steps)) };
    }
    true
}

/// The commands that hold the recorded key (or chord) in the same context.
fn holders(items: &[Item], st: &Staged, selected: &str, steps: &[String]) -> Vec<String> {
    let other = |id: &str| id != selected && prefs::can_overlap(st.whens, id, selected);
    match steps {
        [key] => items
            .iter()
            .filter(|(id, _, _, default)| {
                let main = effective(st.overrides, id, default);
                other(id)
                    && (main.as_deref().and_then(prefs::normalize_shortcut).as_deref() == Some(key.as_str())
                        || effective_alts(st.alternates, id, main.as_deref()).contains(key))
            })
            .map(|i| i.0.clone())
            .collect(),
        [_, _] => match prefs::check_chord(&steps.join(" ")) {
            Ok(c) => items.iter().filter(|i| other(&i.0) && st.chords.get(&i.0) == Some(&c)).map(|i| i.0.clone()).collect(),
            Err(_) => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// "⌘S is already in use by File › Save and will be removed from it when you click OK."
fn clash_sentence(items: &[Item], ids: &[String], shown_key: &str) -> String {
    let lang = crate::i18n::current();
    let names: Vec<String> = items
        .iter()
        .filter(|(id, ..)| ids.contains(id))
        .map(|(id, label, path, _)| {
            let path: Vec<&str> = path.iter().map(|p| crate::i18n::tr(lang, p)).collect();
            format!("{} › {}", path.join(" › "), crate::i18n::tr_id(lang, id, label).trim_end_matches('…'))
        })
        .collect();
    crate::i18n::fmt(
        tl!("{shortcut} is already in use by {commands} and will be removed from it when you click OK."),
        &[("shortcut", shown_key), ("commands", &names.join(", "))],
    )
}

/// A recording's first combination: the command's main key (or a further key with Add Shortcut once
/// it has a main key), applied at once. What it takes from other commands waits for OK, with the
/// Accept / Undo choice; `before` is what Undo Changes puts back.
fn apply_key(f: &mut Map<String, Value>, items: &[Item], st: &mut Staged, rec: &mut Recording, (key, before): (&str, &Before)) {
    let selected = rec.selected;
    // A second recording of the same row starts from what the first left: the keys as they were.
    before.restore(st);
    let ids = holders(items, st, selected, &[key.to_string()]);
    let default = items.iter().find(|i| i.0 == selected).and_then(|i| i.3.clone());
    let main = effective(st.overrides, selected, &default);
    *rec.conflict = ids.first().map(|first| {
        json!({
            "shortcut": key,
            "with": first,
            // Every command holding the key: Accept takes it from all of them.
            "holders": ids,
            "command": selected,
            "main": st.overrides.get(selected),
            "alternates": st.alternates.get(selected),
        })
    });
    if rec.add && main.is_some() {
        let mut keys = effective_alts(st.alternates, selected, main.as_deref());
        if main.as_deref().and_then(prefs::normalize_shortcut).as_deref() != Some(key) && !keys.iter().any(|k| k == key) && keys.len() < prefs::MAX_ALTERNATES {
            keys.push(key.to_string());
        }
        st.alternates.insert(selected.to_string(), keys);
    } else {
        st.overrides.insert(selected.to_string(), key.to_string());
    }
    f.insert("chordConflict".into(), Value::Null);
    *rec.warning = !ids.is_empty();
    *rec.message = if ids.is_empty() { String::new() } else { clash_sentence(items, &ids, &crate::shortcuts::pretty(key)) };
}

/// Enter on two combinations: the command's chord. What another command holds the same chord
/// loses when OK is clicked (the latest assignment wins).
fn commit_chord(f: &mut Map<String, Value>, items: &[Item], st: &mut Staged, rec: &mut Recording, (first, second): (&str, &str)) {
    let selected = rec.selected;
    let Ok(value) = prefs::check_chord(&format!("{first} {second}")) else {
        *rec.message =
            crate::i18n::fmt(tl!("{chord} can't be used: a key in it is reserved by the system."), &[("chord", &chord::text(&format!("{first} {second}")))]);
        *rec.warning = true;
        return;
    };
    let same = holders(items, st, selected, &[first.to_string(), second.to_string()]);
    let before = st.chords.insert(selected.to_string(), value.clone());
    let mut losers: Vec<String> = f.get("chordLosers").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    losers.retain(|l| l != selected);
    losers.extend(same.iter().cloned());
    f.insert("chordLosers".into(), json!(losers));
    *rec.conflict = None;
    *rec.warning = !same.is_empty();
    *rec.message = if same.is_empty() { String::new() } else { clash_sentence(items, &same, &shortcuts_chord::shown(&value)) };
    f.insert(
        "chordConflict".into(),
        if same.is_empty() { Value::Null } else { json!({"chord": value, "command": selected, "holders": same, "before": before}) },
    );
}
