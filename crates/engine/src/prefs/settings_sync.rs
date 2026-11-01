//! All of a user's settings in one `.pcsettings` file, to move to another Mac: Export Settings…
//! and Import Settings… (Edit › Preferences, and two icons in the Preferences window).
//!
//! The file is JSON with a `kind` and a `version`, and these parts, each optional on import:
//!
//! | part | holds |
//! |---|---|
//! | `preferences` | every section of Preferences (the fork's too), Top Bar items, History Options, Layer Mask Display Options |
//! | `colorSettings` | Edit › Color Settings |
//! | `shortcuts` | `bindings` (the Keyboard Shortcuts backup file, with chords and When), saved `sets`, the `activeSet` |
//! | `menus`, `toolbar` | Edit › Menus (hidden items, colours) and Edit › Toolbar |
//! | `contextMenus` | the arranged right-click menus (Preferences › Context Menus) |
//! | `workspaces` | saved `saved` layouts, `locked`, and the live panel `layout` |
//! | `remembered` | tool options, view extras, Color panel and Brush Picker options |
//! | `dictionary` | words added to the spelling dictionary |
//!
//! Left out because they only make sense on one Mac, or are history rather than settings: see
//! [`LEFT_OUT`] and [`NOT_IN_FILE`] (recent files, folders and disks, the graphics adapter and
//! memory limits, floating-window screen positions, the current brush and tool, script files).
//! Preset libraries (gradients, styles, shapes, patterns, swatches, tool presets, export and
//! document presets) are libraries, not settings: Edit › Presets › Export/Import Presets moves
//! them.
//!
//! An import reads the whole file first ([`plan`]) and changes nothing until [`Session::apply_settings_plan`]:
//! a value this build can't take (a wrong type, an unknown choice, a number out of range, a key
//! it can't press) or a setting it doesn't know is an error that names it and changes nothing.
//! A file made by a newer build takes what this build understands and names what it skipped.
//! A part the file doesn't have, and a setting a Preferences section doesn't list, stay as they are.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::cmds::bad;
use super::files::{file_target, read_text_file, rename_path, writable, write_files_atomically};
use super::{MenuCustomization, Preferences, SECTIONS, ToolbarCustomization};
use crate::color_cmds::ColorSettings;
use crate::commands::CommandSpec;
use crate::shortcut_sets::ShortcutSet;
use crate::{Result, Session};

/// What the file says it is.
pub const KIND: &str = "photocraft-settings";
/// The version this build writes and the newest it reads.
pub const VERSION: u64 = 1;
/// The file name extension.
pub const EXTENSION: &str = "pcsettings";
/// The biggest file read (a settings file is a few hundred kilobytes with saved workspaces).
pub const MAX_BYTES: usize = 8_000_000;

/// The parts of a file, in the order they are applied.
pub const PARTS: [&str; 9] = ["preferences", "colorSettings", "shortcuts", "menus", "toolbar", "contextMenus", "workspaces", "remembered", "dictionary"];

/// Preferences sections outside the Preferences window's list that the `preferences` part holds.
const EXTRA_SECTIONS: [&str; 2] = ["historyOptions", "layerMaskOverlay"];

/// `section.key` (or a whole section) of Preferences that stay out of the file, with the reason.
pub const LEFT_OUT: &[(&str, &str)] = &[
    ("fileHandling.recentFiles", "the recent files list"),
    ("fileHandling.lutFolder", "a folder on this Mac"),
    ("historyLog.filePath", "a file on this Mac"),
    ("scratchDisks", "disks of this Mac"),
    ("plugIns.additionalPluginsFolder", "a folder on this Mac"),
    ("performance.memoryUsageMb", "tied to this Mac's memory"),
    ("performance.gpuMemoryMb", "tied to this Mac's graphics"),
    ("performance.gpuBackend", "tied to this Mac's graphics"),
    ("performance.gpuAdapter", "tied to this Mac's graphics"),
    ("performance.cacheTileSize", "tied to this Mac's graphics"),
    ("performance.linuxDisplayServer", "Linux only"),
    ("integrations.controlPort", "a port of this Mac"),
];

