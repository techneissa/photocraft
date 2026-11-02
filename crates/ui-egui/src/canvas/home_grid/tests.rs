use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::thumbs::{self, Thumb};
use super::{fill_uv, grid_columns};
use crate::PhotocraftApp;

type Home = Harness<'static, (PhotocraftApp, bool)>;

fn home(app: PhotocraftApp, width: f32, height: f32) -> Home {
    let mut h = Harness::builder().with_size(egui::vec2(width, height)).build_ui_state(
        |ui, (app, ready): &mut (PhotocraftApp, bool)| {
            if *ready {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| super::super::tabs::document_area(app, ui));
            }
        },
        (app, false),
    );
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::default());
    h.state_mut().1 = true;
    h.run_steps(3);
    h
}

/// A `w`×`h` PNG of one colour at `path`.
fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: [u8; 3]) {
    let data: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    let img = photocraft_codecs::Image::from_raw(w, h, photocraft_codecs::ChannelLayout::Rgb, photocraft_codecs::SampleType::U8, data).unwrap();
    let bytes = photocraft_codecs::encode(&img, photocraft_codecs::Format::Png, &photocraft_codecs::EncodeOptions::default()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Runs frames until `done` holds or a few seconds pass (the previews decode on a thread).
fn wait_for(h: &mut Home, mut done: impl FnMut(&mut Home) -> bool) -> bool {
    for _ in 0..200 {
        if done(h) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.run_steps(1);
    }
    false
}

fn thumb_of(h: &Home, path: &str) -> Thumb {
    let cache = thumbs::cache(&h.ctx);
    let mut c = cache.lock().unwrap();
    c.thumb(&h.ctx, path)
}

#[test]
fn four_to_six_columns_on_a_desktop_window_and_never_none() {
    assert_eq!(grid_columns(1000.0).0, 4);
    assert_eq!(grid_columns(1200.0).0, 5);
    assert_eq!(grid_columns(1440.0).0, 6);
    assert_eq!(grid_columns(3000.0).0, 6, "six at most");
    for width in [0.0, -50.0, 120.0, 390.0, f32::NAN] {
        let (columns, tile, _) = grid_columns(width);
        assert!(columns >= 1 && tile >= 1.0, "{width}: {columns} columns of {tile}");
    }
    // The grid is centred and keeps clear of the edges.
    let (columns, tile, left) = grid_columns(1200.0);
    let right = left + columns as f32 * tile + (columns as f32 - 1.0) * super::TILE_GAP;
    assert!((left - (1200.0 - right)).abs() < 0.5 && left >= super::SIDE_SPACE, "{left} {right}");
}

#[test]
fn a_preview_fills_its_frame_and_is_cropped_in_the_middle() {
    let frame = egui::vec2(400.0, 300.0);
    let wide = fill_uv(egui::vec2(800.0, 400.0), frame);
    assert_eq!((wide.top(), wide.bottom()), (0.0, 1.0));
    assert!((wide.width() - 2.0 / 3.0).abs() < 1e-5 && (wide.center().x - 0.5).abs() < 1e-5, "{wide:?}");
    let tall = fill_uv(egui::vec2(300.0, 600.0), frame);
    assert_eq!((tall.left(), tall.right()), (0.0, 1.0));
    assert!((tall.height() - 0.375).abs() < 1e-5 && (tall.center().y - 0.5).abs() < 1e-5, "{tall:?}");
    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    assert_eq!(fill_uv(egui::vec2(0.0, 10.0), frame), full);
    assert_eq!(fill_uv(egui::vec2(10.0, 10.0), egui::vec2(0.0, 0.0)), full);
}

#[test]
fn recent_files_show_as_previews_under_the_buttons_and_the_links_are_gone() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let mut paths = Vec::new();
    for (name, rgb) in [("red.png", [220, 30, 30]), ("green.png", [30, 200, 60]), ("blue.png", [40, 60, 220])] {
        let p = dir.path().join(name);
        write_png(&p, 90, 60, rgb);
        paths.push(p.to_string_lossy().into_owned());
        app.push_recent(paths.last().unwrap());
    }
    let missing = dir.path().join("gone.psd").to_string_lossy().into_owned();
    app.push_recent(&missing);
    let mut h = home(app, 1200.0, 860.0);
    for name in ["red.png", "green.png", "blue.png", "gone.psd"] {
        assert!(h.query_by_label(name).is_some(), "{name} has a tile");
    }
    for link in ["PhotoCraft website", "GitHub", "ArtCraft"] {
        assert!(h.query_all_by_label_contains(link).next().is_none(), "no `{link}` link");
    }
    let button = h.query_all_by_label_contains("New document").next().unwrap().rect();
    assert!(button.top() < 860.0 / 3.0, "the buttons move to the top: {button:?}");
    // Five columns at 1200 points: the four tiles sit in one row under the buttons.
    let tiles: Vec<_> = ["red.png", "green.png", "blue.png", "gone.psd"].iter().map(|n| h.get_by_label(n).rect()).collect();
    assert!(tiles.iter().all(|r| r.top() > button.bottom() && (r.top() - tiles[0].top()).abs() < 0.5), "{tiles:?}");
    assert!(wait_for(&mut h, |h| paths.iter().all(|p| matches!(thumb_of(h, p), Thumb::Ready(..)))), "the previews are decoded");
    let Thumb::Ready(_, size) = thumb_of(&h, &paths[0]) else { panic!("not decoded") };
    assert_eq!(size, egui::vec2(90.0, 60.0), "a small picture is not scaled up");
    assert!(wait_for(&mut h, |h| thumb_of(h, &missing) == Thumb::Unavailable), "a missing file shows the file icon");
}

/// Preferences › Interface › Recent files on Home: the grid shows at most that many, newest
/// first; File › Open Recent keeps them all.
#[test]
fn the_grid_shows_as_many_recent_files_as_the_setting_allows() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    for name in ["d.png", "c.png", "b.png", "a.png"] {
        app.push_recent(&format!("/tmp/photocraft-home-limit/{name}"));
    }
    assert_eq!(app.session.prefs().interface.home_recent_count, 12, "12 by default");
    let mut h = home(app, 1200.0, 860.0);
    assert!(["a.png", "b.png", "c.png", "d.png"].iter().all(|n| h.query_by_label(n).is_some()));
    h.state_mut().0.run("prefs.set", serde_json::json!({"values": {"interface.homeRecentCount": 2}})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("a.png").is_some() && h.query_by_label("b.png").is_some(), "the newest two stay");
    assert!(h.query_by_label("c.png").is_none() && h.query_by_label("d.png").is_none());
    assert_eq!(h.state().0.ui.recent_files.len(), 4, "the list itself is kept");
    for bad in [0, 101] {
        assert!(h.state_mut().0.run("prefs.set", serde_json::json!({"values": {"interface.homeRecentCount": bad}})).is_err(), "{bad} is refused");
    }
    assert_eq!(h.state().0.session.prefs().interface.home_recent_count, 2);
}

