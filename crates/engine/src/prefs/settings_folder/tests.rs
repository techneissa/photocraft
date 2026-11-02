use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use super::*;

/// A new empty folder under the system's temporary folder, removed at the end. Never a real
/// ArtCraft or iCloud folder.
struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("pc-sync-folder-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn s(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A session on the Mac `mac`, with its shared file inside `dir` and `folder` chosen.
fn session(mac: &str, dir: &Dir, folder: &Dir) -> Session {
    let mut s = Session::new();
    s.sync_env = SyncEnv { shared_file: Some(dir.path().join("ArtCraft").join(SHARED_NAME)), computer: Some(mac.into()) };
    s.choose_sync_folder(&folder.s()).unwrap();
    s
}

fn read_json(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn by(computer: &str, at: &str) -> SavedBy {
    SavedBy { computer: computer.into(), at: at.into() }
}

#[test]
fn save_then_load_round_trips_and_uses_the_file_name() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("prefs.set", json!({"path": "export.jpegQuality", "value": 33})).unwrap();
    let out = a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    assert_eq!(folder.names(), vec!["photocraft.pcsettings"], "only the one file, no temporary file left");
    assert_eq!(out["file"], json!(folder.path().join("photocraft.pcsettings")));
    assert_eq!(out["savedBy"]["computer"], "Studio Mac");
    let mut b = session("Laptop", &home, &folder);
    assert_eq!(b.prefs().export.jpeg_quality, 85);
    let dry = b.execute("edit.preferences.loadSettingsFromFolder", json!({"dryRun": true})).unwrap();
    assert_eq!(dry["applied"], false);
    assert_eq!(dry["savedBy"]["computer"], "Studio Mac");
    assert_eq!(b.prefs().export.jpeg_quality, 85, "a dry run changes nothing");
    let done = b.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap();
    assert_eq!(done["applied"], true);
    assert_eq!(b.prefs().export.jpeg_quality, 33);
    assert_eq!(done["unknownCount"], 0, "savedBy is not an unknown setting");
    assert_eq!(b.prefs().sync.folder, folder.s(), "this Mac's own sync folder stays");
    assert_eq!(b.prefs().sync.last, a.prefs().sync.last, "the load is noted: the time of the file");
}

#[test]
fn saved_by_is_written_as_computer_and_an_rfc_3339_time_and_read_back() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    let doc = read_json(&folder.path().join(FILE_NAME));
    assert_eq!(doc["savedBy"]["computer"], "Studio Mac");
    let at = doc["savedBy"]["at"].as_str().unwrap();
    assert!(DateTime::parse_from_rfc3339(at).is_ok(), "{at}");
    assert_eq!(doc["kind"], "photocraft-settings");
    assert_eq!(doc["version"], 1);
    let file = read(&folder.s()).unwrap();
    assert_eq!(file.by, Some(by("Studio Mac", at)));
    assert_eq!(a.prefs().sync.last, at);
    // The time is chrono's, to the second, in the Mac's own offset.
    assert!(at.len() == 25 || at.ends_with('Z'), "{at}");
}

#[test]
fn a_file_without_or_with_a_bad_saved_by_still_loads() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("prefs.set", json!({"path": "export.jpegQuality", "value": 41})).unwrap();
    let mut doc: Value = serde_json::from_str(&a.settings_text()).unwrap();
    for bad in [Value::Null, json!(5), json!({"computer": 4, "at": []}), json!({"computer": "", "at": "x"})] {
        doc["savedBy"] = bad;
        std::fs::write(folder.path().join(FILE_NAME), doc.to_string()).unwrap();
        let mut b = session("Laptop", &home, &folder);
        let out = b.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap();
        assert_eq!(out["savedBy"], Value::Null);
        assert_eq!(out["unknownCount"], 0);
        assert_eq!(b.prefs().export.jpeg_quality, 41);
    }
    doc.as_object_mut().unwrap().remove("savedBy");
    std::fs::write(folder.path().join(FILE_NAME), doc.to_string()).unwrap();
    assert_eq!(read(&folder.s()).unwrap().by, None);
}

