//! A backup file of the arranged menus: every menu's arrangement and the three presets as one
//! `.json` file (the Export and Import icons in Preferences › Context Menus).
//!
//! ```json
//! { "kind": "photocraft-context-menus", "version": 1,
//!   "menus": { "layers.row": { "items": ["cmd:layer.duplicate", "---"], "hidden": ["layer.delete"] } },
//!   "presets": { "slots": [{ "tab.document": { "items": [], "hidden": ["file.closeAll"] } }, null, null], "active": 0 } }
//! ```
//!
//! An import checks the whole file before it changes anything: a file of another kind, one a newer
//! PhotoCraft wrote, or one that is not a backup is refused with a message and nothing changes. Menus
//! and items this build does not know (a menu key or command from a newer version, a repeated item)
//! are left out, and the message names them. `presets` is optional: a file without it leaves the
//! presets as they are.

use std::collections::HashSet;

use serde_json::{Value, json};

use super::presets::{COUNT, Presets, Set};
use super::{Arrangement, Custom, MAX_ITEMS, Slot, catalog, menus};
use crate::PhotocraftApp;

/// The file's `kind`.
pub const KIND: &str = "photocraft-context-menus";
/// The file's `version`: the newest this build reads and the one it writes.
pub const VERSION: u64 = 1;
/// A backup is a few kilobytes; a bigger file is not one.
const MAX_BYTES: usize = 1 << 20;
/// The suggested file name.
pub const FILE_NAME: &str = "Context Menus.json";

/// The file's text for `custom`.
pub fn render(custom: &Custom) -> Result<String, String> {
    let mut file = json!({"kind": KIND, "version": VERSION, "menus": super::presets::menus_to_value(&custom.menus)});
    if !custom.presets.is_empty() {
        file["presets"] = json!({"slots": custom.presets.to_value()["slots"], "active": custom.presets.active});
    }
    serde_json::to_string_pretty(&file).map(|s| s + "\n").map_err(|e| e.to_string())
}

/// What a file holds, checked.
#[derive(Debug, PartialEq)]
pub struct Parsed {
    pub menus: Set,
    /// None when the file has no `presets`.
    pub presets: Option<Presets>,
    /// Entries left out, as they were written.
    pub skipped: Vec<String>,
}

fn refuse(why: &str) -> String {
    format!("This file is not a PhotoCraft context menu backup ({why}).")
}

/// Check `text` as a backup file.
pub fn parse(text: &str) -> Result<Parsed, String> {
    let file: Value = serde_json::from_str(text).map_err(|_| refuse("it is not JSON"))?;
    let Some(obj) = file.as_object() else { return Err(refuse("it is not a JSON object")) };
    match obj.get("kind").and_then(Value::as_str) {
        Some(KIND) => {}
        Some(other) => return Err(refuse(&format!("its kind is \"{}\"", other.chars().take(40).collect::<String>()))),
        None => return Err(refuse("it has no kind")),
    }
    match obj.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        Some(v) if v > VERSION => {
            return Err(format!("This backup was made by a newer PhotoCraft (file version {v}; this one reads up to {VERSION}). Update PhotoCraft to import it."));
        }
        _ => return Err("This context menu backup has no valid version.".into()),
    }
    let Some(saved) = obj.get("menus").and_then(Value::as_object) else { return Err("This context menu backup has no `menus` object.".into()) };
    let mut skipped = Vec::new();
    let menus = clean_set(saved, &mut skipped);
    let presets = obj.get("presets").and_then(Value::as_object).map(|saved| {
        let slots: Vec<Option<Set>> = (0..COUNT)
            .map(|i| saved.get("slots").and_then(Value::as_array).and_then(|a| a.get(i)).and_then(Value::as_object).map(|set| clean_set(set, &mut skipped)))
            .collect();
        let active = saved.get("active").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).filter(|i| slots.get(*i).is_some_and(Option::is_some));
        Presets { slots: if slots.iter().all(Option::is_none) { Vec::new() } else { slots }, active, undo: None }
    });
    Ok(Parsed { menus, presets, skipped })
}

