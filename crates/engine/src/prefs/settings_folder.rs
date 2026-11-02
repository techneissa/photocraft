//! A sync folder for Settings sync (fork/settings-sync.md): Save to Folder and Load from Folder
//! keep every setting in `photocraft.pcsettings` in a folder the user chose (one in iCloud Drive,
//! say), and Keep in Sync saves there on quit and offers a newer file from another Mac at start.
//!
//! The file is the Export Settings… file plus a top-level `savedBy` object,
//! `{"computer": "<the Mac's name>", "at": "<RFC 3339 time>"}`; [`super::settings_sync`] reads it
//! like any other file (an older build skips it and names it, a build before that refuses it).
//! Other ArtCraft apps keep their own file in the same folder (`lightcraft.lcsettings`); only
//! [`FILE_NAME`] is ever read or written here.
//!
//! The chosen folder is also kept in `~/Library/Application Support/ArtCraft/sync-folder.json`
//! (`{"path": "<folder>"}`), shared by the ArtCraft apps: an app with no folder yet offers it.
//!
//! A file is written to a hidden temporary name in the same folder and renamed over the old one,
//! so a sync service never sees half a file, and a failed write leaves the old file alone.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::Preferences;
use super::cmds::bad;
use super::files::{read_text_file, rename_path};
use super::settings_sync::{self, MAX_BYTES, Plan};
use crate::color_cmds::ColorSettings;
use crate::commands::CommandSpec;
use crate::{Result, Session};

/// The file this app keeps in the sync folder.
pub const FILE_NAME: &str = "photocraft.pcsettings";
/// The ArtCraft folder's file that names the chosen sync folder.
pub const SHARED_NAME: &str = "sync-folder.json";
/// The most of the shared file that is read.
const SHARED_MAX_BYTES: usize = 64 * 1024;
/// The longest Mac name kept.
const NAME_MAX: usize = 100;

/// `sync.*` of the preferences: this Mac's sync folder and state. Not part of the settings file
/// (each Mac has its own).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncPrefs {
    /// The chosen sync folder; empty when none is chosen.
    pub folder: String,
    /// Keep in Sync: save to the folder on quit, and offer a newer file from another Mac at start.
    pub keep: bool,
    /// The `savedBy.at` of the file this Mac saved to the folder or loaded from it last.
    pub last: String,
}

/// Who saved a file and when: the file's `savedBy`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedBy {
    /// The Mac's name, as macOS shows it in Sharing.
    pub computer: String,
    /// When, as RFC 3339 with the Mac's offset (`2026-11-02T14:30:05+01:00`).
    pub at: String,
}

impl SavedBy {
    /// [`SavedBy::at`] as a point in time; `None` when it isn't RFC 3339.
    pub fn when(&self) -> Option<DateTime<FixedOffset>> {
        DateTime::parse_from_rfc3339(&self.at).ok()
    }

    /// [`SavedBy::at`] for a person, in this Mac's time zone ("November 2, 2026 at 14:30"); the
    /// text as written when it isn't RFC 3339.
    pub fn when_text(&self) -> String {
        self.when().map_or_else(|| self.at.clone(), |t| t.with_timezone(&Local).format("%B %-d, %Y at %H:%M").to_string())
    }
}

/// Where the session finds this Mac's name and the shared file; tests set both so nothing real is
/// touched.
#[derive(Clone, Debug, Default)]
pub struct SyncEnv {
    /// The ArtCraft `sync-folder.json` to use in place of the real one.
    pub shared_file: Option<PathBuf>,
    /// This Mac's name in place of the real one.
    pub computer: Option<String>,
}

impl SyncEnv {
    /// This Mac's name.
    pub fn computer(&self) -> String {
        self.computer.clone().unwrap_or_else(computer_name)
    }

    /// The shared file's path.
    pub fn shared_file(&self) -> Option<PathBuf> {
        self.shared_file.clone().or_else(default_shared_file)
    }

    /// This Mac's name and the time now.
    pub fn saved_by(&self) -> SavedBy {
        SavedBy { computer: self.computer(), at: now_stamp() }
    }
}

/// The Mac's name as macOS shows it in Sharing (System Settings › General › Sharing), else the
/// host name, else "This Mac".
pub fn computer_name() -> String {
    let from = |program: &str, args: &[&str]| {
        let out = std::process::Command::new(program).args(args).output().ok().filter(|o| o.status.success())?;
        clean_name(&String::from_utf8_lossy(&out.stdout))
    };
    (cfg!(target_os = "macos").then(|| from("scutil", &["--get", "ComputerName"])).flatten())
        .or_else(|| ["HOSTNAME", "COMPUTERNAME"].iter().find_map(|k| std::env::var(k).ok().and_then(|v| clean_name(&v))))
        .or_else(|| from("hostname", &[]))
        .unwrap_or_else(|| "This Mac".into())
}