/// Top-level keys of the preferences document that no part holds, with the reason.
pub const NOT_IN_FILE: &[(&str, &str)] = &[
    ("recentColors", "recent colors"),
    ("dialogs", "the last choices of dialogs"),
    ("lastBrush", "the brush in use"),
    ("toolBrushes", "the brush of each tool"),
    ("selectedTool", "the tool in use"),
    ("scriptEvents", "script files of this Mac"),
];

/// Top-level keys of the preferences document that a part other than `preferences` holds.
pub const PART_KEYS: &[&str] = &[
    "shortcuts",
    "alternateShortcuts",
    "chords",
    "shortcutWhen",
    "shortcutSets",
    "shortcutSet",
    "menus",
    "toolbar",
    "contextMenus",
    "workspaces",
    "workspaceLocked",
    "panelLayout",
    "colorPanel",
    "brushPicker",
    "toolOptions",
    "viewExtras",
    "userDictionary",
];

/// The shell's reading of a context menus value (see [`SettingsChecks::context_menus`]).
pub type ContextMenusCheck = fn(&Value) -> std::result::Result<(Value, Vec<String>), String>;

/// What only the shell can say about a file's contents. The shell installs its own in
/// [`Session::settings_checks`]; without them a session takes what the engine can check.
#[derive(Clone, Copy)]
pub struct SettingsChecks {
    /// Can the shell press this key (a canonical key such as `Cmd+K`)?
    pub key_ok: fn(&str) -> bool,
    /// Is this the name of a tool?
    pub tool_ok: fn(&str) -> bool,
    /// The context menus value as the shell would keep it, and what it had to leave out; an
    /// error for a value that is not a context menus value at all.
    pub context_menus: ContextMenusCheck,
}

impl Default for SettingsChecks {
    fn default() -> Self {
        Self {
            key_ok: |_| true,
            tool_ok: |_| true,
            context_menus: |v| {
                if v.is_null() || v.is_object() { Ok((v.clone(), Vec::new())) } else { Err("it must be an object or null".into()) }
            },
        }
    }
}

fn left_out(path: &str) -> Option<&'static str> {
    LEFT_OUT.iter().find(|(p, _)| path == *p || path.strip_prefix(*p).is_some_and(|rest| rest.starts_with('.'))).map(|(_, why)| *why)
}

fn section_ids() -> impl Iterator<Item = &'static str> {
    SECTIONS.iter().map(|(id, _)| *id).chain(EXTRA_SECTIONS)
}

fn looks_like_path(spec: &str) -> bool {
    let lower = spec.to_ascii_lowercase();
    spec.contains('/') || spec.contains('\\') || lower.ends_with(".icc") || lower.ends_with(".icm")
}

/// `v` without the screen positions of floating windows (`native`), which only fit the screen
/// they were made on.
fn without_screen_positions(v: &Value, depth: usize) -> Value {
    match v {
        Value::Object(m) if depth < 48 => {
            Value::Object(m.iter().filter(|(k, _)| k.as_str() != "native").map(|(k, x)| (k.clone(), without_screen_positions(x, depth + 1))).collect())
        }
        Value::Array(a) if depth < 48 => Value::Array(a.iter().map(|x| without_screen_positions(x, depth + 1)).collect()),
        other => other.clone(),
    }
}

