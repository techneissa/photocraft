use serde_json::{Value, json};

use super::*;
use crate::Session;

/// Preferences with something changed in every part of the file.
fn customised() -> (Preferences, ColorSettings) {
    let mut p = Preferences::default();
    p.interface.top_bar.search = false;
    p.interface.top_bar.title = false;
    p.interface.show_tooltips = false;
    p.general.beep_when_done = true;
    p.export.jpeg_quality = 61;
    p.workspace.enable_narrow_options_bar = true;
    p.history_options.allow_non_linear_history = true;
    p.layer_mask_overlay.opacity = 33.0;
    p.shortcuts.insert("edit.copy".into(), "Cmd+Alt+C".into());
    p.shortcuts.insert("edit.paste".into(), String::new());
    p.chords.insert("edit.cut".into(), "Cmd+K X".into());
    p.shortcut_when.insert("edit.cut".into(), "!dialogOpen".into());
    p.shortcut_sets.insert("Mine".into(), ShortcutSet { shortcuts: [("edit.copy".to_string(), "Cmd+Alt+C".to_string())].into(), ..Default::default() });
    p.shortcut_set = "Mine".into();
    p.menus.hidden = vec!["file.print".into()];
    p.menus.colors.insert("edit.copy".into(), "red".into());
    p.toolbar.hidden = vec!["Hand".into()];
    p.toolbar.order = vec!["Move".into(), "Brush".into()];
    p.context_menus = json!({"menus": {"layers.row": {"items": ["cmd:layer.duplicate"], "hidden": []}}});
    p.workspaces.insert(
        "Painting".into(),
        json!({"panels": {"layers": true}, "dock": {"floating": [{"id": 1, "native": [1.0, 2.0, 3.0, 4.0], "rect": [0.0, 0.0, 10.0, 10.0]}]}}),
    );
    p.workspace_locked = true;
    p.panel_layout = json!({"workspace": "Painting", "dock": {"floating": [{"id": 2, "native": [5.0, 6.0, 7.0, 8.0]}]}});
    p.color_panel = json!({"ramp": true});
    p.brush_picker = json!({"scale": 2});
    p.tool_options = json!({"moveAutoSelect": true});
    p.view_extras = json!({"grid": true});
    p.user_dictionary = vec!["photocraft".into(), "techneissa".into()];
    let c = ColorSettings { ask_on_paste: false, blend_text_gamma: 1.0, ..Default::default() };
    (p, c)
}

fn text_of(p: &Preferences, c: &ColorSettings) -> String {
    to_text(p, c)
}

fn plan_of(p: &Preferences, c: &ColorSettings, text: &str) -> std::result::Result<Plan, String> {
    plan(p, c, text, &SettingsChecks::default())
}

/// The file as a JSON tree, to edit.
fn file() -> Value {
    let (p, c) = customised();
    serde_json::from_str(&text_of(&p, &c)).unwrap()
}

fn refused(v: &Value) -> String {
    let (p, c) = (Preferences::default(), ColorSettings::default());
    plan_of(&p, &c, &v.to_string()).unwrap_err()
}

#[test]
fn export_then_import_gives_back_the_same_settings() {
    let (p, c) = customised();
    let text = text_of(&p, &c);
    let (mut other, mut other_c) = (Preferences::default(), ColorSettings::default());
    other.general.beep_when_done = false;
    other_c.ask_on_paste = true;
    let plan = plan_of(&other, &other_c, &text).unwrap();
    assert_eq!(plan.parts, PARTS.to_vec());
    assert_eq!(to_json(&plan.prefs, &plan.color), to_json(&p, &c), "the file of the result equals the file exported");
    assert_eq!(plan.prefs.interface.top_bar, p.interface.top_bar);
    assert_eq!(plan.prefs.shortcuts, p.shortcuts);
    assert_eq!(plan.prefs.chords, p.chords);
    assert_eq!(plan.prefs.shortcut_when, p.shortcut_when);
    assert_eq!(plan.prefs.shortcut_sets, p.shortcut_sets);
    assert_eq!(plan.prefs.shortcut_set, "Mine");
    assert_eq!(plan.prefs.menus, p.menus);
    assert_eq!(plan.prefs.toolbar, p.toolbar);
    assert_eq!(plan.prefs.context_menus, p.context_menus);
    assert_eq!(plan.prefs.user_dictionary, p.user_dictionary);
    assert_eq!(plan.prefs.tool_options, p.tool_options);
    assert!(plan.prefs.workspace_locked);
    assert!(!plan.color.ask_on_paste);
}

