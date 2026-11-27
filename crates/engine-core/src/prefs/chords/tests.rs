use super::*;
use serde_json::json;

fn prefs() -> Preferences {
    Preferences::default()
}

#[test]
fn a_chord_is_two_canonical_steps() {
    assert_eq!(normalize_chord("cmd+k c").as_deref(), Some("Cmd+K C"));
    assert_eq!(normalize_chord("  Shift+Cmd+K    Alt+C ").as_deref(), Some("Cmd+Shift+K Alt+C"));
    for bad in ["", "   ", "Cmd+K", "Cmd+K C D", "Cmd+ C", "Hyper+K C", "Cmd+K Cmd+Shift+"] {
        assert_eq!(normalize_chord(bad), None, "{bad:?}");
    }
    assert_eq!(chord_steps("Cmd+K C"), Some(["Cmd+K".to_string(), "C".to_string()]));
}

#[test]
fn a_chord_with_a_system_key_is_refused() {
    let key = if cfg!(target_os = "macos") { "Cmd+Tab" } else { "Alt+Tab" };
    assert!(check_chord(&format!("{key} C")).unwrap_err().contains("reserved"));
    assert!(check_chord(&format!("Cmd+K {key}")).unwrap_err().contains("reserved"));
    assert_eq!(check_chord("Cmd+K C").unwrap(), "Cmd+K C");
    assert!(check_chord("Cmd+K").unwrap_err().contains("not a chord"));
}