#[test]
fn another_apps_file_in_the_folder_is_never_read_or_written() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let theirs = folder.path().join("lightcraft.lcsettings");
    std::fs::write(&theirs, b"{\"kind\":\"lightcraft-settings\",\"version\":1}").unwrap();
    let before = std::fs::read(&theirs).unwrap();
    let mut a = session("Studio Mac", &home, &folder);
    // With only their file there, loading finds nothing of ours.
    let e = a.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap_err().to_string();
    assert!(e.contains("There is no photocraft.pcsettings"), "{e}");
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    a.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap();
    assert_eq!(std::fs::read(&theirs).unwrap(), before);
    assert_eq!(folder.names(), vec!["lightcraft.lcsettings", "photocraft.pcsettings"]);
}

#[test]
fn an_unknown_setting_is_skipped_and_named_and_a_bad_known_value_refuses() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    let path = folder.path().join(FILE_NAME);
    let mut doc = read_json(&path);
    doc["preferences"]["export"]["jpegQuality"] = json!(22);
    doc["preferences"]["export"]["someNewSetting"] = json!(true);
    std::fs::write(&path, doc.to_string()).unwrap();
    let mut b = session("Laptop", &home, &folder);
    let out = b.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap();
    assert_eq!(out["unknown"], json!(["export.someNewSetting"]));
    assert_eq!(b.prefs().export.jpeg_quality, 22);
    // A bad value for a known setting: an error, and nothing changes.
    doc["preferences"]["export"]["jpegQuality"] = json!(0);
    std::fs::write(&path, doc.to_string()).unwrap();
    let mut c = session("Third", &home, &folder);
    let e = c.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap_err().to_string();
    assert!(e.contains("Nothing was changed"), "{e}");
    assert_eq!(c.prefs().export.jpeg_quality, 85);
    assert_eq!(c.prefs().sync.last, "", "a refused file is not noted as loaded");
}

#[test]
fn a_failed_write_leaves_the_old_file_and_no_partial_file() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    let path = folder.path().join(FILE_NAME);
    let old = std::fs::read(&path).unwrap();
    a.execute("prefs.set", json!({"path": "export.jpegQuality", "value": 12})).unwrap();
    let failing = |_: &Path, _: &Path| Err(std::io::Error::other("the disk went away"));
    let e = save_with(&folder.s(), a.prefs(), &a.color.settings, by("Studio Mac", "2026-11-02T10:00:00+01:00"), &failing).unwrap_err();
    assert!(e.contains("the disk went away"), "{e}");
    assert_eq!(std::fs::read(&path).unwrap(), old, "the old file is whole");
    assert_eq!(folder.names(), vec!["photocraft.pcsettings"], "no temporary file is left");
    // A first save that fails leaves no file at all.
    let empty = Dir::new("empty");
    save_with(&empty.s(), a.prefs(), &a.color.settings, by("x", "y"), &failing).unwrap_err();
    assert!(empty.names().is_empty());
}

#[test]
fn the_new_file_is_written_beside_the_old_one_and_renamed_over_it() {
    let folder = Dir::new("folder");
    let target = folder.path().join(FILE_NAME);
    std::fs::write(&target, "old").unwrap();
    let seen = std::cell::RefCell::new(None);
    let spy = |from: &Path, to: &Path| {
        *seen.borrow_mut() = Some((from.to_path_buf(), to.to_path_buf(), std::fs::read_to_string(from).unwrap(), std::fs::read_to_string(to).unwrap()));
        std::fs::rename(from, to)
    };
    write_atomic(&target, "new", &spy).unwrap();
    let (from, to, staged, current) = seen.take().unwrap();
    assert_eq!(from.parent(), to.parent(), "the same folder, so the rename is atomic");
    assert!(from.file_name().unwrap().to_string_lossy().starts_with('.'), "a hidden name");
    assert_eq!((staged.as_str(), current.as_str()), ("new", "old"), "complete before the rename, the old file untouched until it");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(folder.names(), vec![FILE_NAME]);
}

