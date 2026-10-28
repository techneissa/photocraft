use super::*;

fn customised() -> Preferences {
    let mut p = Preferences::default();
    p.shortcuts.insert("edit.copy".into(), "Cmd+J".into());
    p.shortcuts.insert("edit.cut".into(), String::new());
    p.alternate_shortcuts.insert("edit.paste".into(), vec!["F5".into()]);
    p.chords.insert("edit.undo".into(), "Cmd+K Z".into());
    p.shortcut_when.insert("edit.undo".into(), "!textFocus && tool == brush".into());
    p
}

#[test]
fn a_keymap_survives_a_round_trip_with_chords_and_when() {
    let p = customised();
    let doc = to_json(&p);
    assert_eq!(doc["kind"], KIND);
    assert_eq!(doc["version"], VERSION);
    let mut other = Preferences::default();
    other.shortcuts.insert("edit.fill".into(), "Cmd+9".into());
    let report = import_text(&mut other, &doc.to_string()).unwrap();
    assert_eq!(report.notices, Vec::<String>::new());
    assert_eq!(report.commands, 4);
    assert_eq!(other.shortcuts, p.shortcuts, "the file's keys replace the old ones");
    assert_eq!(other.alternate_shortcuts, p.alternate_shortcuts);
    assert_eq!(other.chords, p.chords);
    assert_eq!(other.shortcut_when, p.shortcut_when);
}

#[test]
fn only_what_the_user_changed_is_written() {
    let doc = to_json(&Preferences::default());
    assert_eq!(doc["shortcuts"], json!({}));
    let doc = to_json(&customised());
    assert_eq!(doc["shortcuts"]["edit.copy"], json!({"shortcut": "Cmd+J"}));
    assert_eq!(doc["shortcuts"]["edit.cut"], json!({"shortcut": ""}));
    assert_eq!(doc["shortcuts"]["edit.undo"], json!({"chord": "Cmd+K Z", "when": "!textFocus && tool == brush"}));
}

#[test]
fn a_file_that_is_not_ours_is_refused_and_changes_nothing() {
    let before = customised();
    let refused = |text: &str| {
        let mut p = before.clone();
        let err = import_text(&mut p, text).unwrap_err();
        assert_eq!(p, before, "{text:?} changed the keymap");
        err
    };
    assert!(refused("not json at all").contains("isn't readable JSON"));
    assert!(refused("").contains("isn't readable JSON"));
    assert!(refused("[]").contains("no \"kind\""));
    assert!(refused(r#"{"kind":"lightcraft-shortcuts","version":1,"shortcuts":{}}"#).contains("its kind is “lightcraft-shortcuts”"));
    assert!(refused(&format!(r#"{{"kind":"{KIND}","shortcuts":{{}}}}"#)).contains("no readable \"version\""));
    assert!(refused(&format!(r#"{{"kind":"{KIND}","version":0,"shortcuts":{{}}}}"#)).contains("no readable \"version\""));
    assert!(refused(&format!(r#"{{"kind":"{KIND}","version":"1","shortcuts":{{}}}}"#)).contains("no readable \"version\""));
    assert!(refused(&format!(r#"{{"kind":"{KIND}","version":{},"shortcuts":{{}}}}"#, VERSION + 1)).contains("newer PhotoCraft"));
    assert!(refused(&format!(r#"{{"kind":"{KIND}","version":1}}"#)).contains("no \"shortcuts\" list"));
    assert!(refused(&"x".repeat(MAX_BYTES + 1)).contains("too big"));
}

#[test]
fn a_byte_order_mark_is_fine() {
    let mut p = Preferences::default();
    let text = format!("\u{feff}{}", to_json(&customised()));
    assert_eq!(import_text(&mut p, &text).unwrap().commands, 4);
}

#[test]
fn unknown_ids_and_unusable_entries_are_skipped_with_a_notice() {
    let text = json!({
        "kind": KIND, "version": 1,
        "shortcuts": {
            "no.such.command": {"shortcut": "Cmd+J"},
            "edit.copy": {"shortcut": "Hyper+J", "chord": "Cmd+K", "when": "bogus ==", "alternates": ["F5", "Hyper+J"]},
            "edit.cut": "not an object",
            "edit.paste": {"shortcut": 5, "chord": 5},
            "edit.undo": {"chord": "Cmd+K Z", "when": "hasDocument"},
        }
    })
    .to_string();
    let mut p = Preferences::default();
    let report = import_text(&mut p, &text).unwrap();
    assert_eq!(report.commands, 1, "only edit.undo was usable");
    assert_eq!(report.notices.len(), 8, "{:?}", report.notices);
    assert!(report.notices.iter().any(|n| n.contains("no.such.command")));
    assert!(report.notices.iter().any(|n| n.contains("not a shortcut")));
    assert!(report.notices.iter().any(|n| n.contains("not a chord")));
    assert!(report.notices.iter().any(|n| n.contains("not a condition")));
    assert!(p.shortcuts.is_empty() && p.alternate_shortcuts.is_empty());
    assert_eq!(p.chords.len(), 1);
    assert_eq!(p.shortcut_when["edit.undo"], "hasDocument");
}

#[test]
fn what_is_not_a_command_is_kept() {
    let mut p = Preferences::default();
    p.shortcuts.insert("actions.play:Sepia".into(), "F7".into());
    p.shortcuts.insert("edit.copy".into(), "Cmd+J".into());
    import_text(&mut p, &to_json(&Preferences::default()).to_string()).unwrap();
    assert_eq!(p.shortcuts.keys().collect::<Vec<_>>(), ["actions.play:Sepia"]);
}

#[test]
fn a_reserved_key_is_not_imported() {
    let key = if cfg!(target_os = "macos") { "Cmd+Tab" } else { "Alt+Tab" };
    let text = json!({"kind": KIND, "version": 1, "shortcuts": {"edit.copy": {"shortcut": key, "chord": format!("{key} C")}}}).to_string();
    let mut p = Preferences::default();
    let report = import_text(&mut p, &text).unwrap();
    assert_eq!(report.commands, 0);
    assert_eq!(report.notices.len(), 2);
    assert!(report.notices.iter().all(|n| n.contains("reserved")));
}

#[test]
fn the_shell_can_veto_keys_it_cannot_press() {
    let text = json!({"kind": KIND, "version": 1, "shortcuts": {
        "edit.copy": {"shortcut": "Nope", "alternates": ["F5"]},
        "edit.cut": {"chord": "Cmd+K Nope"},
        "edit.paste": {"chord": "Cmd+K V"},
    }})
    .to_string();
    let mut p = Preferences::default();
    let ok = |k: &str| k != "Nope" && !k.ends_with("+Nope");
    let report = import_text_with(&mut p, &text, &ok).unwrap();
    assert_eq!(report.notices.len(), 2, "{:?}", report.notices);
    assert_eq!(p.alternate_shortcuts["edit.copy"], ["F5"]);
    assert_eq!(p.chords.keys().collect::<Vec<_>>(), ["edit.paste"]);
    assert!(p.shortcuts.is_empty());
}
