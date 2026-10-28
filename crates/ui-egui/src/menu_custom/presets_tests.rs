use serde_json::{Value, json};

use super::menus::{CANVAS_NONE, LAYERS_ROW, TAB};
use super::presets::COUNT;
use super::*;

fn arrangement(items: &[&str], hidden: &[&str]) -> Arrangement {
    Arrangement { items: items.iter().map(|s| s.to_string()).collect(), hidden: hidden.iter().map(|s| s.to_string()).collect() }
}

fn with(key: &str, a: Arrangement) -> Custom {
    Custom { menus: [(key.to_string(), a)].into(), ..Default::default() }
}

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
}

#[test]
fn slots_save_switch_and_say_when_empty() {
    let mut c = with(LAYERS_ROW, arrangement(&["cmd:layer.delete"], &["layer.exportAs"]));
    assert!(c.states().iter().all(|s| !s.saved && !s.active));
    assert!(c.switch_to(1).unwrap_err().contains("Preset 2 is empty"));
    c.save_to(0).unwrap();
    let s = c.states();
    assert!(s[0].saved && s[0].active && !s[0].edited && !s[1].saved);
    // an edit shows on the active slot
    c.menus.insert(TAB.into(), arrangement(&[], &["file.closeAll"]));
    assert!(c.states()[0].edited);
    assert!(c.save_to(COUNT).unwrap_err().contains("from 1 to 3"));
    assert!(c.switch_to(COUNT).is_err() && c.clear(COUNT).is_err());
}

#[test]
fn a_switch_can_be_taken_back_until_the_menus_change() {
    let mut c = with(LAYERS_ROW, arrangement(&["cmd:layer.delete"], &[]));
    c.save_to(0).unwrap();
    c.menus = with(CANVAS_NONE, arrangement(&["cmd:select.all"], &[])).menus;
    c.save_to(1).unwrap();
    c.switch_to(0).unwrap();
    assert!(c.menus.contains_key(LAYERS_ROW) && !c.menus.contains_key(CANVAS_NONE));
    assert!(c.states()[0].active && !c.states()[1].active);
    assert!(c.can_undo());
    c.undo().unwrap();
    assert!(c.menus.contains_key(CANVAS_NONE) && c.states()[1].active);
    assert!(!c.can_undo() && c.undo().is_err());
    // an edit after a switch leaves nothing to take back
    c.switch_to(0).unwrap();
    c.menus.insert(TAB.into(), arrangement(&[], &["file.closeAll"]));
    assert!(!c.can_undo());
    // switching to what is already there records nothing
    let mut d = with(LAYERS_ROW, arrangement(&["cmd:layer.delete"], &[]));
    d.save_to(2).unwrap();
    d.switch_to(2).unwrap();
    assert!(!d.can_undo());
}

#[test]
fn the_default_menus_are_a_preset_too() {
    let mut c = Custom::default();
    c.save_to(1).unwrap();
    c.menus.insert(TAB.into(), arrangement(&[], &["file.closeAll"]));
    c.switch_to(1).unwrap();
    assert!(c.menus.is_empty() && c.states()[1].active && !c.states()[1].edited);
}

#[test]
fn clear_empties_a_slot_and_keeps_the_menus() {
    let mut c = with(TAB, arrangement(&[], &["file.closeAll"]));
    c.save_to(0).unwrap();
    assert!(c.clear(1).unwrap_err().contains("already empty"));
    c.clear(0).unwrap();
    assert!(c.presets.is_empty() && c.menus.contains_key(TAB));
    assert!(c.states().iter().all(|s| !s.saved && !s.active));
}

#[test]
fn presets_persist_in_the_preferences_and_a_broken_value_loads_as_none() {
    let mut c = with(TAB, arrangement(&[], &["file.closeAll"]));
    c.save_to(2).unwrap();
    let v = c.to_value();
    assert_eq!(Custom::from_value(&v), c);
    let json_text = serde_json::to_string(&v).unwrap();
    assert_eq!(Custom::from_value(&serde_json::from_str(&json_text).unwrap()), c);
    for junk in
        [json!({"presets": {"slots": [5, "x", []], "active": 9}}), json!({"presets": {"slots": [null, null, null], "active": 0}}), json!({"presets": 3})]
    {
        let p = Custom::from_value(&junk).presets;
        assert!(p.is_empty(), "{junk}");
    }
    // an active slot that is empty is dropped
    let p = Custom::from_value(&json!({"presets": {"slots": [{"tab.document": {"items": [], "hidden": ["file.close"]}}, null, null], "active": 2}})).presets;
    assert_eq!(p.active, None);
    assert!(p.get(0).is_some());
}