#[test]
fn a_missing_gone_or_read_only_folder_gives_a_message_to_act_on() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = Session::new();
    a.sync_env = SyncEnv { shared_file: Some(home.path().join(SHARED_NAME)), computer: Some("Studio Mac".into()) };
    for cmd in ["edit.preferences.saveSettingsToFolder", "edit.preferences.loadSettingsFromFolder"] {
        let e = a.execute(cmd, json!({})).unwrap_err().to_string();
        assert!(e.contains("No sync folder is chosen"), "{e}");
    }
    let gone = folder.path().join("deleted");
    a.choose_sync_folder(&gone.to_string_lossy()).unwrap();
    for cmd in ["edit.preferences.saveSettingsToFolder", "edit.preferences.loadSettingsFromFolder"] {
        let e = a.execute(cmd, json!({})).unwrap_err().to_string();
        assert!(e.contains("isn't there") && e.contains("Choose the sync folder again"), "{e}");
    }
    // A file where the folder should be.
    let file = folder.path().join("plain.txt");
    std::fs::write(&file, "x").unwrap();
    a.choose_sync_folder(&file.to_string_lossy()).unwrap();
    assert!(a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap_err().to_string().contains("is not a folder"));
    // A read-only folder.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let ro = Dir::new("readonly");
        std::fs::set_permissions(ro.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        a.choose_sync_folder(&ro.s()).unwrap();
        let writable_anyway = std::fs::write(ro.path().join("probe"), "x").is_ok();
        let r = a.execute("edit.preferences.saveSettingsToFolder", json!({}));
        std::fs::set_permissions(ro.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        if !writable_anyway {
            let e = r.unwrap_err().to_string();
            assert!(e.contains("can't be written to") && e.contains("Choose another folder"), "{e}");
            assert!(!ro.path().join(FILE_NAME).exists());
        }
    }
}

#[test]
fn a_file_icloud_has_not_downloaded_says_so() {
    let folder = Dir::new("folder");
    std::fs::write(folder.path().join(".photocraft.pcsettings.icloud"), "placeholder").unwrap();
    let e = read(&folder.s()).unwrap_err();
    assert!(e.contains("iCloud hasn't downloaded"), "{e}");
}

#[test]
fn a_file_that_is_too_big_or_not_text_is_refused_without_a_crash() {
    let folder = Dir::new("folder");
    let path = folder.path().join(FILE_NAME);
    std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
    assert!(read(&folder.s()).is_err());
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0x01]).unwrap();
    assert!(read(&folder.s()).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(read(&folder.s()).is_err(), "a folder named like the file");
}

#[test]
fn the_shared_file_is_written_read_and_offered_and_a_bad_one_is_ignored() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let shared = home.path().join("ArtCraft").join(SHARED_NAME);
    let a = session("Studio Mac", &home, &folder);
    assert_eq!(read_json(&shared), json!({"path": folder.s()}), "{{\"path\": …}}, the folder made on the way");
    assert!(a.offered_sync_folder().is_none(), "this Mac has its folder already");
    assert_eq!(home.path().join("ArtCraft").read_dir().unwrap().count(), 1, "no temporary file left");
    // A Mac with no folder yet is offered the shared one.
    let mut b = Session::new();
    b.sync_env = a.sync_env.clone();
    assert_eq!(b.offered_sync_folder(), Some(folder.s()));
    b.choose_sync_folder(&folder.s()).unwrap();
    assert_eq!(b.offered_sync_folder(), None);
    // Unknown keys are ignored when reading and kept when writing.
    std::fs::write(&shared, json!({"path": folder.s(), "color": "blue", "apps": ["x"]}).to_string()).unwrap();
    assert_eq!(read_shared(&shared), Some(folder.s()));
    let other = Dir::new("other");
    write_shared(&shared, &other.s()).unwrap();
    assert_eq!(read_json(&shared), json!({"path": other.s(), "color": "blue", "apps": ["x"]}));
    // Missing, damaged or odd files are not errors, only nothing to offer.
    assert_eq!(read_shared(&home.path().join("none.json")), None);
    for text in ["", "not json", "[]", "{}", r#"{"path": 5}"#, r#"{"path": ""}"#, r#"{"path": "relative/dir"}"#, r#"{"path": null}"#] {
        std::fs::write(&shared, text).unwrap();
        assert_eq!(read_shared(&shared), None, "{text}");
        let mut c = Session::new();
        c.sync_env = a.sync_env.clone();
        assert_eq!(c.offered_sync_folder(), None);
    }
    // A damaged shared file is replaced when a folder is chosen.
    std::fs::write(&shared, "garbage").unwrap();
    let mut d = Session::new();
    d.sync_env = a.sync_env.clone();
    d.choose_sync_folder(&folder.s()).unwrap();
    assert_eq!(read_shared(&shared), Some(folder.s()));
}

#[test]
fn choosing_a_folder_reports_a_shared_file_that_cannot_be_written_and_still_chooses() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    // The shared file's folder is a file, so it can't be made.
    std::fs::write(home.path().join("ArtCraft"), "in the way").unwrap();
    let mut s = Session::new();
    s.sync_env = SyncEnv { shared_file: Some(home.path().join("ArtCraft").join(SHARED_NAME)), computer: Some("M".into()) };
    let e = s.choose_sync_folder(&folder.s()).unwrap_err();
    assert!(e.contains("the other ArtCraft apps could not be told"), "{e}");
    assert_eq!(s.prefs().sync.folder, folder.s());
}

