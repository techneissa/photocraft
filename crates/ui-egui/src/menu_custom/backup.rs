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
use crate::file_dialog::{FileDialogAnswer, FileDialogRequest, last_used_dir};

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
    crate::i18n::fmt(tl!("This file is not a PhotoCraft context menu backup ({why})."), &[("why", why)])
}

/// Check `text` as a backup file.
pub fn parse(text: &str) -> Result<Parsed, String> {
    let file: Value = serde_json::from_str(text).map_err(|_| refuse(tl!("it is not JSON")))?;
    let Some(obj) = file.as_object() else { return Err(refuse(tl!("it is not a JSON object"))) };
    match obj.get("kind").and_then(Value::as_str) {
        Some(KIND) => {}
        Some(other) => {
            let kind: String = other.chars().take(40).collect();
            return Err(refuse(&crate::i18n::fmt(tl!("its kind is \"{kind}\""), &[("kind", &kind)])));
        }
        None => return Err(refuse(tl!("it has no kind"))),
    }
    match obj.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        Some(v) if v > VERSION => {
            return Err(crate::i18n::fmt(
                tl!("This backup was made by a newer PhotoCraft (file version {v}; this one reads up to {max}). Update PhotoCraft to import it."),
                &[("v", &v.to_string()), ("max", &VERSION.to_string())],
            ));
        }
        _ => return Err(tl!("This context menu backup has no valid version.").to_string()),
    }
    let Some(saved) = obj.get("menus").and_then(Value::as_object) else { return Err(tl!("This context menu backup has no `menus` object.").to_string()) };
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
            skipped.push(crate::i18n::fmt(tl!("{n} more"), &[("n", &(a.items.len() - MAX_ITEMS).to_string())]));
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
    let tail = if more > 0 { crate::i18n::fmt(tl!(" and {n} more"), &[("n", &more.to_string())]) } else { String::new() };
    let template = if skipped.len() == 1 { tl!("{n} unknown item skipped: {items}") } else { tl!("{n} unknown items skipped: {items}") };
    crate::i18n::fmt(template, &[("n", &skipped.len().to_string()), ("items", &shown.join(", "))]) + &tail
}

/// Check `text` and make its menus (and presets, when it holds them) the arrangement `custom`
/// holds. The reply names what was skipped. Nothing changes when the file is refused.
pub fn import_into(custom: &mut Custom, text: &str) -> Result<Vec<String>, String> {
    if text.len() > MAX_BYTES {
        return Err(too_big());
    }
    let parsed = parse(text)?;
    custom.menus = parsed.menus;
    if let Some(p) = parsed.presets {
        custom.presets = p;
    }
    Ok(parsed.skipped)
}

fn too_big() -> String {
    tl!("This file is too big to be a PhotoCraft context menu backup.").to_string()
}

fn error(app: &mut PhotocraftApp, text: String) {
    app.ui.status = text;
    app.ui.status_error = true;
}

/// Write the backup of the arrangement being edited to `path`, with the app's writer.
pub fn export_to(app: &mut PhotocraftApp, path: &str) -> Result<Value, String> {
    let text = render(&super::current(app))?;
    let write = app.services.write.as_mut().ok_or_else(|| tl!("no writer configured").to_string())?;
    write(path, text.as_bytes()).map_err(|e| crate::i18n::fmt(tl!("Could not write {path}: {e}"), &[("path", path), ("e", &e)]))?;
    Ok(json!({"path": path}))
}

/// Read the backup in `bytes` into the arrangement being edited. A refused file changes nothing.
pub fn import_bytes(app: &mut PhotocraftApp, bytes: Vec<u8>) -> Result<Value, String> {
    let text = String::from_utf8(bytes).map_err(|_| refuse(tl!("it is not text"))).inspect_err(|e| error(app, e.clone()))?;
    let mut custom = super::current(app);
    let skipped = import_into(&mut custom, &text).inspect_err(|e| error(app, e.clone()))?;
    super::set_current(app, &custom);
    if !skipped.is_empty() {
        app.ui.status = skipped_note(&skipped);
        app.ui.status_error = false;
    }
    Ok(json!({"skipped": skipped}))
}

/// The name and bytes of the file an open dialog answered with, refusing one over [`MAX_BYTES`]
/// before it is read.
fn read_capped(answer: FileDialogAnswer) -> Result<Vec<u8>, String> {
    match answer {
        FileDialogAnswer::Paths(paths) => {
            let path = paths.into_iter().next().ok_or_else(|| "cancelled".to_string())?;
            let size = std::fs::metadata(&path).map_err(|e| format!("{path}: {e}"))?.len();
            if size > MAX_BYTES as u64 {
                return Err(too_big());
            }
            photocraft_format::read_file(std::path::Path::new(&path)).map_err(|e| format!("{path}: {e}"))
        }
        FileDialogAnswer::Contents(_, bytes) => Ok(bytes),
        FileDialogAnswer::ContentsMany(files) => files.into_iter().next().map(|(_, bytes)| bytes).ok_or_else(|| "cancelled".to_string()),
        _ => Err(tl!("no file was chosen").to_string()),
    }
}

/// Export icon: ask where, then write.
pub fn export(app: &mut PhotocraftApp) {
    if let Err(e) = app.pick_save(FILE_NAME, |app, path| export_to(app, &path)) {
        error(app, e);
    }
}

/// Import icon: ask which file, then read it.
pub fn import(app: &mut PhotocraftApp) {
    let request = FileDialogRequest::Open { multiple: false, initial_dir: last_used_dir(&app.ui.recent_files), extensions: Some(vec!["json".to_string()]) };
    if let Err(e) = app.ask_file(request, |app, answer| import_bytes(app, read_capped(answer)?)) {
        error(app, e);
    }
}