/// The settings of `p` and `color` as the file.
pub fn to_json(p: &Preferences, color: &ColorSettings) -> Value {
    let all = p.to_json();
    let get = |k: &str| all.get(k).cloned().unwrap_or(Value::Null);
    let mut sections = Map::new();
    for id in section_ids() {
        if left_out(id).is_some() {
            continue;
        }
        let mut body = get(id);
        if let Some(m) = body.as_object_mut() {
            m.retain(|k, _| left_out(&format!("{id}.{k}")).is_none());
        }
        sections.insert(id.to_string(), body);
    }
    let mut colors = serde_json::to_value(color).unwrap_or(Value::Null);
    if let Some(m) = colors.as_object_mut() {
        m.remove("monitorProfile");
        m.retain(|k, v| !(k.starts_with("working") && v.as_str().is_some_and(looks_like_path)));
    }
    json!({
        "kind": KIND,
        "version": VERSION,
        "app": "PhotoCraft",
        "preferences": sections,
        "colorSettings": colors,
        "shortcuts": {"bindings": crate::shortcut_backup::to_json(p), "sets": get("shortcutSets"), "activeSet": get("shortcutSet")},
        "menus": get("menus"),
        "toolbar": get("toolbar"),
        "contextMenus": get("contextMenus"),
        "workspaces": {"saved": without_screen_positions(&get("workspaces"), 0), "locked": get("workspaceLocked"), "layout": without_screen_positions(&get("panelLayout"), 0)},
        "remembered": {"colorPanel": get("colorPanel"), "brushPicker": get("brushPicker"), "toolOptions": get("toolOptions"), "viewExtras": get("viewExtras")},
        "dictionary": get("userDictionary"),
    })
}

/// [`to_json`] as the file's text.
pub fn to_text(p: &Preferences, color: &ColorSettings) -> String {
    let mut text = serde_json::to_string_pretty(&to_json(p, color)).unwrap_or_default();
    text.push('\n');
    text
}

/// What an import will do, found out without changing anything.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The preferences after the import.
    pub prefs: Preferences,
    /// The colour settings after the import.
    pub color: ColorSettings,
    /// The parts the file replaces, as [`PARTS`] names them.
    pub parts: Vec<&'static str>,
    /// What was left out or skipped, one line each.
    pub notes: Vec<String>,
}

impl Plan {
    /// The reply of the commands.
    pub fn report(&self, applied: bool) -> Value {
        json!({"applied": applied, "parts": self.parts, "skipped": self.notes})
    }
}

/// How many notes are kept (a hand-made file could name thousands of unknown settings).
const MAX_NOTES: usize = 200;

struct Cx {
    strict: bool,
    problems: Vec<String>,
    notes: Vec<String>,
}

impl Cx {
    /// A value or setting this build can't take: an error in a file of this version, skipped in a
    /// newer one.
    fn bad(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.strict {
            self.problems.push(msg);
        } else {
            self.note(format!("Skipped {msg}"));
        }
    }

    fn note(&mut self, msg: impl Into<String>) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(msg.into());
        }
    }
}

/// The file as a JSON object and its version, refused when it is not ours.
fn header(text: &str) -> std::result::Result<(Map<String, Value>, u64), String> {
    if text.len() > MAX_BYTES {
        return Err("This file is too big to be a settings file".into());
    }
    let doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("This isn't a PhotoCraft settings file: it isn't readable JSON ({e})"))?;
    let Value::Object(obj) = doc else { return Err("This isn't a PhotoCraft settings file: it isn't a JSON object".into()) };
    match obj.get("kind").and_then(Value::as_str) {
        Some(KIND) => {}
        Some(other) => return Err(format!("This isn't a PhotoCraft settings file (its kind is “{}”)", other.chars().take(40).collect::<String>())),
        None => return Err("This isn't a PhotoCraft settings file (it has no \"kind\")".into()),
    }
    match obj.get("version").and_then(Value::as_u64) {
        Some(v) if v >= 1 => Ok((obj, v)),
        _ => Err("This settings file has no readable \"version\"".into()),
    }
}

/// Read `text` as a settings file over `p` and `color`, and say what the import would do. A file
/// that can't be taken is an error and nothing else happens: [`Plan`] is all a caller keeps.
pub fn plan(p: &Preferences, color: &ColorSettings, text: &str, checks: &SettingsChecks) -> std::result::Result<Plan, String> {
    plan_allowing(p, color, text, checks, &|_| true)
}