#[test]
fn choosing_another_folder_forgets_the_last_sync_and_empty_forgets_the_folder() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    assert!(!a.prefs().sync.last.is_empty());
    let other = Dir::new("other");
    a.choose_sync_folder(&other.s()).unwrap();
    assert_eq!(a.prefs().sync.last, "");
    a.choose_sync_folder("  ").unwrap();
    assert_eq!(a.prefs().sync.folder, "");
}

#[test]
fn the_sync_state_is_not_in_the_settings_file_and_not_taken_from_one() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("prefs.set", json!({"path": "sync.keep", "value": true})).unwrap();
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    let doc = read_json(&folder.path().join(FILE_NAME));
    assert!(!doc.to_string().contains(&folder.s()), "no folder of this Mac in the file");
    assert!(doc["preferences"].get("sync").is_none() && doc.get("sync").is_none());
    // A file that carries one anyway is skipped and named, and this Mac keeps its own.
    let mut forged = doc.clone();
    forged["preferences"]["sync"] = json!({"folder": "/elsewhere", "keep": false});
    std::fs::write(folder.path().join(FILE_NAME), forged.to_string()).unwrap();
    let mut b = session("Laptop", &home, &folder);
    let out = b.execute("edit.preferences.loadSettingsFromFolder", json!({})).unwrap();
    assert_eq!(out["unknown"], json!(["sync"]));
    assert_eq!(b.prefs().sync.folder, folder.s());
}

#[test]
fn the_rule_for_offering_a_file_is_newer_and_from_another_mac() {
    let old = "2026-11-02T10:00:00+01:00";
    let new = "2026-11-02T10:00:01+01:00";
    let same_instant_other_zone = "2026-11-02T09:00:00+00:00";
    assert!(is_newer_from_another_mac(&by("Studio", new), "Laptop", old));
    assert!(!is_newer_from_another_mac(&by("Studio", old), "Laptop", old), "not newer");
    assert!(!is_newer_from_another_mac(&by("Studio", same_instant_other_zone), "Laptop", old), "compared as instants, not as text");
    assert!(!is_newer_from_another_mac(&by("Studio", old), "Laptop", new), "older");
    assert!(!is_newer_from_another_mac(&by("Laptop", new), "Laptop", old), "this Mac's own file");
    assert!(is_newer_from_another_mac(&by("Studio", old), "Laptop", ""), "nothing saved or loaded here yet");
    assert!(is_newer_from_another_mac(&by("Studio", old), "Laptop", "garbage"));
    assert!(!is_newer_from_another_mac(&by("Studio", "yesterday"), "Laptop", old), "a time that can't be read is never offered");
}

#[test]
fn keep_in_sync_offers_a_newer_file_from_another_mac_at_start_and_not_otherwise() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut studio = session("Studio Mac", &home, &folder);
    let mut laptop = session("Laptop", &home, &folder);
    // Off: nothing is offered.
    studio.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    assert!(laptop.newer_sync_file().unwrap().is_none());
    laptop.execute("prefs.set", json!({"path": "sync.keep", "value": true})).unwrap();
    let file = laptop.newer_sync_file().unwrap().expect("a file from another Mac, never loaded here");
    assert_eq!(file.by.as_ref().unwrap().computer, "Studio Mac");
    // After loading it, it is no longer new.
    let (file, plan) = laptop.plan_folder_load().unwrap();
    laptop.apply_folder_load(&file, plan);
    assert!(laptop.newer_sync_file().unwrap().is_none());
    // The Mac's own file is not offered back to it.
    studio.execute("prefs.set", json!({"path": "sync.keep", "value": true})).unwrap();
    assert!(studio.newer_sync_file().unwrap().is_none());
    // A later save from the other Mac is offered again.
    let path = folder.path().join(FILE_NAME);
    let mut doc = read_json(&path);
    doc["savedBy"]["at"] = json!("2999-01-01T00:00:00+00:00");
    std::fs::write(&path, doc.to_string()).unwrap();
    assert!(laptop.newer_sync_file().unwrap().is_some());
    // No file yet, and no folder: nothing, not an error.
    std::fs::remove_file(&path).unwrap();
    assert!(laptop.newer_sync_file().unwrap().is_none());
    laptop.choose_sync_folder("").unwrap();
    assert!(laptop.newer_sync_file().unwrap().is_none());
    // A folder that went away is a message.
    laptop.choose_sync_folder(&folder.path().join("deleted").to_string_lossy()).unwrap();
    assert!(laptop.newer_sync_file().unwrap_err().contains("isn't there"));
}