#[test]
fn a_session_round_trip_export_change_import_is_equal() {
    let mut s = Session::new();
    let (p, c) = customised();
    s.edit_prefs(|x| *x = p.clone());
    s.color.settings = c.clone();
    let out = s.execute("edit.preferences.exportSettings", json!({})).unwrap();
    let text = out["text"].as_str().unwrap().to_string();
    let before = s.settings_text();
    s.execute("prefs.reset", json!({})).unwrap();
    assert_ne!(s.settings_text(), before);
    let report = s.execute("edit.preferences.importSettings", json!({"text": text})).unwrap();
    assert_eq!(report["applied"], true);
    assert_eq!(s.settings_text(), before);
    assert!(!s.prefs().interface.top_bar.search);
}

#[test]
fn a_file_on_disk_round_trips_through_the_commands() {
    let dir = std::env::temp_dir().join(format!("pc-settings-sync-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Mine.pcsettings");
    let path = path.to_str().unwrap();
    let mut s = Session::new();
    s.edit_prefs(|x| x.export.jpeg_quality = 42);
    let out = s.execute("edit.preferences.exportSettings", json!({"path": path})).unwrap();
    assert_eq!(out["path"], path);
    assert!(out.get("text").is_none());
    s.edit_prefs(|x| x.export.jpeg_quality = 90);
    s.execute("edit.preferences.importSettings", json!({"path": path})).unwrap();
    assert_eq!(s.prefs().export.jpeg_quality, 42);
    assert!(s.execute("edit.preferences.exportSettings", json!({"path": dir.join("no/such/folder/x.pcsettings").to_str().unwrap()})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_is_left_out_stays_as_it_was_on_this_mac() {
    let (mut p, c) = customised();
    p.file_handling.recent_files = vec!["/Users/a/secret.psd".into()];
    p.file_handling.lut_folder = "/Users/a/luts".into();
    p.history_log.file_path = "/Users/a/log.txt".into();
    p.plug_ins.additional_plugins_folder = "/Users/a/plugins".into();
    p.performance.gpu_adapter = "nvidia".into();
    p.performance.memory_usage_mb = 99_999;
    p.recent_colors = vec!["#123abc".into()];
    p.last_brush = json!({"size": 9});
    p.selected_tool = json!("Brush");
    p.dialogs.insert("edit.fill".into(), json!({"x": 1}));
    let text = text_of(&p, &c);
    for private in ["secret.psd", "/Users/a", "nvidia", "99999", "#123abc", "edit.fill", "scratchDisks", "monitorProfile"] {
        assert!(!text.contains(private), "{private} must not be in the file");
    }
    let mut mine = Preferences::default();
    mine.file_handling.recent_files = vec!["/Users/b/mine.psd".into()];
    mine.performance.memory_usage_mb = 4096;
    mine.performance.gpu_adapter = "apple".into();
    mine.selected_tool = json!("Move");
    let mine_c = ColorSettings { monitor_profile: "sRGB".into(), ..Default::default() };
    let plan = plan_of(&mine, &mine_c, &text).unwrap();
    assert_eq!(plan.prefs.file_handling.recent_files, vec!["/Users/b/mine.psd".to_string()]);
    assert_eq!(plan.prefs.performance.memory_usage_mb, 4096);
    assert_eq!(plan.prefs.performance.gpu_adapter, "apple");
    assert_eq!(plan.prefs.selected_tool, json!("Move"));
    assert_eq!(plan.color.monitor_profile, "sRGB");
    assert_eq!(plan.prefs.export.jpeg_quality, 61, "the rest is taken");
}

#[test]
fn a_path_to_a_profile_stays_out_of_the_file() {
    let c = ColorSettings { working_rgb: "/Users/a/Profiles/mine.icc".into(), ..Default::default() };
    let v = to_json(&Preferences::default(), &c);
    assert!(v["colorSettings"].get("workingRgb").is_none());
    assert!(v["colorSettings"].get("workingCmyk").is_some());
}

#[test]
fn floating_window_screen_positions_stay_out() {
    let (p, c) = customised();
    let v = to_json(&p, &c);
    assert!(!v.to_string().contains("native"), "screen positions are tied to one screen");
    assert!(v["workspaces"]["saved"]["Painting"]["dock"]["floating"][0].get("rect").is_some(), "the layout itself stays");
}

#[test]
fn every_preferences_key_is_in_the_file_or_listed_as_left_out() {
    let all = Preferences::default().to_json();
    for key in all.as_object().unwrap().keys() {
        let known = section_ids().any(|s| s == key) || PART_KEYS.contains(&key.as_str()) || NOT_IN_FILE.iter().any(|(k, _)| k == key);
        assert!(known, "`{key}` is a new part of the preferences: put it in the settings file (settings_sync.rs) or in NOT_IN_FILE, with a reason");
    }
    for (path, _) in LEFT_OUT {
        let found = path.split('.').try_fold(&all, |v, k| v.get(k)).is_some();
        assert!(found, "LEFT_OUT lists `{path}`, which is not a preference");
    }
}

#[test]
fn the_default_settings_import_into_anything() {
    let text = text_of(&Preferences::default(), &ColorSettings::default());
    let (p, c) = customised();
    let plan = plan_of(&p, &c, &text).unwrap();
    assert_eq!(to_json(&plan.prefs, &plan.color), to_json(&Preferences::default(), &ColorSettings::default()));
}

#[test]
fn a_file_that_is_not_ours_is_refused() {
    let (p, c) = (Preferences::default(), ColorSettings::default());
    for text in [
        "",
        "nope",
        "[]",
        "42",
        "null",
        "{}",
        r#"{"kind":"photocraft-shortcuts","version":1}"#,
        r#"{"kind":"photocraft-settings"}"#,
        r#"{"kind":"photocraft-settings","version":0}"#,
        r#"{"kind":"photocraft-settings","version":"1"}"#,
    ] {
        assert!(plan_of(&p, &c, text).is_err(), "{text}");
    }
    let e = plan_of(&p, &c, r#"{"kind":"photocraft-shortcuts","version":1}"#).unwrap_err();
    assert!(e.contains("photocraft-shortcuts"), "{e}");
    let empty = plan_of(&p, &c, r#"{"kind":"photocraft-settings","version":1}"#).unwrap_err();
    assert!(empty.contains("no settings"), "{empty}");
}

#[test]
fn a_bad_value_refuses_the_whole_file_and_changes_nothing() {
    type Edit = Box<dyn Fn(&mut Value)>;
    let cases: Vec<(&str, Edit)> = vec![
        ("jpegQuality", Box::new(|f| f["preferences"]["export"]["jpegQuality"] = json!(0))),
        ("wrong type", Box::new(|f| f["preferences"]["export"]["jpegQuality"] = json!("high"))),
        ("unknown choice", Box::new(|f| f["preferences"]["interface"]["uiScale"] = json!("77"))),
        ("unknown setting", Box::new(|f| f["preferences"]["interface"]["notASetting"] = json!(true))),
        ("unknown top bar item", Box::new(|f| f["preferences"]["interface"]["topBar"]["flag"] = json!(true))),
        ("unknown section", Box::new(|f| f["preferences"]["nope"] = json!({}))),
        ("section type", Box::new(|f| f["preferences"]["general"] = json!(3))),
        ("unknown part", Box::new(|f| f["extra"] = json!({}))),
        ("colour setting", Box::new(|f| f["colorSettings"]["intent"] = json!("sideways"))),
        ("unknown colour setting", Box::new(|f| f["colorSettings"]["nope"] = json!(1))),
        ("profile path", Box::new(|f| f["colorSettings"]["workingRgb"] = json!("/etc/passwd.icc"))),
        ("menu colour", Box::new(|f| f["menus"]["colors"]["edit.copy"] = json!("plaid"))),
        ("menus type", Box::new(|f| f["menus"] = json!([1]))),
        ("toolbar key", Box::new(|f| f["toolbar"]["extra"] = json!([]))),
        ("shortcuts file", Box::new(|f| f["shortcuts"]["bindings"]["kind"] = json!("other"))),
        ("shortcut set key", Box::new(|f| f["shortcuts"]["sets"]["Mine"]["shortcuts"]["edit.copy"] = json!("Banana+Q"))),
        ("shortcut set name", Box::new(|f| f["shortcuts"]["sets"][""] = json!({}))),
        ("active set", Box::new(|f| f["shortcuts"]["activeSet"] = json!("Ghost"))),
        ("workspaces", Box::new(|f| f["workspaces"]["saved"]["x"] = json!(3))),
        ("locked", Box::new(|f| f["workspaces"]["locked"] = json!("yes"))),
        ("remembered", Box::new(|f| f["remembered"]["toolOptions"] = json!([1]))),
        ("dictionary", Box::new(|f| f["dictionary"] = json!([1, 2]))),
        ("preferences type", Box::new(|f| f["preferences"] = json!("x"))),
    ];
    let mut s = Session::new();
    s.edit_prefs(|x| x.export.jpeg_quality = 77);
    let before = (s.settings_text(), s.prefs.rev());
    for (name, edit) in cases {
        let mut f = file();
        edit(&mut f);
        let e = s.execute("edit.preferences.importSettings", json!({"text": f.to_string()})).unwrap_err().to_string();
        assert!(e.contains("Nothing was changed"), "{name}: {e}");
        assert_eq!((s.settings_text(), s.prefs.rev()), before, "{name} changed something");
    }
}

#[test]
fn the_error_names_the_setting_and_counts_the_rest() {
    let mut f = file();
    f["preferences"]["export"]["jpegQuality"] = json!(0);
    f["preferences"]["export"]["webpQuality"] = json!(500);
    let e = refused(&f);
    assert!(e.contains("jpegQuality") || e.contains("webpQuality"), "{e}");
    assert!(e.contains("and 1 more"), "{e}");
    assert!(e.chars().count() < 300, "a short error: {e}");
}

#[test]
fn a_newer_file_gives_what_it_understands_and_names_the_rest() {
    let mut f = file();
    f["version"] = json!(VERSION + 1);
    f["preferences"]["export"]["jpegQuality"] = json!(12);
    f["preferences"]["interface"]["uiScale"] = json!("77");
    f["preferences"]["interface"]["notASetting"] = json!(true);
    f["preferences"]["futureSection"] = json!({"a": 1});
    f["futurePart"] = json!({});
    let (p, c) = (Preferences::default(), ColorSettings::default());
    let plan = plan_of(&p, &c, &f.to_string()).unwrap();
    assert_eq!(plan.prefs.export.jpeg_quality, 12, "what it understands is taken");
    assert_eq!(plan.prefs.interface.ui_scale, p.interface.ui_scale, "a choice it doesn't know is skipped");
    let notes = plan.notes.join("\n");
    for named in ["newer PhotoCraft", "uiScale", "notASetting", "futureSection", "futurePart"] {
        assert!(notes.contains(named), "{named} not named in: {notes}");
    }
}

#[test]
fn a_partial_file_changes_only_what_it_has() {
    let text = json!({"kind": KIND, "version": 1, "preferences": {"interface": {"topBar": {"search": false}}, "export": {"jpegQuality": 33}}}).to_string();
    let (mut p, c) = customised();
    p.interface.top_bar.search = true;
    p.interface.top_bar.title = true;
    let plan = plan_of(&p, &c, &text).unwrap();
    assert_eq!(plan.parts, vec!["preferences"]);
    assert!(!plan.prefs.interface.top_bar.search);
    assert!(plan.prefs.interface.top_bar.title, "a key the file lacks stays");
    assert_eq!(plan.prefs.export.jpeg_quality, 33);
    assert_eq!(plan.prefs.shortcuts, p.shortcuts, "a part the file lacks stays");
    assert_eq!(plan.prefs.menus, p.menus);
    assert_eq!(plan.prefs.user_dictionary, p.user_dictionary);
    assert_eq!(plan.color.ask_on_paste, c.ask_on_paste);
}

#[test]
fn a_shortcut_the_shell_cannot_press_is_skipped_and_named() {
    let (p, c) = customised();
    let checks = SettingsChecks { key_ok: |k| k != "Cmd+Alt+C", ..Default::default() };
    let plan = plan(&Preferences::default(), &c, &text_of(&p, &c), &checks).unwrap();
    assert!(plan.notes.iter().any(|n| n.contains("Cmd+Alt+C")), "{:?}", plan.notes);
    assert!(!plan.prefs.shortcuts.contains_key("edit.copy"));
}

#[test]
fn tools_and_context_menus_the_shell_does_not_know_are_left_out_and_named() {
    let (p, c) = customised();
    let checks = SettingsChecks { tool_ok: |t| t != "Hand", context_menus: |_| Ok((json!({"menus": {}}), vec!["layer.gone".into()])), ..Default::default() };
    let plan = plan(&Preferences::default(), &c, &text_of(&p, &c), &checks).unwrap();
    assert!(plan.prefs.toolbar.hidden.is_empty());
    assert_eq!(plan.prefs.toolbar.order, vec!["Move", "Brush"]);
    assert_eq!(plan.prefs.context_menus, json!({"menus": {}}));
    let notes = plan.notes.join("\n");
    assert!(notes.contains("Toolbar") && notes.contains("layer.gone"), "{notes}");
    let refusing = SettingsChecks { context_menus: |_| Err("no".into()), ..Default::default() };
    assert!(super::plan(&Preferences::default(), &c, &text_of(&p, &c), &refusing).is_err());
}

#[test]
fn a_dry_run_changes_nothing_and_reports() {
    let mut s = Session::new();
    let text = text_of(&customised().0, &customised().1);
    let before = s.settings_text();
    let report = s.execute("edit.preferences.importSettings", json!({"text": text, "dryRun": true})).unwrap();
    assert_eq!(report["applied"], false);
    assert_eq!(report["parts"].as_array().unwrap().len(), PARTS.len());
    assert_eq!(s.settings_text(), before);
    assert!(s.execute("edit.preferences.importSettings", json!({})).is_err());
    assert!(s.execute("edit.preferences.importSettings", json!({"path": "/no/such/file.pcsettings"})).is_err());
}

#[test]
fn the_commands_are_in_the_edit_preferences_menu() {
    for id in ["edit.preferences.exportSettings", "edit.preferences.importSettings"] {
        let spec = crate::commands::find(id).unwrap();
        assert_eq!(spec.menu, &["Edit", "Preferences"]);
    }
}

/// Every JSON value a leaf of the file could be replaced with.
fn mutants() -> Vec<Value> {
    vec![
        Value::Null,
        json!(true),
        json!(0),
        json!(-1),
        json!(1.5e300),
        json!(u64::MAX),
        json!(""),
        json!("x"),
        json!("Cmd+"),
        json!("a".repeat(5000)),
        json!([]),
        json!([null, [], {}]),
        json!({}),
        json!({"a": {"b": {"c": null}}}),
    ]
}

/// Visit every path of `v` (as keys), at most `limit` of them.
fn paths(v: &Value, here: &mut Vec<String>, out: &mut Vec<Vec<String>>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    out.push(here.clone());
    if let Value::Object(m) = v {
        for (k, x) in m {
            here.push(k.clone());
            paths(x, here, out, limit);
            here.pop();
        }
    }
}

fn set_at(root: &mut Value, path: &[String], value: Value) {
    let mut cur = root;
    for k in path {
        let Some(next) = cur.get_mut(k.as_str()) else { return };
        cur = next;
    }
    *cur = value;
}

#[test]
fn no_input_panics() {
    let (p, c) = customised();
    let good: Value = serde_json::from_str(&text_of(&p, &c)).unwrap();
    let base = (Preferences::default(), ColorSettings::default());
    let mut all = Vec::new();
    paths(&good, &mut Vec::new(), &mut all, 5000);
    assert!(all.len() > 200);
    let mut ran = 0;
    let mut applied = Session::new();
    let stride = (all.len() / 400).max(1);
    for path in all.iter().skip(1).step_by(stride) {
        for m in mutants() {
            let mut f = good.clone();
            set_at(&mut f, path, m);
            // Ok or Err, never a panic; and the result of an Ok is always applicable.
            if let Ok(plan) = plan(&base.0, &base.1, &f.to_string(), &SettingsChecks::default()) {
                applied.apply_settings_plan(plan);
                let _ = applied.settings_text();
            }
            ran += 1;
        }
    }
    assert!(ran > 3000, "{ran}");
    // Cut off anywhere, and arbitrary bytes.
    let text = text_of(&p, &c);
    for end in (0..text.len()).step_by(97) {
        if let Some(cut) = text.get(..end) {
            let _ = plan_of(&base.0, &base.1, cut);
        }
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..300 {
        let junk: String = (0..200)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                char::from_u32(((seed >> 33) % 0x2000) as u32).unwrap_or('?')
            })
            .collect();
        let _ = plan_of(&base.0, &base.1, &junk);
        let _ = plan_of(&base.0, &base.1, &format!(r#"{{"kind":"photocraft-settings","version":1,"preferences":{junk:?}}}"#));
    }
    let deep = format!("{}1{}", "[".repeat(10_000), "]".repeat(10_000));
    let _ = plan_of(&base.0, &base.1, &format!(r#"{{"kind":"photocraft-settings","version":1,"remembered":{{"toolOptions":{deep}}}}}"#));
    let big = format!(r#"{{"kind":"photocraft-settings","version":1,"pad":"{}"}}"#, "x".repeat(MAX_BYTES));
    assert!(plan_of(&base.0, &base.1, &big).unwrap_err().contains("too big"));
}

#[test]
fn the_notes_stay_bounded() {
    let mut f = json!({"kind": KIND, "version": 2, "preferences": {"general": {}}});
    for n in 0..1000 {
        f["preferences"]["general"][format!("nope{n}")] = json!(1);
    }
    let plan = plan_of(&Preferences::default(), &ColorSettings::default(), &f.to_string()).unwrap();
    assert!(plan.notes.len() <= MAX_NOTES);
}

fn deny_ambient(id: &str, params: &Value) -> Result<()> {
    let path = params.get("path").and_then(Value::as_str).unwrap_or("");
    if id == "prefs.set" && (path.starts_with("historyLog") || path.starts_with("colorSettings")) {
        return Err(crate::EngineError::BadParams { cmd: id.into(), msg: "not from here".into() });
    }
    Ok(())
}

#[test]
fn a_session_that_is_not_trusted_cannot_set_what_prefs_set_would_refuse() {
    let mut s = Session::new();
    s.authorize = Some(deny_ambient);
    let mut other = Preferences::default();
    other.history_log.enabled = true;
    other.export.jpeg_quality = 21;
    let color = ColorSettings { ask_on_paste: false, ..Default::default() };
    let text = text_of(&other, &color);
    let report = s.execute("edit.preferences.importSettings", json!({"text": text})).unwrap();
    assert!(!s.prefs().history_log.enabled, "History Log stays as it was");
    assert!(s.color.settings.ask_on_paste, "Color Settings stay as they were");
    assert_eq!(s.prefs().export.jpeg_quality, 21, "the rest is taken");
    let skipped = report["skipped"].to_string();
    assert!(skipped.contains("historyLog.enabled") && skipped.contains("colorSettings"), "{skipped}");
}