/// `name` without control characters and spaces around, at most [`NAME_MAX`] characters; `None`
/// when nothing is left.
fn clean_name(name: &str) -> Option<String> {
    let name: String = name.chars().filter(|c| !c.is_control()).take(NAME_MAX).collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The time now as RFC 3339 in the Mac's own offset, to the second.
pub fn now_stamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// `~/Library/Application Support/ArtCraft/sync-folder.json` (macOS).
pub fn default_shared_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    cfg!(target_os = "macos").then(|| PathBuf::from(home).join("Library").join("Application Support").join("ArtCraft").join(SHARED_NAME))
}

// ------------------------------------------------------------------ the file

/// The settings file with `savedBy`.
pub fn stamped_text(p: &Preferences, color: &ColorSettings, by: &SavedBy) -> String {
    let mut doc = settings_sync::to_json(p, color);
    if let Some(m) = doc.as_object_mut() {
        m.insert("savedBy".into(), json!(by));
    }
    let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    text.push('\n');
    text
}

/// The `savedBy` of a settings file's text. A missing or unreadable one is `None`: it is a note
/// about the file, not a setting, so it never refuses a file.
pub fn saved_by(text: &str) -> Option<SavedBy> {
    let doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let by = doc.get("savedBy")?.as_object()?;
    let computer = clean_name(by.get("computer")?.as_str()?)?;
    let at: String = by.get("at")?.as_str()?.chars().filter(|c| !c.is_control()).take(64).collect();
    Some(SavedBy { computer, at })
}

/// A message for an I/O failure on `path` that tells the user what to do.
fn io_message(path: &Path, e: &std::io::Error, writing: bool) -> String {
    use std::io::ErrorKind;
    let shown = path.display();
    match e.kind() {
        ErrorKind::NotFound => format!("{shown} isn't there. Choose the sync folder again."),
        ErrorKind::PermissionDenied if writing => format!("{shown} can't be written to: it is read-only, or you have no permission. Choose another folder."),
        ErrorKind::PermissionDenied => format!("{shown} can't be read: you have no permission. Choose another folder."),
        ErrorKind::StorageFull => format!("The disk is full, so nothing was saved to {shown}"),
        _ => format!("{shown}: {e}"),
    }
}

/// The folder at `folder`, checked: chosen, there, and a folder.
pub fn check_folder(folder: &str) -> std::result::Result<PathBuf, String> {
    let folder = folder.trim();
    if folder.is_empty() {
        return Err("No sync folder is chosen. Choose one in Preferences first.".into());
    }
    let path = PathBuf::from(folder);
    match std::fs::metadata(&path) {
        Ok(m) if m.is_dir() => Ok(path),
        Ok(_) => Err(format!("{} is not a folder. Choose the sync folder again.", path.display())),
        Err(e) => Err(io_message(&path, &e, false)),
    }
}

/// Write `text` to `target` through a hidden temporary file beside it that is renamed over it
/// with `rename`; the temporary file goes when anything fails, and `target` stays as it was.
pub fn write_atomic(target: &Path, text: &str, rename: &dyn Fn(&Path, &Path) -> std::io::Result<()>) -> std::result::Result<(), String> {
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut staged = None;
    for n in 0..1000u32 {
        let tmp = dir.join(format!(".{name}.{}-{n}.tmp", std::process::id()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => {
                staged = Some((tmp, f));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_message(dir, &e, true)),
        }
    }
    let Some((tmp, mut file)) = staged else { return Err(format!("{}: no free temporary name", dir.display())) };
    let written = file.write_all(text.as_bytes()).and_then(|()| file.sync_all());
    drop(file);
    let done = written.and_then(|()| rename(&tmp, target));
    if let Err(e) = done {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_message(dir, &e, true));
    }
    Ok(())
}

/// What a save wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    /// The file.
    pub path: PathBuf,
    /// The `savedBy` written into it.
    pub by: SavedBy,
}

/// [`save`] with the rename replaced (a test fails it on purpose).
pub fn save_with(
    folder: &str,
    p: &Preferences,
    color: &ColorSettings,
    by: SavedBy,
    rename: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
) -> std::result::Result<Saved, String> {
    let path = check_folder(folder)?.join(FILE_NAME);
    write_atomic(&path, &stamped_text(p, color, &by), rename)?;
    Ok(Saved { path, by })
}

/// Save the settings `p` and `color` in `folder` as [`FILE_NAME`], with `by` as `savedBy`.
pub fn save(folder: &str, p: &Preferences, color: &ColorSettings, by: SavedBy) -> std::result::Result<Saved, String> {
    save_with(folder, p, color, by, &rename_path)
}

