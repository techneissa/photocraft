//! Opt-in pictures (`PHOTOCRAFT_UPDATE_SNAPSHOTS` names the folder): Settings › General with the
//! Check for Updates row, and the Check for Updates window. Sample state only; nothing is asked
//! of GitHub (the start check is marked done before the first frame).

use egui_kittest::Harness;
use serde_json::json;

use super::{Runtime, State};
use crate::PhotocraftApp;

fn build(state: State, open: bool) -> Harness<'static, PhotocraftApp> {
    Harness::builder().with_size(egui::vec2(1000.0, 640.0)).with_pixels_per_point(1.0).with_max_steps(64).wgpu().build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.fork_update_rt = Runtime { state, started: true, open, official_off: true, ..Runtime::default() };
        app
    })
}

#[test]
fn update_check_snapshots() {
    let Some(directory) = std::env::var_os("PHOTOCRAFT_UPDATE_SNAPSHOTS") else {
        eprintln!("set PHOTOCRAFT_UPDATE_SNAPSHOTS (a folder) to take the pictures");
        return;
    };
    let _gpu = photocraft_testkit::gpu_lock();
    let directory = std::path::PathBuf::from(directory);
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || take(&directory)).unwrap().join().unwrap();
}

fn take(directory: &std::path::Path) {
    std::fs::create_dir_all(directory).unwrap();
    for (name, state) in [("settings-ready", State::Ready), ("settings-up-to-date", State::UpToDate), ("settings-being-made", State::BeingMade)] {
        let mut h = build(state, false);
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "edit.preferences.general", json!({})).unwrap();
        h.run_steps(8);
        h.render().expect("render the window").save(directory.join(format!("{name}.png"))).unwrap();
    }
    for (name, state) in [("window-ready", State::Ready), ("window-failed", State::Failed("Couldn't reach GitHub (timed out).".into()))] {
        let mut h = build(state, true);
        h.run_steps(8);
        h.render().expect("render the window").save(directory.join(format!("{name}.png"))).unwrap();
    }
}