#[test]
fn export_then_import_restores_the_menus_and_the_presets() {
    let mut c = with(LAYERS_ROW, arrangement(&["cmd:layer.delete", "---", "cmd:edit.copy"], &["layer.exportAs"]));
    c.save_to(0).unwrap();
    c.menus = with(TAB, arrangement(&["cmd:file.closeAll"], &["file.close"])).menus;
    let text = backup::render(&c).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    assert_eq!((file["kind"].as_str(), file["version"].as_u64()), (Some("photocraft-context-menus"), Some(1)));
    let parsed = backup::parse(&text).unwrap();
    assert!(parsed.skipped.is_empty());
    assert_eq!(parsed.menus, c.menus);
    assert_eq!(parsed.presets.as_ref().unwrap().slots, c.presets.slots);
    assert_eq!(parsed.presets.unwrap().active, Some(0));
    // into the app, through the file the app writes
    let mut a = app();
    let dir = std::env::temp_dir().join(format!("pc-context-menus-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Context Menus.json").to_string_lossy().into_owned();
    a.services.write = Some(Box::new(|p: &str, bytes: &[u8]| std::fs::write(p, bytes).map_err(|e| e.to_string())));
    store(&mut a, &c);
    backup::export_to(&mut a, &path).unwrap();
    store(&mut a, &Custom::default());
    assert!(load(&a).menus.is_empty());
    backup::import_bytes(&mut a, "Context Menus.json", std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(load(&a).menus, c.menus);
    assert_eq!(load(&a).presets.slots, c.presets.slots);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_default_menus_export_and_import_as_the_default() {
    let text = backup::render(&Custom::default()).unwrap();
    let parsed = backup::parse(&text).unwrap();
    assert!(parsed.menus.is_empty() && parsed.presets.is_none() && parsed.skipped.is_empty());
    let mut a = app();
    store(&mut a, &with(TAB, arrangement(&[], &["file.closeAll"])));
    backup::import_text(&mut a, &text).unwrap();
    assert!(load(&a).menus.is_empty(), "a file without arrangements clears them");
}

#[test]
fn a_wrong_foreign_or_newer_file_is_refused_and_changes_nothing() {
    let mut a = app();
    let before = with(TAB, arrangement(&[], &["file.closeAll"]));
    store(&mut a, &before);
    for (text, why) in [
        ("not json", "not JSON"),
        ("[1]", "not a JSON object"),
        (r#"{"version": 1, "menus": {}}"#, "no kind"),
        (r#"{"kind": "lightcraft-photo-menu", "version": 1, "menus": {}}"#, "lightcraft-photo-menu"),
        (r#"{"kind": "photocraft-context-menus", "version": 2, "menus": {}}"#, "newer PhotoCraft"),
        (r#"{"kind": "photocraft-context-menus", "menus": {}}"#, "no valid version"),
        (r#"{"kind": "photocraft-context-menus", "version": 1}"#, "no `menus`"),
    ] {
        let e = backup::import_text(&mut a, text).unwrap_err();
        assert!(e.contains(why), "{text}: {e}");
        assert_eq!(load(&a), before, "{text}");
    }
    assert!(backup::import_bytes(&mut a, "x.json", vec![0xff, 0xfe]).is_err());
    assert!(backup::import_text(&mut a, &" ".repeat((1 << 20) + 1)).unwrap_err().contains("too big"));
    assert_eq!(load(&a), before);
}

#[test]
fn unknown_items_are_skipped_and_named() {
    let text = json!({
        "kind": "photocraft-context-menus", "version": 1,
        "menus": {
            "layers.row": {"items": ["cmd:layer.delete", "cmd:layer.delete", "cmd:from.the.future", "---", "cmd:edit.copy", "oops"], "hidden": ["layer.exportAs", "nope"]},
            "tab.document": {"items": ["cmd:edit.copy", "cmd:file.close"], "hidden": []},
            "menu.of.the.future": {"items": ["cmd:a"], "hidden": []}
        },
        "presets": {"slots": [{"canvas.noSelection": {"items": ["cmd:select.all", "cmd:gone"], "hidden": []}}, null, null], "active": 0}
    })
    .to_string();
    let parsed = backup::parse(&text).unwrap();
    assert_eq!(parsed.menus["layers.row"], arrangement(&["cmd:layer.delete", "---", "cmd:edit.copy"], &["layer.exportAs"]));
    // a command of the menu bar cannot be added to the tab menu
    assert_eq!(parsed.menus["tab.document"].items, ["cmd:file.close"]);
    assert!(!parsed.menus.contains_key("menu.of.the.future"));
    assert_eq!(parsed.presets.as_ref().unwrap().get(0).unwrap()["canvas.noSelection"].items, ["cmd:select.all"]);
    let skipped = parsed.skipped.join("|");
    for name in ["cmd:layer.delete", "cmd:from.the.future", "oops", "menu.of.the.future", "cmd:edit.copy", "cmd:gone"] {
        assert!(skipped.contains(name), "{name} in {skipped}");
    }
    assert!(backup::skipped_note(&parsed.skipped).contains("unknown items skipped"));
}
