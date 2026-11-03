//! Fork: render a dialog opening for the first time, offscreen, after another command-less dialog
//! was moved (`--moved X,Y` seeds the place the old code shared between them).
//!
//! ```sh
//! cargo run -p photocraft-ui-egui --example dialog_place_snapshot -- \
//!     --out settings.png --size 1440x900 --moved 900,520 --script '[["ui.menu.invoke", {"id": "edit.preferences.interface"}]]'
//! ```
//! `--load-prompt DIR` opens the Load Settings from Folder prompt (a sync folder in DIR).
use photocraft_ui_egui::control::{ControlRequest, Outcome, handle};
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::Value;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn pair(s: &str, sep: char) -> Option<(f32, f32)> {
    let (a, b) = s.split_once(sep)?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = arg(&args, "--out").unwrap_or_else(|| "dialog.png".into());
    let (w, h) = arg(&args, "--size").and_then(|s| pair(&s, 'x')).unwrap_or((1440.0, 900.0));
    let moved = arg(&args, "--moved").and_then(|s| pair(&s, ','));
    let script: Vec<(String, Value)> = arg(&args, "--script").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut render_options = eframe::egui_wgpu::RendererOptions::PREDICTABLE;
    render_options.predictable_texture_filtering = true;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(w, h))
        .with_pixels_per_point(1.0)
        .with_max_steps(64)
        .with_render_options(render_options)
        .wgpu()
        .build_eframe(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            if let Some((x, y)) = moved {
                // Where the old code kept the place of every dialog without a command id.
                cc.egui_ctx.data_mut(|d| d.insert_persisted(egui::Id::unique(("dialog-place", "Command")), egui::vec2(x, y)));
            }
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
            if let Some(rs) = cc.wgpu_render_state.as_ref() {
                app.set_wgpu(rs.clone());
            }
            app
        });
    harness.run_steps(4);
    let ctx = harness.ctx.clone();
    // `--load-prompt DIR`: sync folder DIR, then Edit > Preferences > Load Settings from Folder
    // (the confirm prompt with Cancel / Import), as a person opens it.
    if let Some(dir) = arg(&args, "--load-prompt") {
        let app = harness.state_mut();
        std::fs::create_dir_all(&dir).expect("create the sync folder");
        app.run("prefs.set", serde_json::json!({"path": "sync.folder", "value": dir})).expect("choose the folder");
        app.run("edit.preferences.saveSettingsToFolder", serde_json::json!({"dryRun": false})).expect("save a file to load");
        // The file says which Mac saved it: give it a neutral name for the picture.
        let file = std::path::Path::new(&dir).join("photocraft.pcsettings");
        let mut text: Value = serde_json::from_str(&std::fs::read_to_string(&file).expect("read the file")).expect("json");
        text["savedBy"]["computer"] = "Studio Mac".into();
        std::fs::write(&file, text.to_string()).expect("write the file");
        let ctx = harness.ctx.clone();
        let (req, _rx) = ControlRequest::new("ui.menu.invoke", serde_json::json!({"id": "edit.preferences.loadSettingsFromFolder"}));
        eprintln!("load: {:?}", matches!(handle(harness.state_mut(), &ctx, &req), Outcome::Done(_)));
        harness.run_steps(6);
    }
    for (method, params) in script {
        let (req, _rx) = ControlRequest::new(&method, params);
        if let Outcome::Done(v) = handle(harness.state_mut(), &ctx, &req)
            && v.get("ok") == Some(&Value::Bool(false))
        {
            eprintln!("{method}: {v}");
        }
        harness.run_steps(6);
    }
    for _ in 0..12 {
        harness.step();
    }
    let img = harness.render().expect("render");
    img.save(&out).expect("save png");
    println!("wrote {out} ({}×{})", img.width(), img.height());
}