#[test]
fn keep_in_sync_saves_on_quit_unless_that_would_replace_a_newer_file_from_another_mac() {
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut studio = session("Studio Mac", &home, &folder);
    let mut laptop = session("Laptop", &home, &folder);
    // Off: nothing is written.
    assert!(laptop.save_settings_on_quit().unwrap().is_none());
    assert!(folder.names().is_empty());
    laptop.execute("prefs.set", json!({"path": "sync.keep", "value": true})).unwrap();
    laptop.execute("prefs.set", json!({"path": "export.jpegQuality", "value": 20})).unwrap();
    let saved = laptop.save_settings_on_quit().unwrap().expect("saved");
    assert_eq!(saved.by.computer, "Laptop");
    assert_eq!(read_json(&folder.path().join(FILE_NAME))["preferences"]["export"]["jpegQuality"], 20);
    // The other Mac saves meanwhile; the laptop has not loaded it, so quitting must not replace it.
    studio.execute("prefs.set", json!({"path": "export.jpegQuality", "value": 55})).unwrap();
    let path = folder.path().join(FILE_NAME);
    let mut doc = read_json(&path);
    doc["preferences"]["export"]["jpegQuality"] = json!(55);
    doc["savedBy"] = json!({"computer": "Studio Mac", "at": "2999-01-01T00:00:00+00:00"});
    std::fs::write(&path, doc.to_string()).unwrap();
    assert!(laptop.save_settings_on_quit().unwrap().is_none(), "left alone");
    assert_eq!(read_json(&path)["preferences"]["export"]["jpegQuality"], 55);
    // Once loaded, quitting saves again.
    let (file, plan) = laptop.plan_folder_load().unwrap();
    laptop.apply_folder_load(&file, plan);
    assert!(laptop.save_settings_on_quit().unwrap().is_some());
    // A damaged file in the folder is replaced by a good one.
    std::fs::write(&path, "garbage").unwrap();
    assert!(laptop.save_settings_on_quit().unwrap().is_some());
    assert!(read(&folder.s()).unwrap().by.is_some());
    // A folder that went away is an error to log, not a crash.
    laptop.choose_sync_folder(&folder.path().join("deleted").to_string_lossy()).unwrap();
    assert!(laptop.save_settings_on_quit().is_err());
}

#[test]
fn this_macs_name_is_never_empty_and_the_time_is_rfc_3339() {
    let name = computer_name();
    assert!(!name.is_empty() && name.chars().count() <= NAME_MAX && !name.chars().any(char::is_control), "{name:?}");
    assert!(DateTime::parse_from_rfc3339(&now_stamp()).is_ok());
    assert_eq!(clean_name("  Studio\u{7} Mac \n"), Some("Studio Mac".into()));
    assert_eq!(clean_name(" \n"), None);
    assert_eq!(clean_name(&"x".repeat(500)).unwrap().chars().count(), NAME_MAX);
    assert_eq!(SyncEnv { computer: Some("Fixed".into()), ..Default::default() }.computer(), "Fixed");
}

#[test]
fn the_shared_file_lives_in_artcrafts_application_support_folder() {
    if let Some(p) = default_shared_file() {
        assert!(p.ends_with("Library/Application Support/ArtCraft/sync-folder.json"), "{}", p.display());
    }
}

#[test]
fn an_older_build_that_knows_nothing_of_saved_by_skips_and_names_it() {
    // What PR #2's rule does with a top-level key it doesn't have: the file imports, and the key is named.
    let (home, folder) = (Dir::new("home"), Dir::new("folder"));
    let mut a = session("Studio Mac", &home, &folder);
    a.execute("edit.preferences.saveSettingsToFolder", json!({})).unwrap();
    let mut doc = read_json(&folder.path().join(FILE_NAME));
    doc.as_object_mut().unwrap().insert("savedByANewerKey".into(), json!({}));
    let plan = a.plan_settings_import(&doc.to_string()).unwrap();
    assert_eq!(plan.unknown, vec!["savedByANewerKey"], "savedBy itself is known, a neighbour is not");
}
