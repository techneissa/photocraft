//! Opt-in pictures of Edit › Keyboard Shortcuts and Menus (`PHOTOCRAFT_SHORTCUT_SNAPSHOTS` names the folder,
//! `PHOTOCRAFT_SHORTCUT_PHASE` the file prefix): the tabs at one size with the same sample data.

use craft::menu::shortcuts::DialogState;
use egui_kittest::Harness;

use crate::PhotocraftApp;

fn build(tab: &'static str) -> Harness<'static, Box<PhotocraftApp>> {
    let mut h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_pixels_per_point(1.0).with_max_steps(64).wgpu().build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        // Sample data only.
        app.session.edit_prefs(|p| {
            p.chords.insert("tools.select.brush".into(), "Cmd+K B".into());
            p.chords.insert("tools.select.eraser".into(), "Cmd+K E".into());
            p.shortcut_when.insert("tools.select.brush".into(), "hasDocument && !dialogOpen".into());
            p.shortcut_when.insert("select.deselect".into(), "hasSelection".into());
            p.menus.colors.extend([
                ("file.open".to_string(), "blue".to_string()),
                ("file.save".to_string(), "red".to_string()),
                ("file.close".to_string(), "green".to_string()),
            ]);
        });
        crate::prefs_ui::open_shortcuts(&mut app, 0);
        if tab != "shortcuts"
            && let Some(s) = app.prefs_rt.shortcuts.as_mut()
        {
            s.state = DialogState::new().with_tab(tab);
        }
        app
    });
    h.run_steps(8);
    h
}

#[test]
fn shortcuts_window_snapshots() {
    let (Some(directory), Some(phase)) = (std::env::var_os("PHOTOCRAFT_SHORTCUT_SNAPSHOTS"), std::env::var("PHOTOCRAFT_SHORTCUT_PHASE").ok()) else {
        eprintln!("set PHOTOCRAFT_SHORTCUT_SNAPSHOTS (folder) and PHOTOCRAFT_SHORTCUT_PHASE to take the pictures");
        return;
    };
    let _gpu = photocraft_testkit::gpu_lock();
    let directory = std::path::PathBuf::from(directory);
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            std::fs::create_dir_all(&directory).unwrap();
            for tab in ["shortcuts", "chords", "menus"] {
                let mut h = build(tab);
                h.render().expect("render the window").save(directory.join(format!("{phase}-{tab}.png"))).unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
