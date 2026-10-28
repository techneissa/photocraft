use super::super::*;

fn session() -> Session {
    Session::new()
}

fn call(s: &mut Session, p: Value) -> Result<Value> {
    s.execute("edit.keyboardShortcuts", p)
}

#[test]
fn chords_and_conditions_are_set_listed_and_removed_by_command() {
    let mut s = session();
    let r = call(&mut s, json!({"chords": {"edit.copy": "cmd+k c"}, "when": {"edit.copy": "!textFocus&&tool==brush"}})).unwrap();
    assert_eq!(r["chords"], json!({"edit.copy": "Cmd+K C"}));
    assert_eq!(r["when"], json!({"edit.copy": "!textFocus && tool == brush"}));
    let listed = call(&mut s, json!({"filter": "edit.copy"})).unwrap();
    let copy = listed["commands"].as_array().unwrap().iter().find(|c| c["id"] == "edit.copy").unwrap().clone();
    assert_eq!(copy["chord"], "Cmd+K C");
    assert_eq!(copy["when"], "!textFocus && tool == brush");
    assert_eq!(s.prefs().chords["edit.copy"], "Cmd+K C");
    call(&mut s, json!({"chords": {"edit.copy": null}, "when": {"edit.copy": ""}})).unwrap();
    assert!(s.prefs().chords.is_empty() && s.prefs().shortcut_when.is_empty());
}

#[test]
fn a_bad_chord_or_condition_changes_nothing() {
    let mut s = session();
    call(&mut s, json!({"chords": {"edit.copy": "Cmd+K C"}})).unwrap();
    let before = s.prefs().clone();
    for p in [
        json!({"chords": {"edit.cut": "Cmd+K X", "edit.paste": "Cmd+K"}}),
        json!({"chords": {"no.such.command": "Cmd+K C"}}),
        json!({"chords": ["x"]}),
        json!({"when": {"edit.cut": "tool =="}}),
        json!({"when": {"edit.cut": "hasDocument", "edit.paste": "bogus"}}),
        json!({"when": {"no.such.command": "hasDocument"}}),
        json!({"importJson": "{}"}),
        json!({"importJson": 5}),
        json!({"set": {"edit.cut": "F9"}, "chords": {"edit.paste": "nope"}}),
    ] {
        assert!(call(&mut s, p.clone()).is_err(), "{p}");
        assert_eq!(s.prefs(), &before, "{p}");
    }
}

#[test]
fn reset_clears_chords_and_conditions() {
    let mut s = session();
    call(&mut s, json!({"chords": {"edit.copy": "Cmd+K C", "edit.cut": "Cmd+K X"}, "when": {"edit.copy": "hasSelection", "edit.cut": "hasSelection"}}))
        .unwrap();
    call(&mut s, json!({"reset": ["edit.copy"]})).unwrap();
    assert_eq!(s.prefs().chords.keys().collect::<Vec<_>>(), ["edit.cut"]);
    assert_eq!(s.prefs().shortcut_when.keys().collect::<Vec<_>>(), ["edit.cut"]);
    call(&mut s, json!({"reset": true})).unwrap();
    assert!(s.prefs().chords.is_empty() && s.prefs().shortcut_when.is_empty());
}

#[test]
fn conflicts_respect_the_condition() {
    let mut s = session();
    call(&mut s, json!({"set": {"edit.copy": "F9", "edit.cut": "F9"}, "removeConflicts": false})).unwrap();
    let r = call(&mut s, json!({})).unwrap();
    assert_eq!(r["conflicts"][0]["shortcut"], "F9");
    let r = call(&mut s, json!({"when": {"edit.copy": "tool == brush", "edit.cut": "tool != brush"}})).unwrap();
    assert!(r["conflicts"].as_array().unwrap().is_empty(), "{r}");
    let r = call(&mut s, json!({"when": {"edit.cut": "hasSelection"}})).unwrap();
    assert_eq!(r["conflicts"][0]["shortcut"], "F9");
}

#[test]
fn a_chord_whose_leader_is_a_key_is_a_conflict() {
    let mut s = session();
    call(&mut s, json!({"set": {"edit.copy": "Cmd+K"}})).unwrap();
    let r = call(&mut s, json!({"chords": {"edit.cut": "Cmd+K C"}})).unwrap();
    assert_eq!(r["conflicts"][0]["shortcut"], "Cmd+K");
    assert_eq!(r["conflicts"][0]["commands"], json!(["edit.copy", "edit.cut"]));
}

#[test]
fn unreadable_conditions_in_old_settings_are_reported_and_ignored() {
    let mut s = session();
    let mut p = s.prefs().clone();
    p.shortcut_when.insert("edit.copy".into(), "tool ==".into());
    s.edit_prefs(|x| *x = p);
    let r = call(&mut s, json!({})).unwrap();
    assert_eq!(r["damagedWhen"][0]["id"], "edit.copy");
}

#[test]
fn export_json_and_import_json_round_trip_through_the_command() {
    let mut s = session();
    call(
        &mut s,
        json!({"set": {"edit.copy": "Cmd+J"}, "alternates": {"edit.paste": ["F6"]}, "chords": {"edit.undo": "Cmd+K Z"}, "when": {"edit.undo": "!dialogOpen"}, "removeConflicts": false}),
    )
    .unwrap();
    let out = call(&mut s, json!({"exportJson": true})).unwrap();
    let text = out["json"].as_str().unwrap().to_string();
    assert!(text.contains("photocraft-shortcuts"));
    let saved = s.prefs().clone();
    call(&mut s, json!({"reset": true})).unwrap();
    assert!(s.prefs().shortcuts.is_empty());
    let r = call(&mut s, json!({"importJson": text})).unwrap();
    assert_eq!(r["importJson"]["imported"], 3);
    assert_eq!(r["importJson"]["notices"], json!([]));
    assert_eq!(s.prefs().shortcuts, saved.shortcuts);
    assert_eq!(s.prefs().alternate_shortcuts, saved.alternate_shortcuts);
    assert_eq!(s.prefs().chords, saved.chords);
    assert_eq!(s.prefs().shortcut_when, saved.shortcut_when);
}

#[test]
fn import_json_from_a_path_and_refusals() {
    let dir = std::env::temp_dir().join(format!("pc-shortcuts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("keys.json");
    let mut s = session();
    call(&mut s, json!({"chords": {"edit.copy": "Cmd+K C"}})).unwrap();
    let out = call(&mut s, json!({"exportJson": {"path": path.to_str().unwrap()}})).unwrap();
    assert_eq!(out["jsonPath"], path.to_str().unwrap());
    call(&mut s, json!({"reset": true})).unwrap();
    call(&mut s, json!({"importJson": {"path": path.to_str().unwrap()}})).unwrap();
    assert_eq!(s.prefs().chords["edit.copy"], "Cmd+K C");
    let before = s.prefs().clone();
    std::fs::write(&path, r#"{"kind": "photocraft-shortcuts", "version": 99, "shortcuts": {}}"#).unwrap();
    let e = call(&mut s, json!({"importJson": {"path": path.to_str().unwrap()}})).unwrap_err().to_string();
    assert!(e.contains("newer PhotoCraft"), "{e}");
    std::fs::write(&path, "{broken").unwrap();
    assert!(call(&mut s, json!({"importJson": {"path": path.to_str().unwrap()}})).is_err());
    assert!(call(&mut s, json!({"importJson": {"path": dir.join("missing.json").to_str().unwrap()}})).is_err());
    assert_eq!(s.prefs(), &before, "a refused file changes nothing");
    std::fs::remove_dir_all(&dir).ok();
}