/// The × shows in the corner of the hovered preview only, and takes the file off Recent without
/// opening it.
#[test]
fn hovering_a_preview_shows_its_remove_button() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    for name in ["b.png", "a.png"] {
        app.push_recent(&format!("/tmp/photocraft-home-x/{name}"));
    }
    let mut h = home(app, 1200.0, 860.0);
    assert!(h.query_by_label("Remove a.png from Recent").is_none());
    h.get_by_label("a.png").hover();
    h.run_steps(2);
    let tile = h.get_by_label("a.png").rect();
    let x = h.get_by_label("Remove a.png from Recent").rect();
    assert!(tile.contains_rect(x) && x.right() > tile.center().x && x.top() < tile.top() + 40.0, "top right: {x:?} in {tile:?}");
    assert!(h.query_by_label("Remove b.png from Recent").is_none(), "only on the hovered preview");
    h.get_by_label("Remove a.png from Recent").click();
    h.run_steps(2);
    assert_eq!(h.state().0.ui.recent_files, ["/tmp/photocraft-home-x/b.png"]);
    assert!(h.state().0.session.documents().is_empty(), "the file was not opened");
}

#[test]
fn without_recent_files_the_header_stays_in_the_middle() {
    let h = home(PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default()), 1200.0, 860.0);
    let button = h.query_all_by_label_contains("New document").next().unwrap().rect();
    assert!((button.center().y - 430.0).abs() < 80.0, "{button:?}");
}

#[test]
fn a_file_taken_off_recent_loses_its_preview_and_a_saved_file_gets_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    write_png(&a, 40, 30, [10, 10, 10]);
    write_png(&b, 40, 30, [10, 10, 10]);
    let (a, b) = (a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned());
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.push_recent(&a);
    app.push_recent(&b);
    let mut h = home(app, 1000.0, 760.0);
    assert!(wait_for(&mut h, |h| matches!(thumb_of(h, &b), Thumb::Ready(..))));
    h.state_mut().0.remove_recent(&a);
    h.run_steps(2);
    assert!(!thumbs::cache(&h.ctx).lock().unwrap().is_known(&a), "forgotten with its texture");
    // Saved again at another size: the next check, two seconds on, decodes it again.
    write_png(std::path::Path::new(&b), 64, 32, [200, 200, 200]);
    assert!(wait_for(&mut h, |h| matches!(thumb_of(h, &b), Thumb::Ready(_, s) if s == egui::vec2(64.0, 32.0))), "the new save is shown");
}
