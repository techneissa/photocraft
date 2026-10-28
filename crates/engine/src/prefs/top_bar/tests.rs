use super::*;
use crate::Session;
use crate::prefs::{InvalidPref, Preferences};
use serde_json::json;

#[test]
fn every_item_is_shown_by_default_and_ids_match_the_fields() {
    let bar = TopBar::default();
    for id in TopBar::IDS {
        assert_eq!(bar.shown(id), Some(true), "{id}");
    }
    assert_eq!(bar.shown("menus"), None, "the menus are not an item");
    // Each id is the key of the stored object, so the Preferences page and prefs.set agree with it.
    let stored = serde_json::to_value(&bar).unwrap();
    let mut keys: Vec<&str> = stored.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut ids = TopBar::IDS.to_vec();
    ids.sort_unstable();
    assert_eq!(keys, ids);
    assert_eq!(
        Preferences::default().get("interface.topBar"),
        Some(json!({"brand": true, "title": true, "workspace": true, "search": true, "appearance": true}))
    );
}

#[test]
fn settings_from_before_the_top_bar_show_every_item() {
    let mut s = Session::new();
    let invalid = s.load_prefs_json(r#"{"interface": {"showTooltips": false}}"#).unwrap();
    assert!(invalid.is_empty());
    assert_eq!(s.prefs().interface.top_bar, TopBar::default());
    // An item added later is shown too: a saved bar that names only some of the items.
    s.load_prefs_json(r#"{"interface": {"topBar": {"workspace": false}}}"#).unwrap();
    assert_eq!(s.prefs().interface.top_bar, TopBar { workspace: false, ..TopBar::default() });
}

#[test]
fn items_are_set_saved_and_reset_by_path() {
    let mut s = Session::new();
    for id in TopBar::IDS {
        let path = format!("interface.topBar.{id}");
        s.execute("prefs.set", json!({"path": path, "value": false})).unwrap();
        assert_eq!(s.prefs().interface.top_bar.shown(id), Some(false), "{id}");
        assert_eq!(s.execute("prefs.get", json!({"path": path})).unwrap(), json!(false));
    }
    let mut restored = Session::new();
    restored.load_prefs_json(&s.prefs_to_json()).unwrap();
    assert!(TopBar::IDS.iter().all(|id| restored.prefs().interface.top_bar.shown(id) == Some(false)));
    // One item back, then the whole bar back.
    s.execute("prefs.reset", json!({"path": "interface.topBar.search"})).unwrap();
    assert_eq!(s.prefs().interface.top_bar.shown("search"), Some(true));
    assert_eq!(s.prefs().interface.top_bar.shown("title"), Some(false));
    s.execute("prefs.reset", json!({"path": "interface.topBar"})).unwrap();
    assert_eq!(s.prefs().interface.top_bar, TopBar::default());
}

#[test]
fn a_bad_top_bar_value_is_refused_and_changes_nothing() {
    let mut s = Session::new();
    for (path, value) in [
        ("interface.topBar.workspace", json!("no")),
        ("interface.topBar.workspace", json!(null)),
        ("interface.topBar.workspace", json!(2)),
        ("interface.topBar.menus", json!(false)),
        ("interface.topBar", json!(false)),
    ] {
        assert!(s.execute("prefs.set", json!({"path": path, "value": value})).is_err(), "{path} {value}");
        assert_eq!(s.prefs().interface.top_bar, TopBar::default(), "{path} {value}");
    }
    // All or nothing, as for every `prefs.set`: a good item next to a bad one is not kept.
    let r = s.execute("prefs.set", json!({"values": {"interface.topBar.search": false, "interface.topBar.title": "x"}}));
    assert!(r.is_err());
    assert_eq!(s.prefs().interface.top_bar, TopBar::default());
}

#[test]
fn a_wrongly_typed_item_in_a_file_resets_only_itself() {
    let mut s = Session::new();
    let invalid = s.load_prefs_json(r#"{"interface": {"topBar": {"workspace": false, "search": "yes", "title": false}}}"#).unwrap();
    assert_eq!(s.prefs().interface.top_bar, TopBar { workspace: false, title: false, ..TopBar::default() });
    let paths: Vec<String> = invalid.iter().map(InvalidPref::path).collect();
    assert_eq!(paths, ["interface.topBar.search"]);
    // A top bar that isn't an object resets as a whole, the rest of the interface settings load.
    let invalid = s.load_prefs_json(r#"{"interface": {"topBar": 3, "showTooltips": false}}"#).unwrap();
    assert_eq!(s.prefs().interface.top_bar, TopBar::default());
    assert!(!s.prefs().interface.show_tooltips);
    assert_eq!(invalid.iter().map(InvalidPref::path).collect::<Vec<_>>(), ["interface.topBar"]);
}
