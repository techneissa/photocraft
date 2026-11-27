use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::preview::{Sizes, load};
use crate::PhotocraftApp;

type Home = Harness<'static, Box<PhotocraftApp>>;

fn home(app: Box<PhotocraftApp>) -> Home {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 860.0))
        .with_max_steps(64)
        .with_context(app.ctx().clone())
        .build_ui_state(|ui, app: &mut Box<PhotocraftApp>| super::super::start_screen::start_screen(app, ui), app);
    PhotocraftApp::setup_context(&h.ctx, Default::default());
    h.run_steps(4);
    h
}

/// A `w`×`h` PNG of one colour at `path`.
fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: [u8; 3]) {
    let data: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    let img = photocraft_codecs::Image::from_raw(w, h, photocraft_codecs::ChannelLayout::Rgb, photocraft_codecs::SampleType::U8, data).unwrap();
    let bytes = photocraft_codecs::encode(&img, photocraft_codecs::Format::Png, &photocraft_codecs::EncodeOptions::default()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Home lists as many recent files as Preferences › Interface › Recent files on Home allows, newest first;
/// File › Open Recent keeps them all. The website, Discord and GitHub links stay off it.
#[test]
fn home_lists_as_many_recent_files_as_the_setting_allows_and_has_no_community_links() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    for name in ["d.png", "c.png", "b.png", "a.png"] {
        app.push_recent(&format!("/tmp/photocraft-home-limit/{name}"));
    }
    assert_eq!(app.session.prefs().interface.home_recent_count, 12, "12 by default");
    let mut h = home(app);
    let shown = |h: &Home, name: &str| h.query_all_by_label_contains(name).next().is_some();
    assert!(["a.png", "b.png", "c.png", "d.png"].iter().all(|n| shown(&h, n)));
    for link in ["Website", "Discord", "GitHub", "ArtCraft"] {
        assert!(h.query_all_by_label_contains(link).next().is_none(), "no `{link}` link");
    }
    h.state_mut().run("prefs.set", serde_json::json!({"values": {"interface.homeRecentCount": 2}})).unwrap();
    h.run_steps(4);
    assert!(shown(&h, "a.png") && shown(&h, "b.png"), "the newest two stay");
    assert!(!shown(&h, "c.png") && !shown(&h, "d.png"));
    assert_eq!(h.state().ui.recent_files.len(), 4, "the list itself is kept");
    for bad in [0, 101] {
        assert!(h.state_mut().run("prefs.set", serde_json::json!({"values": {"interface.homeRecentCount": bad}})).is_err(), "{bad} is refused");
    }
}

/// The list Home gets carries the fork's dim line: the size once a preview decoded it, and when the file was opened.
#[test]
fn a_recent_file_carries_its_size_and_when_it_was_opened() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let now = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).unwrap().as_secs();
    app.push_recent_at("/tmp/photocraft-home-note/a.png", now - 2 * 3600 - 10);
    app.push_recent_at("/tmp/photocraft-home-note/b.png", now - 5);
    let sizes = app.thumbnail_cache.lock().unwrap().sizes();
    sizes.set("/tmp/photocraft-home-note/a.png", Some((6000, 4000)));
    let files = super::recent_files(&app);
    assert_eq!(files[1].note.as_deref(), Some("6000 × 4000 · 2 h ago"));
    assert_eq!(files[0].note.as_deref(), Some("Just now"), "no size until a preview decoded one");
    assert_eq!(files[0].opened, Some(i64::try_from(now - 5).unwrap()));
}

/// A flat picture's size is noted from its header while the preview decodes; a file the loader can't read has none.
#[test]
fn a_decoded_picture_notes_its_size_and_an_unreadable_file_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png").to_string_lossy().into_owned();
    write_png(std::path::Path::new(&png), 90, 60, [10, 20, 30]);
    let sizes = Sizes::default();
    let some = |_: &str, _: u64| Some(egui::ColorImage::filled([1, 1], egui::Color32::RED));
    assert!(load(&png, 1, &sizes, some).is_some());
    assert_eq!(sizes.get(&png), Some((90, 60)));
    assert!(load(&png, 1, &sizes, |_, _| None).is_none());
    assert_eq!(sizes.get(&png), None, "a failed decode forgets the old size");
}

/// A PSD's preview is the merged image, not an import of its layers (the layer is green, the merged image red), and
/// its size is the file's own.
#[test]
fn a_psd_preview_is_the_merged_image_not_an_import_of_its_layers() {
    use craft_psd::{
        PsdBuilder,
        builder::{LayerSpec, PixelData},
    };
    let mut b = PsdBuilder::new(8, 8);
    b.push_layer(LayerSpec::new("Layer", 0, 0, 8, 8, PixelData::Rgba8([0, 255, 0, 255].repeat(64))));
    b.composite(PixelData::Rgba8([255, 0, 0, 255].repeat(64)));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.psd").to_string_lossy().into_owned();
    std::fs::write(&path, b.to_bytes().unwrap()).unwrap();
    let sizes = Sizes::default();
    let img = load(&path, 1, &sizes, |_, _| panic!("the official loader is not asked for a PSD with a preview")).unwrap();
    let px = img.pixels[0];
    assert!(px.r() > 230 && px.g() < 25, "{px:?}");
    assert_eq!(sizes.get(&path), Some((8, 8)));
}

#[test]
fn the_dim_line_gives_the_size_and_how_long_ago_the_file_was_opened() {
    use super::caption::line;
    let now = 1_762_171_200; // 3 Nov 2025, 12:00 UTC
    let size = Some((6000, 4000));
    assert_eq!(line(size, Some(now - 30), now), "6000 × 4000 · Just now");
    assert_eq!(line(size, Some(now - 5 * 60), now), "6000 × 4000 · 5 min ago");
    assert_eq!(line(size, Some(now - 2 * 3600 - 10), now), "6000 × 4000 · 2 h ago");
    assert_eq!(line(size, Some(now - 30 * 3600), now), "6000 × 4000 · Yesterday");
    assert_eq!(line(size, Some(now - 5 * 86_400), now), "6000 × 4000 · 29 Oct");
    assert_eq!(line(None, Some(now - 3600), now), "1 h ago", "no size until the preview is decoded");
    assert_eq!(line(Some((90, 60)), None, now), "90 × 60");
    assert_eq!(line(None, None, now), "");
}

#[test]
fn opening_a_file_records_its_time_and_taking_it_off_recent_forgets_it() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.push_recent_at("/tmp/a.png", 1000);
    app.push_recent_at("/tmp/b.png", 2000);
    app.push_recent_at("/tmp/a.png", 3000);
    let opened = app.session.prefs().file_handling.recent_opened.clone();
    assert_eq!((opened.get("/tmp/a.png"), opened.get("/tmp/b.png")), (Some(&3000), Some(&2000)), "opening again updates the time");
    app.remove_recent("/tmp/b.png");
    assert!(!app.session.prefs().file_handling.recent_opened.contains_key("/tmp/b.png"));
    app.clear_recent();
    assert!(app.session.prefs().file_handling.recent_opened.is_empty());
}