/// The file read from the sync folder.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderFile {
    /// Where it is.
    pub path: PathBuf,
    /// Its text.
    pub text: String,
    /// Who saved it and when, when it says.
    pub by: Option<SavedBy>,
}

/// Read [`FILE_NAME`] from `folder`. An iCloud file that isn't downloaded yet, a missing file and
/// a file that isn't readable each say what to do.
pub fn read(folder: &str) -> std::result::Result<FolderFile, String> {
    let dir = check_folder(folder)?;
    let path = dir.join(FILE_NAME);
    match std::fs::metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // iCloud Drive leaves `.name.icloud` in place of a file it hasn't downloaded.
            return Err(if dir.join(format!(".{FILE_NAME}.icloud")).exists() {
                format!("iCloud hasn't downloaded {FILE_NAME} yet. Open {} in Finder, wait for it, and try again.", dir.display())
            } else {
                format!(
                    "There is no {FILE_NAME} in {} yet. Use Save to Folder on the Mac that has the settings, or choose the folder that holds it.",
                    dir.display()
                )
            });
        }
        Err(e) => return Err(io_message(&path, &e, false)),
    }
    let shown = path.to_string_lossy().into_owned();
    let text = read_text_file(&shown, MAX_BYTES).map_err(|e| format!("{shown}: {e}"))?;
    let by = saved_by(&text);
    Ok(FolderFile { path, text, by })
}

// ------------------------------------------------------------------ newer or not

/// Is a file saved by `file` something to offer a Mac called `me` that last saved or loaded the
/// file stamped `last`? Yes when it comes from another Mac and is newer than `last` (any, when
/// `last` is empty or unreadable). A file whose time can't be read is never offered.
pub fn is_newer_from_another_mac(file: &SavedBy, me: &str, last: &str) -> bool {
    if file.computer == me {
        return false;
    }
    match (file.when(), DateTime::parse_from_rfc3339(last).ok()) {
        (Some(file), Some(last)) => file > last,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

// ------------------------------------------------------------------ the shared file

/// The folder `sync-folder.json` names. A missing file, a bad one, a missing or empty `path`, or
/// a path that isn't absolute is `None`, never an error; other keys are ignored.
pub fn read_shared(file: &Path) -> Option<String> {
    let text = read_text_file(&file.to_string_lossy(), SHARED_MAX_BYTES).ok()?;
    let doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let path = doc.get("path")?.as_str()?.trim();
    (!path.is_empty() && Path::new(path).is_absolute()).then(|| path.to_string())
}

/// Write `folder` as `path` in the shared file, atomically, keeping any other keys it holds.
pub fn write_shared(file: &Path, folder: &str) -> std::result::Result<(), String> {
    if let Some(dir) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| io_message(dir, &e, true))?;
    }
    let mut doc: Map<String, Value> =
        read_text_file(&file.to_string_lossy(), SHARED_MAX_BYTES).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    doc.insert("path".into(), json!(folder));
    let mut text = serde_json::to_string_pretty(&Value::Object(doc)).unwrap_or_default();
    text.push('\n');
    write_atomic(file, &text, &rename_path)
}

// ------------------------------------------------------------------ the session

impl Session {
    /// Choose the sync folder (empty to forget it) and tell the other ArtCraft apps through the
    /// shared file. The folder counts even when the shared file can't be written; that is the
    /// error returned.
    pub fn choose_sync_folder(&mut self, folder: &str) -> std::result::Result<(), String> {
        let folder = folder.trim().to_string();
        if self.prefs().sync.folder == folder {
            return Ok(());
        }
        self.prefs.edit(|p| {
            p.sync.folder.clone_from(&folder);
            // A different folder holds a different file.
            p.sync.last.clear();
        });
        match (folder.is_empty(), self.sync_env.shared_file()) {
            (false, Some(file)) => {
                write_shared(&file, &folder).map_err(|e| format!("The folder is chosen, but the other ArtCraft apps could not be told: {e}"))
            }
            _ => Ok(()),
        }
    }

    /// The folder the shared file names, when this Mac has none chosen yet and the file names one.
    pub fn offered_sync_folder(&self) -> Option<String> {
        if !self.prefs().sync.folder.is_empty() {
            return None;
        }
        read_shared(&self.sync_env.shared_file()?)
    }

    /// Note that this Mac saved or loaded the file stamped `by`.
    pub fn note_synced(&mut self, by: &SavedBy) {
        if self.prefs().sync.last != by.at {
            self.prefs.edit(|p| p.sync.last.clone_from(&by.at));
        }
    }

