//! Chords (two-step shortcuts, ⌘K then C) and When conditions of shortcuts. Both are kept beside
//! the single-key bindings: [`Preferences::chords`] (command id → `"Cmd+K C"`) and
//! [`Preferences::shortcut_when`] (command id → condition text, limiting every key of the
//! command). Old settings files hold neither and load unchanged.

use super::shortcuts::{normalize_shortcut, reserved_shortcut};
use super::{BTreeMap, Preferences, Value};
use crate::when::{self, Expr};

/// The spelling of a chord all code compares: two steps, each canonical, one space between.
/// `None` for anything else (one step, three steps, a step that isn't a key).
pub fn normalize_chord(s: &str) -> Option<String> {
    let mut steps = s.split_whitespace();
    let (first, second) = (steps.next()?, steps.next()?);
    if steps.next().is_some() {
        return None;
    }
    Some(format!("{} {}", normalize_shortcut(first)?, normalize_shortcut(second)?))
}

/// The two canonical steps of a chord text.
pub fn chord_steps(s: &str) -> Option<[String; 2]> {
    let n = normalize_chord(s)?;
    let (a, b) = n.split_once(' ')?;
    Some([a.to_string(), b.to_string()])
}

/// A chord this build can bind: canonical spelling, or why not (a step the system takes is refused).
pub fn check_chord(s: &str) -> Result<String, String> {
    let n = normalize_chord(s).ok_or_else(|| format!("`{s}` is not a chord (two shortcuts with a space between, e.g. Cmd+K C)"))?;
    match n.split(' ').find(|step| reserved_shortcut(step)) {
        Some(step) => Err(format!("`{step}` is reserved by the system and can't be part of a chord")),
        None => Ok(n),
    }
}

/// A condition's canonical text, `""` for none; an error says what is wrong.
pub fn check_when(s: &str) -> Result<String, String> {
    when::parse(s).map(|e| e.map(|e| e.text()).unwrap_or_default()).map_err(|e| format!("`{s}` is not a condition: {e}"))
}

/// The parsed condition of `id` in `map` (`None` = always). A text that doesn't parse is ignored,
/// as if there were none ([`damaged`] lists them).
pub fn when_of(map: &BTreeMap<String, String>, id: &str) -> Option<Expr> {
    map.get(id).and_then(|t| when::parse(t).ok().flatten())
}

/// The saved conditions that can't be read: command id, text, what is wrong.
pub fn damaged(map: &BTreeMap<String, String>) -> Vec<(String, String, String)> {
    map.iter().filter_map(|(id, t)| Some((id.clone(), t.clone(), when::parse(t).err()?.message))).collect()
}

/// Whether a key of `a` and a key of `b` can both work at once (no condition on either = always).
pub fn can_overlap(map: &BTreeMap<String, String>, a: &str, b: &str) -> bool {
    when::expr::overlap(when_of(map, a).as_ref(), when_of(map, b).as_ref()).is_some()
}

/// Settings paths `chords`, `chords.<id>`, `shortcutWhen`, `shortcutWhen.<id>`. `Some(result)` when
/// `path` is one of them.
pub(super) fn set(p: &mut Preferences, path: &str, value: &Value) -> Option<Result<(), String>> {
    let (chords, id) = match path {
        "chords" | "shortcutWhen" => (path == "chords", None),
        _ => match (path.strip_prefix("chords."), path.strip_prefix("shortcutWhen.")) {
            (Some(id), _) => (true, Some(id)),
            (_, Some(id)) => (false, Some(id)),
            _ => return None,
        },
    };
    Some(match id {
        Some(id) => set_one(p, chords, id, value),
        None => set_all(p, chords, value),
    })
}

fn checked(chords: bool, v: &Value) -> Result<Option<String>, String> {
    let text = match v {
        Value::Null => return Ok(None),
        Value::String(s) => s.as_str(),
        _ => return Err(format!("{} must be text", if chords { "a chord" } else { "a condition" })),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    if chords { check_chord(text).map(Some) } else { check_when(text).map(|t| (!t.is_empty()).then_some(t)) }
}

fn set_one(p: &mut Preferences, chords: bool, id: &str, value: &Value) -> Result<(), String> {
    let new = checked(chords, value).map_err(|e| format!("`{id}`: {e}"))?;
    let map = if chords { &mut p.chords } else { &mut p.shortcut_when };
    match new {
        Some(t) => {
            map.insert(id.to_string(), t);
        }
        None => {
            map.remove(id);
        }
    }
    Ok(())
}

fn set_all(p: &mut Preferences, chords: bool, value: &Value) -> Result<(), String> {
    let obj = value.as_object().ok_or_else(|| format!("`{}` maps command ids to text", if chords { "chords" } else { "shortcutWhen" }))?;
    let mut map = BTreeMap::new();
    for (id, v) in obj {
        if let Some(t) = checked(chords, v).map_err(|e| format!("`{id}`: {e}"))? {
            map.insert(id.clone(), t);
        }
    }
    if chords {
        p.chords = map;
    } else {
        p.shortcut_when = map;
    }
    Ok(())
}

/// Reset of `chords.<id>` / `shortcutWhen.<id>`: true when `path` was one of them.
pub(super) fn reset(p: &mut Preferences, path: &str) -> bool {
    if let Some(id) = path.strip_prefix("chords.") {
        p.chords.remove(id);
    } else if let Some(id) = path.strip_prefix("shortcutWhen.") {
        p.shortcut_when.remove(id);
    } else {
        return false;
    }
    true
}

/// `conflicts` (exact same shortcut on several commands) kept only where the commands' conditions
/// can hold together, plus the chords: two commands on one chord, and a chord whose first step is
/// also a command's own key (the key would fire before the chord could finish). Each entry is the
/// shortcut text and the commands that clash on it.
pub fn clashes(p: &Preferences, bindings: &[(&str, String)]) -> Vec<(String, Vec<String>)> {
    let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut push = |key: String, id: &str| {
        let ids = by.entry(key).or_default();
        if !ids.iter().any(|i| i == id) {
            ids.push(id.to_string());
        }
    };
    for (id, sc) in bindings {
        if let Some(n) = normalize_shortcut(sc) {
            push(n, id);
        }
    }
    for (id, chord) in &p.chords {
        if let Some([first, second]) = chord_steps(chord) {
            push(format!("{first} {second}"), id);
            // the leader also clashes with a single key of the same spelling
            if bindings.iter().any(|(other, sc)| other != id && normalize_shortcut(sc).as_deref() == Some(first.as_str())) {
                push(first.clone(), id);
            }
        }
    }
    let mut out = Vec::new();
    for (sc, ids) in by {
        let keep: Vec<String> = ids.iter().filter(|a| ids.iter().any(|b| a != &b && can_overlap(&p.shortcut_when, a, b))).cloned().collect();
        if keep.len() > 1 {
            out.push((sc, keep));
        }
    }
    out
}

#[cfg(test)]
mod tests;
