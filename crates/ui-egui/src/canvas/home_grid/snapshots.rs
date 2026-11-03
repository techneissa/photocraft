//! Opt-in pictures of the Home screen with recent files, for a before and after:
//! `PHOTOCRAFT_HOME_SNAPSHOTS` names the output folder, `PHOTOCRAFT_HOME_PHASE` the file prefix
//! (`before` or `after`), `PHOTOCRAFT_HOME_SAMPLES` a folder whose `.jpg` files become the recent
//! files (public-domain pictures only), plus one missing file. Uses only the app's public state,
//! so the same file renders the official Home screen too.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::PhotocraftApp;

#[test]
fn home_screen_snapshots() {
    let (Some(out), Ok(phase), Some(samples)) =
        (std::env::var_os("PHOTOCRAFT_HOME_SNAPSHOTS"), std::env::var("PHOTOCRAFT_HOME_PHASE"), std::env::var_os("PHOTOCRAFT_HOME_SAMPLES"))
    else {
        eprintln!("set PHOTOCRAFT_HOME_SNAPSHOTS, PHOTOCRAFT_HOME_PHASE and PHOTOCRAFT_HOME_SAMPLES to take the pictures");
        return;
    };
    let _gpu = photocraft_testkit::gpu_lock();
    let (out, samples) = (std::path::PathBuf::from(out), std::path::PathBuf::from(samples));
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || take(&out, &phase, &samples)).unwrap().join().unwrap();
}

fn take(out: &std::path::Path, phase: &str, samples: &std::path::Path) {
    std::fs::create_dir_all(out).unwrap();
    let mut files: Vec<String> = std::fs::read_dir(samples)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jpg"))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    files.sort();
    files.push(samples.join("old-project.psd").to_string_lossy().into_owned());
    for (state, size) in [("wide", egui::vec2(1440.0, 900.0)), ("narrow", egui::vec2(1000.0, 760.0))] {
        let recent = files.clone();
        let mut h = Harness::builder().with_size(size).with_pixels_per_point(2.0).with_max_steps(64).wgpu().build_eframe(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
            // Opened a few minutes, hours and days ago (the last file, the missing one, has no time).
            let now = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).unwrap().as_secs();
            let ages = [120, 3 * 3600, 30 * 3600, 6 * 86_400, 40 * 86_400, 90 * 86_400, 200 * 86_400];
            let opened: std::collections::BTreeMap<String, u64> = recent.iter().zip(ages).map(|(f, age)| (f.clone(), now - age)).collect();
            app.session.edit_prefs(|p| {
                p.file_handling.recent_files = recent.clone();
                p.file_handling.recent_opened = opened.clone();
            });
            app.ui.recent_files = recent;
            app
        });
        // The previews decode on a thread: give them time.
        for _ in 0..150 {
            std::thread::sleep(std::time::Duration::from_millis(30));
            h.run_steps(1);
        }
        h.render().expect("render the window").save(out.join(format!("{phase}-{state}.png"))).unwrap();
        if state == "wide" {
            // The pointer on the newest file: its × (and the row or tile highlight).
            h.get_by_label("earthrise-crop.jpg").hover();
            h.run_steps(3);
            h.render().expect("render the window").save(out.join(format!("{phase}-hover.png"))).unwrap();
        }
    }
}