fn clean_set(saved: &serde_json::Map<String, Value>, skipped: &mut Vec<String>) -> Set {
    let mut out = Set::new();
    for (key, value) in saved {
        if !menus::is_key(key) {
            skipped.push(key.chars().take(60).collect());
            continue;
        }
        let a = clean(key, &Arrangement::from_value(value), skipped);
        if !a.items.is_empty() || !a.hidden.is_empty() {
            out.insert(key.clone(), a);
        }
    }
    out
}

/// The entries of `a` menu `key` can show, as they are saved; the others go to `skipped`.
fn clean(key: &str, a: &Arrangement, skipped: &mut Vec<String>) -> Arrangement {
    let universe: HashSet<&str> = menus::universe(key).into_iter().collect();
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for (n, s) in a.items.iter().enumerate() {
        if n >= MAX_ITEMS {
            skipped.push(format!("{} more", a.items.len() - MAX_ITEMS));
            break;
        }
        let known = Slot::parse(s).filter(|slot| match slot {
            Slot::Separator => true,
            Slot::Cmd(id) => universe.contains(id.as_str()) || (menus::can_add(key) && catalog::find(id).is_some()),
        });
        match known {
            Some(slot) if slot == Slot::Separator || seen.insert(slot.key()) => items.push(slot.key()),
            _ => skipped.push(s.chars().take(60).collect()),
        }
    }
    let hidden = a.hidden.iter().filter(|id| universe.contains(id.as_str())).cloned().collect();
    Arrangement { items, hidden }
}

/// What the notice says about `skipped`: the first few, then how many more.
pub fn skipped_note(skipped: &[String]) -> String {
    let shown: Vec<&str> = skipped.iter().take(4).map(String::as_str).collect();
    let more = skipped.len().saturating_sub(shown.len());
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    format!("{} unknown item{} skipped: {}{tail}", skipped.len(), if skipped.len() == 1 { "" } else { "s" }, shown.join(", "))
}

/// Make `text`'s menus (and presets, when it holds them) the arrangement. The reply names what was skipped.
pub fn import_text(app: &mut PhotocraftApp, text: &str) -> Result<Vec<String>, String> {
    if text.len() > MAX_BYTES {
        return Err("This file is too big to be a PhotoCraft context menu backup.".into());
    }
    let parsed = parse(text)?;
    let mut custom = super::load(app);
    custom.menus = parsed.menus;
    if let Some(p) = parsed.presets {
        custom.presets = p;
    }
    super::store(app, &custom);
    Ok(parsed.skipped)
}

fn say(app: &mut PhotocraftApp, text: String, error: bool) {
    app.ui.status = text;
    app.ui.status_error = error;
}

/// Write the backup to `path`, with the app's writer.
pub fn export_to(app: &mut PhotocraftApp, path: &str) -> Result<Value, String> {
    let text = render(&super::load(app))?;
    let write = app.services.write.as_mut().ok_or("no writer configured")?;
    write(path, text.as_bytes()).map_err(|e| format!("Could not write {path}: {e}"))?;
    let name = std::path::Path::new(path).file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
    say(app, format!("{} {name}", tl!("Exported the context menus to")), false);
    Ok(json!({"path": path}))
}

/// Read the backup in `bytes` (the file `name`).
pub fn import_bytes(app: &mut PhotocraftApp, name: &str, bytes: Vec<u8>) -> Result<Value, String> {
    let text = String::from_utf8(bytes).map_err(|_| refuse("it is not text")).inspect_err(|e| say(app, e.clone(), true))?;
    let skipped = import_text(app, &text).inspect_err(|e| say(app, e.clone(), true))?;
    let name = std::path::Path::new(name).file_name().map_or_else(|| name.to_string(), |n| n.to_string_lossy().into_owned());
    let mut note = format!("{} {name}", tl!("Imported the context menus from"));
    if !skipped.is_empty() {
        note = format!("{note}. {}", skipped_note(&skipped));
    }
    say(app, note, false);
    Ok(json!({"skipped": skipped}))
}

/// Export icon: ask where, then write.
pub fn export(app: &mut PhotocraftApp) {
    if let Err(e) = app.pick_save(FILE_NAME, |app, path| export_to(app, &path)) {
        say(app, e, true);
    }
}

/// Import icon: ask which file, then read it.
pub fn import(app: &mut PhotocraftApp) {
    if let Err(e) = app.pick_file_bytes_filtered(&["json"], |app, name, bytes| import_bytes(app, &name, bytes)) {
        say(app, e, true);
    }
}
