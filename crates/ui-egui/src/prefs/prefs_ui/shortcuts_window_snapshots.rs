//! Opt-in pictures of Edit › Keyboard Shortcuts and Menus (`PHOTOCRAFT_SHORTCUT_SNAPSHOTS` names
//! the folder, `PHOTOCRAFT_SHORTCUT_PHASE` the file prefix, `before` or `after`): the same views at
//! the same size with the same sample data, for a before and after of the window.

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::PhotocraftApp;

fn build() -> Harness<'static, PhotocraftApp> {
    Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_pixels_per_point(1.0).with_max_steps(64).wgpu().build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        // Sample data only: recent files with made-up names, and one changed shortcut.
        app.session.edit_prefs(|p| {
            p.file_handling.recent_files = ["sample-1.png", "sample-2.png", "sample-3.png", "sample-4.png"].map(String::from).to_vec();
            p.shortcuts.insert("file.open".into(), "Cmd+Alt+O".into());
        });
        app.ui.recent_files = app.session.prefs().file_handling.recent_files.clone();
        app
    })
}

fn open(command: &str, fields: &[(&str, Value)]) -> (Harness<'static, PhotocraftApp>, u64) {
    let mut h = build();
    let ctx = h.ctx.clone();
    let id = crate::menus::invoke(h.state_mut(), &ctx, command, json!({})).unwrap()["dialog"].as_u64().unwrap();
    let f = &mut h.state_mut().ui.dialog_mut(id).unwrap().fields;
    for (name, v) in fields {
        f.insert((*name).into(), v.clone());
    }
    h.run_steps(8);
    (h, id)
}

fn set_field(h: &mut Harness<'_, PhotocraftApp>, id: u64, name: &str, v: Value) {
    h.state_mut().ui.dialog_mut(id).unwrap().fields.insert(name.into(), v);
}

fn press(h: &mut Harness<'_, PhotocraftApp>, key: Key, modifiers: Modifiers) {
    h.event(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run_steps(2);
    h.event(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run_steps(2);
}

#[test]
fn shortcuts_window_snapshots() {
    let (Some(directory), Some(phase)) = (std::env::var_os("PHOTOCRAFT_SHORTCUT_SNAPSHOTS"), std::env::var("PHOTOCRAFT_SHORTCUT_PHASE").ok()) else {
        eprintln!("set PHOTOCRAFT_SHORTCUT_SNAPSHOTS (folder) and PHOTOCRAFT_SHORTCUT_PHASE (before or after) to take the pictures");
        return;
    };
    let _gpu = photocraft_testkit::gpu_lock();
    let directory = std::path::PathBuf::from(directory);
    // Many harnesses in one function need more than a test thread's stack in a debug build.
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || take(&directory, &phase)).unwrap().join().unwrap();
}

fn take(directory: &std::path::Path, phase: &str) {
    std::fs::create_dir_all(directory).unwrap();
    let save = |h: &mut Harness<'_, PhotocraftApp>, state: &str| {
        h.render().expect("render the window").save(directory.join(format!("{phase}-{state}.png"))).unwrap();
    };
    let on_new = [("selected", json!("file.new"))];

    let (mut h, _) = open("edit.keyboardShortcuts", &on_new);
    save(&mut h, "shortcuts");

    let (mut h, _) = open("edit.keyboardShortcuts", &[("selected", json!("file.new")), ("capture", json!(true))]);
    save(&mut h, "recording");

    // A key already in use (⌘S saves): before it is assigned at once, after it is only shown.
    press(&mut h, Key::S, Modifiers::COMMAND);
    save(&mut h, "recording-clash");

    if phase == "after" {
        let (mut h, _) = open("edit.keyboardShortcuts", &[("selected", json!("file.new")), ("capture", json!(true))]);
        press(&mut h, Key::K, Modifiers::COMMAND);
        press(&mut h, Key::J, Modifiers::COMMAND);
        save(&mut h, "recording-chord");

        // The right-click menu of a row, and the reset icon's tooltip.
        let (mut h, _) = open("edit.keyboardShortcuts", &on_new);
        h.get_by_label(&crate::shortcuts::pretty("Cmd+N")).click_secondary();
        h.run_steps(6);
        save(&mut h, "context-menu");
        let (mut h, _) = open("edit.keyboardShortcuts", &on_new);
        h.get_by_label(&format!("Reset to {}", crate::shortcuts::pretty("Cmd+O"))).hover();
        h.run_steps(10);
        save(&mut h, "reset-tooltip");

        // Menus: the colour popover, with two items coloured.
        let (mut h, id) = open("edit.menus", &on_new);
        set_field(&mut h, id, "colors", json!({"file.open": "blue", "file.close": "green", "file.save": "red", "file.revert": "orange"}));
        h.run_steps(4);
        h.query_all_by_label_contains("Color: None").nth(1).unwrap().click();
        h.run_steps(6);
        save(&mut h, "menus-color-popover");
    }

    let (mut h, _) = open("edit.keyboardShortcuts", &[("filter", json!("cmd shift")), ("selected", json!(""))]);
    save(&mut h, "search-cmd-shift");

    let (mut h, _) = open("edit.menus", &on_new);
    save(&mut h, "menus");

    let (mut h, _) = open("edit.toolbar", &[]);
    save(&mut h, "toolbar");
}