/// [`plan`] for a session that is not fully trusted: `allow` says whether a preference
/// (`section.key`, or `colorSettings`) may be set from here. What it refuses is left out and named.
pub fn plan_allowing(
    p: &Preferences,
    color: &ColorSettings,
    text: &str,
    checks: &SettingsChecks,
    allow: &dyn Fn(&str) -> bool,
) -> std::result::Result<Plan, String> {
    let (file, version) = header(text)?;
    let mut cx = Cx { strict: version <= VERSION, problems: Vec::new(), notes: Vec::new() };
    if version > VERSION {
        cx.note(format!("This file was made by a newer PhotoCraft (file version {version}; this one reads up to {VERSION}); what it understands was taken"));
    }
    for key in file.keys().filter(|k| !matches!(k.as_str(), "kind" | "version" | "app") && !PARTS.contains(&k.as_str())) {
        cx.bad(format!("“{}”: not a part of a settings file", key.chars().take(40).collect::<String>()));
    }
    let mut next = p.clone();
    let mut next_color = color.clone();
    let mut parts = Vec::new();
    for part in PARTS {
        let Some(v) = file.get(part) else { continue };
        parts.push(part);
        match part {
            "preferences" => read_preferences(&mut next, v, allow, &mut cx),
            "colorSettings" if !allow("colorSettings") => cx.note("Left out colorSettings (not allowed from here)"),
            "colorSettings" => read_color(&mut next_color, v, &mut cx),
            "shortcuts" => read_shortcuts(&mut next, v, checks, &mut cx),
            "menus" => read_menus(&mut next, v, &mut cx),
            "toolbar" => read_toolbar(&mut next, v, checks, &mut cx),
            "contextMenus" => match (checks.context_menus)(v) {
                Ok((kept, left)) => {
                    next.context_menus = kept;
                    for item in left {
                        cx.note(format!("Context menus: left out {item}"));
                    }
                }
                Err(e) => cx.bad(format!("contextMenus: {e}")),
            },
            "workspaces" => read_workspaces(&mut next, v, &mut cx),
            "remembered" => read_remembered(&mut next, v, &mut cx),
            _ => read_dictionary(&mut next, v, &mut cx),
        }
    }
    if let Some(first) = cx.problems.first() {
        let more = cx.problems.len().saturating_sub(1);
        let tail = if more > 0 { format!(" (and {more} more)") } else { String::new() };
        return Err(format!("This settings file can't be imported: {first}{tail}. Nothing was changed."));
    }
    if parts.is_empty() {
        return Err("This settings file holds no settings".into());
    }
    Ok(Plan { prefs: next, color: next_color, parts, notes: cx.notes })
}

/// Keys of `file` that `now` (the value as this build reads it back) doesn't have.
fn unknown_inside(file: &Value, now: &Value, path: &str, cx: &mut Cx, depth: usize) {
    if depth > 8 {
        return;
    }
    if let (Value::Object(f), Value::Object(n)) = (file, now) {
        for (k, fv) in f {
            let here = format!("{path}.{k}");
            match n.get(k) {
                None => cx.bad(format!("{here}: unknown setting")),
                Some(nv) => unknown_inside(fv, nv, &here, cx, depth + 1),
            }
        }
    }
}

fn read_preferences(next: &mut Preferences, v: &Value, allow: &dyn Fn(&str) -> bool, cx: &mut Cx) {
    let Some(sections) = v.as_object() else {
        cx.bad("preferences: must be an object");
        return;
    };
    for (section, body) in sections {
        if !section_ids().any(|id| id == section) {
            cx.bad(format!("preferences.{section}: unknown section"));
            continue;
        }
        if let Some(why) = left_out(section) {
            cx.note(format!("Left out {section} ({why})"));
            continue;
        }
        let Some(body) = body.as_object() else {
            cx.bad(format!("preferences.{section}: must be an object"));
            continue;
        };
        for (key, value) in body {
            let path = format!("{section}.{key}");
            if let Some(why) = left_out(&path) {
                cx.note(format!("Left out {path} ({why})"));
                continue;
            }
            if !allow(&path) {
                cx.note(format!("Left out {path} (not allowed from here)"));
                continue;
            }
            match next.set(&path, value.clone()) {
                Ok(()) => {
                    if let Some(now) = next.get(&path) {
                        unknown_inside(value, &now, &path, cx, 0);
                    }
                }
                Err(e) => cx.bad(e),
            }
        }
    }
}