    /// Keep in Sync at start: the folder's file, when it comes from another Mac and is newer
    /// than the last one this Mac saved or loaded. `Ok(None)` when Keep in Sync is off, there is
    /// nothing newer, or the folder has no file yet.
    pub fn newer_sync_file(&self) -> std::result::Result<Option<FolderFile>, String> {
        let sync = &self.prefs().sync;
        if !sync.keep || sync.folder.is_empty() {
            return Ok(None);
        }
        let dir = check_folder(&sync.folder)?;
        if !dir.join(FILE_NAME).exists() && !dir.join(format!(".{FILE_NAME}.icloud")).exists() {
            return Ok(None);
        }
        let file = read(&sync.folder)?;
        let newer = file.by.as_ref().is_some_and(|by| is_newer_from_another_mac(by, &self.sync_env.computer(), &sync.last));
        Ok(newer.then_some(file))
    }

    /// Keep in Sync on quit: save to the folder, unless the folder holds a newer file from
    /// another Mac that this Mac hasn't loaded (saving would replace it). `Ok(None)` when nothing
    /// was saved.
    pub fn save_settings_on_quit(&mut self) -> std::result::Result<Option<Saved>, String> {
        let sync = self.prefs().sync.clone();
        if !sync.keep || sync.folder.is_empty() {
            return Ok(None);
        }
        if self.newer_sync_file()?.is_some() {
            return Ok(None);
        }
        self.save_settings_to_folder().map(Some)
    }

    /// Save the settings to the sync folder now (the command's work).
    pub fn save_settings_to_folder(&mut self) -> std::result::Result<Saved, String> {
        let saved = save(&self.prefs().sync.folder, self.prefs(), &self.color.settings, self.sync_env.saved_by())?;
        self.note_synced(&saved.by);
        Ok(saved)
    }

    /// Read the sync folder's file against the settings now, changing nothing.
    pub fn plan_folder_load(&self) -> std::result::Result<(FolderFile, Plan), String> {
        let file = read(&self.prefs().sync.folder)?;
        let plan = self.plan_settings_import(&file.text)?;
        Ok((file, plan))
    }

    /// Apply a [`Plan`] read from `file` and note that this Mac now has it.
    pub fn apply_folder_load(&mut self, file: &FolderFile, plan: Plan) {
        self.apply_settings_plan(plan);
        if let Some(by) = &file.by {
            self.note_synced(by);
        }
    }
}

fn web_refused(cmd: &str) -> Result<()> {
    if cfg!(target_arch = "wasm32") { Err(bad(cmd, "the sync folder is not available on the web")) } else { Ok(()) }
}

fn save_settings_to_folder(s: &mut Session, _: &Value) -> Result<Value> {
    let cmd = "edit.preferences.saveSettingsToFolder";
    web_refused(cmd)?;
    let saved = s.save_settings_to_folder().map_err(|e| bad(cmd, e))?;
    Ok(json!({"file": saved.path, "savedBy": saved.by}))
}

fn load_settings_from_folder(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "edit.preferences.loadSettingsFromFolder";
    web_refused(cmd)?;
    let (file, plan) = s.plan_folder_load().map_err(|e| bad(cmd, e))?;
    let dry = p.get("dryRun").and_then(Value::as_bool) == Some(true);
    let mut report = plan.report(!dry);
    report["file"] = json!(file.path);
    report["savedBy"] = json!(file.by);
    if !dry {
        s.apply_folder_load(&file, plan);
    }
    Ok(report)
}

/// The two commands.
pub fn specs() -> Vec<CommandSpec> {
    fn always(_: &Session) -> std::result::Result<(), String> {
        Ok(())
    }
    vec![
        CommandSpec {
            id: "edit.preferences.saveSettingsToFolder",
            label: "Save Settings to Folder",
            menu: &["Edit", "Preferences"],
            shortcut: None,
            params: r##"{} (writes photocraft.pcsettings, with `savedBy`, into the folder in the preference `sync.folder`, through a temporary file; the reply has `file` and `savedBy`)"##,
            enabled: always,
            run: save_settings_to_folder,
            journal: false,
        },
        CommandSpec {
            id: "edit.preferences.loadSettingsFromFolder",
            label: "Load Settings from Folder",
            menu: &["Edit", "Preferences"],
            shortcut: None,
            params: r##"{"dryRun":bool=false} (reads photocraft.pcsettings from the folder in the preference `sync.folder` like Import Settings: the whole file is checked first, settings this build doesn't know are skipped and named; the reply has the import reply plus `file` and `savedBy`, who saved the file and when; the app's own window asks first when called without parameters)"##,
            enabled: always,
            run: load_settings_from_folder,
            journal: false,
        },
    ]
}

#[cfg(test)]
#[path = "settings_folder/tests.rs"]
mod tests;
