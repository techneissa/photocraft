//! fork: the settings this fork adds survive the move from `preferences.json` to `config.toml`
//! (the settings store's one-time import ignores keys `Preferences` lacks, so a fork field missing
//! from it would be dropped at the first start of a build with the new settings file).
#![cfg(not(target_arch = "wasm32"))]

use serde_json::json;

use super::{ObjectEngine, Preferences, SavedPrefs, SubjectEngine, store_options};

/// A `preferences.json` as the fork wrote it before `config.toml`, every fork field off its default.
fn fork_preferences_json() -> String {
    json!({
        "version": 2,
        "general": {"lockBackgroundLayer": true},
        "interface": {
            "homeRecentCount": 30,
            "scrollWheelChangesBlendMode": true,
            "showDoubleExposureButton": false,
            "topBar": {"brand": false, "title": true, "workspace": false, "search": true, "appearance": false}
        },
        "tools": {"selectSubjectEngine": "sam3", "objectSelectionEngine": "classic"},
        "fileHandling": {"recentFiles": ["/photos/a.psd"], "recentOpened": {"/photos/a.psd": 1_790_000_000_u64}},
        "chords": {"edit.copy": "Cmd+K C"},
        "shortcutWhen": {"edit.copy": "!textFocus && tool == brush"},
        "shortcutSets": {"Mine": {"shortcuts": {"edit.cut": "Cmd+Alt+X"}, "chords": {"edit.paste": "Cmd+K V"}, "shortcutWhen": {"edit.paste": "hasDocument"}}},
        "shortcutSet": "Mine",
        "contextMenus": {"menus": {"layers.row": {"items": ["cmd:layer.duplicate", "sep", "cmd:layer.doubleExposure"], "hidden": ["layer.delete"]}}, "presets": [null, {"name": "Two"}]},
        "paletteHistory": {"uses": {"layer.doubleExposure": {"count": 3, "last": 1_790_000_000_u64}}, "picked": {"dou": "layer.doubleExposure"}},
        "sync": {"folder": "/Users/me/Sync", "keep": true, "last": "2026-11-20T10:00:00Z"}
    })
    .to_string()
}

fn assert_fork_fields(p: &Preferences, from: &str) {
    assert!(p.general.lock_background_layer, "{from}: lockBackgroundLayer");
    assert_eq!(p.interface.home_recent_count, 30, "{from}: homeRecentCount");
    assert!(p.interface.scroll_wheel_changes_blend_mode, "{from}: scrollWheelChangesBlendMode");
    assert!(!p.interface.show_double_exposure_button, "{from}: showDoubleExposureButton");
    assert!(!p.interface.top_bar.brand && !p.interface.top_bar.workspace && !p.interface.top_bar.appearance, "{from}: topBar");
    assert_eq!(p.tools.select_subject_engine, SubjectEngine::Sam3, "{from}: selectSubjectEngine");
    assert_eq!(p.tools.object_selection_engine, ObjectEngine::Classic, "{from}: objectSelectionEngine");
    assert_eq!(p.file_handling.recent_opened.get("/photos/a.psd"), Some(&1_790_000_000), "{from}: recentOpened");
    assert_eq!(p.chords.get("edit.copy").map(String::as_str), Some("Cmd+K C"), "{from}: chords");
    assert_eq!(p.shortcut_when.get("edit.copy").map(String::as_str), Some("!textFocus && tool == brush"), "{from}: shortcutWhen");
    let set = p.shortcut_sets.get("Mine").unwrap_or_else(|| panic!("{from}: shortcutSets"));
    assert_eq!(set.chords.get("edit.paste").map(String::as_str), Some("Cmd+K V"), "{from}: a set's chords");
    assert_eq!(set.shortcut_when.get("edit.paste").map(String::as_str), Some("hasDocument"), "{from}: a set's shortcutWhen");
    assert_eq!(p.context_menus["menus"]["layers.row"]["items"][2], "cmd:layer.doubleExposure", "{from}: contextMenus");
    assert_eq!(p.context_menus["menus"]["layers.row"]["hidden"][0], "layer.delete", "{from}: contextMenus");
    assert_eq!(p.palette_history["picked"]["dou"], "layer.doubleExposure", "{from}: paletteHistory");
    assert_eq!(p.palette_history["uses"]["layer.doubleExposure"]["count"], 3, "{from}: paletteHistory");
    assert_eq!((p.sync.folder.as_str(), p.sync.keep, p.sync.last.as_str()), ("/Users/me/Sync", true, "2026-11-20T10:00:00Z"), "{from}: sync");
}

/// The first start with `config.toml` imports the fork's `preferences.json`, writes `config.toml`,
/// and the next start reads every fork setting back from it.
#[test]
fn fork_settings_survive_the_import_into_config_toml_and_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(super::LEGACY_PREFS_FILE), fork_preferences_json()).unwrap();

    let (mut store, report) = craft::settings::SettingsStore::<SavedPrefs>::open_with(store_options(Some(dir.path())));
    assert!(report.section_errors.is_empty(), "{:?}", report.section_errors);
    assert_fork_fields(&store.get().prefs, "the import");
    store.flush().unwrap();
    assert!(dir.path().join("config.toml").is_file());

    // The next start: `config.toml` alone, the old file gone.
    std::fs::remove_file(dir.path().join(super::LEGACY_PREFS_FILE)).unwrap();
    let (again, report) = craft::settings::SettingsStore::<SavedPrefs>::read(&store_options(Some(dir.path())));
    assert!(report.section_errors.is_empty(), "{:?}", report.section_errors);
    assert_fork_fields(&again.prefs, "config.toml");
}