fn read_color(next: &mut ColorSettings, v: &Value, cx: &mut Cx) {
    let Some(file) = v.as_object() else {
        cx.bad("colorSettings: must be an object");
        return;
    };
    let Ok(Value::Object(mut cur)) = serde_json::to_value(&*next) else { return };
    for (k, val) in file {
        if k == "monitorProfile" {
            cx.note("Left out colorSettings.monitorProfile (the display of this Mac)");
        } else if k.starts_with("working") && val.as_str().is_some_and(looks_like_path) {
            // A file of profile paths can't be shared between computers, and a settings file never names a file to read.
            cx.bad(format!("colorSettings.{k}: a profile file can't be part of a settings file"));
        } else if cur.contains_key(k) {
            cur.insert(k.clone(), val.clone());
        } else {
            cx.bad(format!("colorSettings.{k}: unknown setting"));
        }
    }
    match serde_json::from_value::<ColorSettings>(Value::Object(cur)) {
        Ok(c) => match crate::color_cmds::validate_settings(&c) {
            Ok(()) => *next = c,
            Err(e) => cx.bad(format!("colorSettings: {e}")),
        },
        Err(e) => cx.bad(format!("colorSettings: {e}")),
    }
}

/// Complain about keys of `obj` outside `allowed`.
fn only_keys(obj: &Map<String, Value>, allowed: &[&str], what: &str, cx: &mut Cx) {
    for k in obj.keys().filter(|k| !allowed.contains(&k.as_str())) {
        cx.bad(format!("{what}.{}: unknown setting", k.chars().take(40).collect::<String>()));
    }
}

fn read_shortcuts(next: &mut Preferences, v: &Value, checks: &SettingsChecks, cx: &mut Cx) {
    let Some(obj) = v.as_object() else {
        cx.bad("shortcuts: must be an object");
        return;
    };
    only_keys(obj, &["bindings", "sets", "activeSet"], "shortcuts", cx);
    if let Some(bindings) = obj.get("bindings") {
        let key_ok = checks.key_ok;
        let read =
            serde_json::to_string(bindings).map_err(|e| e.to_string()).and_then(|text| crate::shortcut_backup::import_text_with(next, &text, &|k| key_ok(k)));
        match read {
            Ok(report) => {
                for n in report.notices {
                    cx.note(n);
                }
            }
            Err(e) => cx.bad(format!("shortcuts.bindings: {e}")),
        }
    }
    if let Some(sets) = obj.get("sets") {
        match serde_json::from_value::<BTreeMap<String, ShortcutSet>>(sets.clone()) {
            Ok(sets) => {
                let mut fine = true;
                for (name, set) in &sets {
                    if let Err(e) = crate::shortcut_sets::set_name(name).and_then(|_| set.validate()) {
                        cx.bad(format!("shortcuts.sets.{}: {e}", name.chars().take(40).collect::<String>()));
                        fine = false;
                    }
                }
                if fine {
                    if !next.shortcut_set.is_empty() && !sets.contains_key(&next.shortcut_set) {
                        next.shortcut_set.clear();
                    }
                    next.shortcut_sets = sets;
                }
            }
            Err(e) => cx.bad(format!("shortcuts.sets: {e}")),
        }
    }
    if let Some(active) = obj.get("activeSet") {
        match active.as_str() {
            Some(name) if name.is_empty() || next.shortcut_sets.contains_key(name) => next.shortcut_set = name.to_string(),
            Some(name) => cx.bad(format!("shortcuts.activeSet: there is no set called “{}”", name.chars().take(40).collect::<String>())),
            None => cx.bad("shortcuts.activeSet: must be text"),
        }
    }
}