#[test]
fn conditions_are_canonical_and_bad_ones_are_errors() {
    assert_eq!(check_when("!dialogOpen&&tool==brush").unwrap(), "!dialogOpen && tool == brush");
    assert_eq!(check_when("  ").unwrap(), "");
    for bad in ["tool", "nope", "(textFocus", "tool == ", "a & b", "\u{0}"] {
        assert!(check_when(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn settings_paths_set_and_remove_one_command() {
    let mut p = prefs();
    p.set("chords.edit.copy", json!("cmd+k c")).unwrap();
    p.set("shortcutWhen.edit.copy", json!("textFocus||tool==brush")).unwrap();
    assert_eq!(p.chords["edit.copy"], "Cmd+K C");
    assert_eq!(p.shortcut_when["edit.copy"], "textFocus || tool == brush");
    assert!(p.set("chords.edit.copy", json!("Cmd+K")).is_err());
    assert!(p.set("chords.edit.copy", json!(5)).is_err());
    assert!(p.set("shortcutWhen.edit.copy", json!("bogus")).is_err());
    assert_eq!(p.chords["edit.copy"], "Cmd+K C", "a refused value changes nothing");
    p.set("chords.edit.copy", Value::Null).unwrap();
    p.set("shortcutWhen.edit.copy", json!("")).unwrap();
    assert!(p.chords.is_empty() && p.shortcut_when.is_empty());
}

#[test]
fn whole_maps_are_checked_before_they_replace_anything() {
    let mut p = prefs();
    p.set("chords", json!({"edit.copy": "Cmd+K C", "edit.cut": ""})).unwrap();
    assert_eq!(p.chords.len(), 1);
    assert!(p.set("chords", json!({"edit.paste": "Cmd+K V", "edit.cut": "nope"})).is_err());
    assert_eq!(p.chords.keys().collect::<Vec<_>>(), ["edit.copy"]);
    assert!(p.set("shortcutWhen", json!(["x"])).is_err());
    p.set("shortcutWhen", json!({"edit.copy": "hasSelection"})).unwrap();
    assert_eq!(p.shortcut_when["edit.copy"], "hasSelection");
}

#[test]
fn reset_removes_one_entry_or_the_map() {
    let mut p = prefs();
    p.set("chords", json!({"edit.copy": "Cmd+K C", "edit.cut": "Cmd+K X"})).unwrap();
    p.reset(Some("chords.edit.copy")).unwrap();
    assert_eq!(p.chords.keys().collect::<Vec<_>>(), ["edit.cut"]);
    p.reset(Some("chords")).unwrap();
    assert!(p.chords.is_empty());
    p.set("shortcutWhen.edit.copy", json!("hasPath")).unwrap();
    p.reset(None).unwrap();
    assert!(p.shortcut_when.is_empty());
}

#[test]
fn old_settings_without_the_fields_load_unchanged() {
    let old = json!({"shortcuts": {"edit.copy": "Cmd+J"}, "alternateShortcuts": {"edit.paste": ["F5"]}});
    let p: Preferences = serde_json::from_value(old).unwrap();
    assert!(p.chords.is_empty() && p.shortcut_when.is_empty());
    assert_eq!(p.shortcuts["edit.copy"], "Cmd+J");
    // and the new fields survive a round trip
    let mut q = p.clone();
    q.set("chords.edit.copy", json!("Cmd+K C")).unwrap();
    q.set("shortcutWhen.edit.copy", json!("!textFocus")).unwrap();
    let back: Preferences = serde_json::from_value(q.to_json()).unwrap();
    assert_eq!(back, q);
    assert_eq!(q.get("chords.edit.copy"), Some(json!("Cmd+K C")), "dotted ids are read through the map");
    assert_eq!(q.get("chords").unwrap()["edit.copy"], "Cmd+K C");
}

#[test]
fn damaged_conditions_are_listed_and_ignored() {
    let map = BTreeMap::from([("a".to_string(), "tool == brush".to_string()), ("b".to_string(), "tool ==".to_string())]);
    assert!(when_of(&map, "a").is_some());
    assert!(when_of(&map, "b").is_none(), "an unreadable condition is ignored, as if there were none");
    assert!(when_of(&map, "c").is_none());
    let d = damaged(&map);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].0, "b");
}

#[test]
fn clashes_that_differ_only_by_when_are_not_clashes() {
    let mut p = prefs();
    let bindings = [("x.one", "Cmd+J".to_string()), ("x.two", "Cmd+J".to_string()), ("x.three", "Cmd+L".to_string())];
    assert_eq!(clashes(&p, &bindings), vec![("Cmd+J".to_string(), vec!["x.one".to_string(), "x.two".to_string()])]);
    p.shortcut_when.insert("x.one".into(), "tool == brush".into());
    p.shortcut_when.insert("x.two".into(), "tool != brush".into());
    assert!(clashes(&p, &bindings).is_empty());
    // conditions that can hold together still clash
    p.shortcut_when.insert("x.two".into(), "hasSelection".into());
    assert_eq!(clashes(&p, &bindings).len(), 1);
    // one condition and none: they overlap
    p.shortcut_when.remove("x.two");
    assert_eq!(clashes(&p, &bindings).len(), 1);
}

#[test]
fn chords_clash_on_the_same_chord_and_on_a_leader_that_is_a_key() {
    let mut p = prefs();
    p.chords.insert("x.one".into(), "Cmd+K C".into());
    p.chords.insert("x.two".into(), "Cmd+K C".into());
    let found = clashes(&p, &[]);
    assert_eq!(found, vec![("Cmd+K C".to_string(), vec!["x.one".to_string(), "x.two".to_string()])]);
    p.chords.remove("x.two");
    assert!(clashes(&p, &[]).is_empty());
    // another command owns Cmd+K by itself: it would fire before the chord could finish
    let found = clashes(&p, &[("x.key", "Cmd+K".to_string())]);
    assert_eq!(found, vec![("Cmd+K".to_string(), vec!["x.key".to_string(), "x.one".to_string()])]);
    // unless their conditions exclude each other
    p.shortcut_when.insert("x.one".into(), "tool == brush".into());
    p.shortcut_when.insert("x.key".into(), "tool != brush".into());
    assert!(clashes(&p, &[("x.key", "Cmd+K".to_string())]).is_empty());
    // a command's own key may lead its own chord
    p.shortcut_when.clear();
    assert!(clashes(&p, &[("x.one", "Cmd+K".to_string())]).is_empty());
}