fn read_menus(next: &mut Preferences, v: &Value, cx: &mut Cx) {
    const COLORS: [&str; 7] = ["red", "orange", "yellow", "green", "blue", "violet", "gray"];
    if let Some(obj) = v.as_object() {
        only_keys(obj, &["hidden", "colors"], "menus", cx);
    }
    match serde_json::from_value::<MenuCustomization>(v.clone()) {
        Ok(mut menus) => {
            if let Some((id, c)) = menus.colors.iter().find(|(_, c)| !COLORS.contains(&c.as_str())) {
                cx.bad(format!("menus.colors.{id}: unknown colour “{}”", c.chars().take(20).collect::<String>()));
                return;
            }
            let before = menus.hidden.len();
            menus.hidden.retain(|id| MenuCustomization::hideable(id));
            if menus.hidden.len() < before {
                cx.note("Menus: some items can't be hidden and stay shown");
            }
            next.menus = menus;
        }
        Err(e) => cx.bad(format!("menus: {e}")),
    }
}

fn read_toolbar(next: &mut Preferences, v: &Value, checks: &SettingsChecks, cx: &mut Cx) {
    if let Some(obj) = v.as_object() {
        only_keys(obj, &["hidden", "order"], "toolbar", cx);
    }
    match serde_json::from_value::<ToolbarCustomization>(v.clone()) {
        Ok(mut toolbar) => {
            let before = toolbar.hidden.len() + toolbar.order.len();
            toolbar.hidden.retain(|t| (checks.tool_ok)(t));
            toolbar.order.retain(|t| (checks.tool_ok)(t));
            if toolbar.hidden.len() + toolbar.order.len() < before {
                cx.note("Toolbar: left out tools this PhotoCraft doesn't have");
            }
            next.toolbar = toolbar;
        }
        Err(e) => cx.bad(format!("toolbar: {e}")),
    }
}

/// `v` as an object or null (the shell owns what is inside).
fn object_or_null(v: &Value, what: &str, cx: &mut Cx) -> Option<Value> {
    if v.is_null() || v.is_object() {
        Some(v.clone())
    } else {
        cx.bad(format!("{what}: must be an object"));
        None
    }
}

fn read_workspaces(next: &mut Preferences, v: &Value, cx: &mut Cx) {
    let Some(obj) = v.as_object() else {
        cx.bad("workspaces: must be an object");
        return;
    };
    only_keys(obj, &["saved", "locked", "layout"], "workspaces", cx);
    if let Some(saved) = obj.get("saved") {
        match saved.as_object() {
            Some(m) if m.values().all(Value::is_object) && m.keys().all(|k| !k.is_empty() && k.chars().count() <= 100) => {
                next.workspaces = m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            }
            _ => cx.bad("workspaces.saved: must map layout names to layouts"),
        }
    }
    if let Some(locked) = obj.get("locked") {
        match locked.as_bool() {
            Some(b) => next.workspace_locked = b,
            None => cx.bad("workspaces.locked: must be true or false"),
        }
    }
    if let Some(layout) = obj.get("layout").and_then(|l| object_or_null(l, "workspaces.layout", cx)) {
        next.panel_layout = layout;
    }
}

fn read_remembered(next: &mut Preferences, v: &Value, cx: &mut Cx) {
    let Some(obj) = v.as_object() else {
        cx.bad("remembered: must be an object");
        return;
    };
    only_keys(obj, &["colorPanel", "brushPicker", "toolOptions", "viewExtras"], "remembered", cx);
    for (key, slot) in [
        ("colorPanel", &mut next.color_panel),
        ("brushPicker", &mut next.brush_picker),
        ("toolOptions", &mut next.tool_options),
        ("viewExtras", &mut next.view_extras),
    ] {
        if let Some(value) = obj.get(key).and_then(|x| object_or_null(x, &format!("remembered.{key}"), cx)) {
            *slot = value;
        }
    }
}

fn read_dictionary(next: &mut Preferences, v: &Value, cx: &mut Cx) {
    match serde_json::from_value::<Vec<String>>(v.clone()) {
        Ok(words) if words.iter().all(|w| w.chars().count() <= 200) => next.user_dictionary = words,
        _ => cx.bad("dictionary: must be a list of words"),
    }
}

impl Session {
    /// The settings file for what is set now.
    pub fn settings_text(&self) -> String {
        to_text(self.prefs(), &self.color.settings)
    }

    /// Read a settings file against the settings now, changing nothing ([`plan`], with the
    /// shell's checks when it installed them).
    pub fn plan_settings_import(&self, text: &str) -> std::result::Result<Plan, String> {
        // A session that is not fully trusted (an agent over MCP) sets only what `prefs.set` lets it set.
        let allow = |path: &str| self.authorize.is_none_or(|auth| auth("prefs.set", &json!({"path": path, "value": null})).is_ok());
        plan_allowing(self.prefs(), &self.color.settings, text, &self.settings_checks.unwrap_or_default(), &allow)
    }

    /// Make a [`Plan`] from [`Session::plan_settings_import`] the settings, in one step.
    pub fn apply_settings_plan(&mut self, plan: Plan) {
        self.color.settings = plan.color;
        photocraft_compose::psblend::set_text_gamma(self.color.settings.blend_text_gamma);
        self.prefs.edit(|p| *p = plan.prefs);
        self.apply_prefs();
    }
}

fn export_settings(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "edit.preferences.exportSettings";
    let target = file_target(cmd, Some(p))?.flatten();
    let text = s.settings_text();
    let mut out = json!({"parts": PARTS, "leftOut": LEFT_OUT.iter().map(|(path, _)| *path).chain(NOT_IN_FILE.iter().map(|(k, _)| *k)).collect::<Vec<_>>()});
    match target {
        Some(path) => {
            writable(&path).map_err(|e| bad(cmd, format!("{path}: {e}")))?;
            write_files_atomically(&[(path.as_str(), text.as_str())], &rename_path).map_err(|e| bad(cmd, e))?;
            out["path"] = json!(path);
        }
        None => out["text"] = json!(text),
    }
    Ok(out)
}

fn import_settings(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "edit.preferences.importSettings";
    let text = match (p.get("text").and_then(Value::as_str), p.get("path").and_then(Value::as_str)) {
        (Some(text), _) => text.to_string(),
        (None, Some(path)) => read_text_file(path, MAX_BYTES).map_err(|e| bad(cmd, format!("{path}: {e}")))?,
        _ => return Err(bad(cmd, "give `text` (the file's text) or `path` (a .pcsettings file)")),
    };
    let plan = s.plan_settings_import(&text).map_err(|e| bad(cmd, e))?;
    if p.get("dryRun").and_then(Value::as_bool) == Some(true) {
        return Ok(plan.report(false));
    }
    let report = plan.report(true);
    s.apply_settings_plan(plan);
    Ok(report)
}

/// The two commands.
pub fn specs() -> Vec<CommandSpec> {
    fn always(_: &Session) -> std::result::Result<(), String> {
        Ok(())
    }
    vec![
        CommandSpec {
            id: "edit.preferences.exportSettings",
            label: "Export Settings…",
            menu: &["Edit", "Preferences"],
            shortcut: None,
            params: r##"{"path":"<.pcsettings file>"?} (without `path` the file's text comes back as `text`); all settings in one file: Preferences, Color Settings, shortcuts, menus, toolbar, context menus, workspaces, dictionary"##,
            enabled: always,
            run: export_settings,
            journal: false,
        },
        CommandSpec {
            id: "edit.preferences.importSettings",
            label: "Import Settings…",
            menu: &["Edit", "Preferences"],
            shortcut: None,
            params: r##"{"path":"<.pcsettings file>"|"text":"<the file's text>","dryRun":bool=false} (the whole file is checked first: a bad value or unknown setting is an error and nothing changes; the reply names the parts replaced and what was skipped)"##,
            enabled: always,
            run: import_settings,
            journal: false,
        },
    ]
}

#[cfg(test)]
#[path = "settings_sync/tests.rs"]
mod tests;
