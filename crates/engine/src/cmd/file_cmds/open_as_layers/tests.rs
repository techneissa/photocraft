use photocraft_color::ColorMode;
use photocraft_geom::Rect;
use serde_json::{Value, json};

use crate::file_cmds::save_doc;
use crate::{EngineError, Session};

fn tmp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("photocraft-open-as-layers-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.to_string_lossy().into_owned()
}

/// A solid-colour PNG of `w × h` (`depth` bits, `gray` or RGB) in `dir`; returns its path.
fn image(dir: &str, name: &str, w: u32, h: u32, color: &str, depth: u32, gray: bool) -> String {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "background": color, "depth": depth})).unwrap();
    if gray {
        s.execute("image.mode.grayscale", json!({})).unwrap();
    }
    let path = format!("{dir}/{name}");
    save_doc(&s.active().unwrap().doc, &path, None).unwrap();
    path
}

fn png(dir: &str, name: &str, w: u32, h: u32, color: &str) -> String {
    image(dir, name, w, h, color, 8, false)
}

fn bounds(s: &Session, layer: usize) -> Rect {
    s.active().unwrap().doc.layers[layer].surface().unwrap().content_bounds()
}

fn names(s: &Session) -> Vec<String> {
    s.active().unwrap().doc.layers.iter().map(|l| l.name.clone()).collect()
}

#[test]
fn opens_the_files_as_one_document_top_file_first_each_centred() {
    let dir = tmp("order");
    let top = png(&dir, "top.png", 40, 20, "#ff0000");
    let mid = png(&dir, "mid.png", 30, 30, "#00ff00");
    let bottom = png(&dir, "bottom.jpg.png", 20, 50, "#0000ff");
    let mut s = Session::new();
    let r = s.execute("file.openAsLayers", json!({"paths": [top, mid, bottom]})).unwrap();
    assert_eq!((r["layers"].as_u64(), r["skipped"].as_array().map(Vec::len)), (Some(3), Some(0)), "{r}");
    assert_eq!(s.documents().len(), 1);
    let st = s.active().unwrap();
    // The largest width × the largest height; unsaved.
    assert_eq!((st.doc.size.width, st.doc.size.height), (40, 50));
    assert!(st.path.is_none());
    // The first file is the top layer; names have no extension.
    assert_eq!(names(&s), ["bottom.jpg", "mid", "top"]);
    assert_eq!(s.active().unwrap().doc.name, "top");
    // Each image is centred on the canvas.
    assert_eq!(bounds(&s, 2), Rect::new(0, 15, 40, 35));
    assert_eq!(bounds(&s, 1), Rect::new(5, 10, 35, 40));
    assert_eq!(bounds(&s, 0), Rect::new(10, 0, 30, 50));
    // And the top file shows over the others at the centre.
    let px = photocraft_compose::render(&s.active().unwrap().doc, Rect::new(20, 25, 21, 26)).px[0];
    assert!(px[0] > 0.99 && px[1] < 0.01 && px[2] < 0.01, "{px:?}");
}

#[test]
fn the_name_is_given_or_the_first_files() {
    let dir = tmp("name");
    let a = png(&dir, "DSC01234.png", 8, 8, "#808080");
    let b = png(&dir, "DSC01235.png", 8, 8, "#808080");
    let mut s = Session::new();
    let r = s.execute("file.openAsLayers", json!({"paths": [a.clone(), b.clone()], "name": "  DSC01234 + 1 "})).unwrap();
    assert_eq!(r["name"], "DSC01234 + 1");
    assert_eq!(s.active().unwrap().doc.name, "DSC01234 + 1");
    s.execute("file.openAsLayers", json!({"paths": [a, b], "name": "  "})).unwrap();
    assert_eq!(s.active().unwrap().doc.name, "DSC01234");
    assert_eq!(s.documents().len(), 2);
}

#[test]
fn the_top_files_mode_and_depth_win() {
    let dir = tmp("mode");
    let top = image(&dir, "top16.png", 16, 16, "#808080", 16, false);
    let low = png(&dir, "low8.png", 16, 16, "#ff0000");
    let gray = image(&dir, "gray.png", 16, 16, "#808080", 8, true);
    let mut s = Session::new();
    s.execute("file.openAsLayers", json!({"paths": [top.clone(), low.clone()]})).unwrap();
    let expect = photocraft_io::import("top16.png", &std::fs::read(&top).unwrap()).unwrap().document;
    let doc = &s.active().unwrap().doc;
    assert_eq!((doc.mode, doc.depth), (expect.mode, expect.depth));
    assert_eq!(doc.layers[0].surface().unwrap().format(), doc.pixel_format());
    s.execute("file.openAsLayers", json!({"paths": [gray, low]})).unwrap();
    let doc = &s.active().unwrap().doc;
    assert_eq!(doc.mode, ColorMode::Grayscale);
    assert_eq!(doc.layers[0].surface().unwrap().format(), doc.pixel_format());
}

#[test]
fn a_missing_or_unreadable_file_is_skipped_and_reported() {
    let dir = tmp("skip");
    let a = png(&dir, "a.png", 10, 10, "#ff0000");
    let gone = format!("{dir}/gone.png");
    let junk = format!("{dir}/junk.png");
    std::fs::write(&junk, b"not an image").unwrap();
    let b = png(&dir, "b.png", 12, 6, "#0000ff");
    let mut s = Session::new();
    let r = s.execute("file.openAsLayers", json!({"paths": [gone.clone(), a, junk.clone(), b]})).unwrap();
    assert_eq!(r["layers"], 2, "{r}");
    let skipped = r["skipped"].as_array().unwrap();
    assert_eq!(skipped.iter().map(|v| v["path"].as_str().unwrap()).collect::<Vec<_>>(), [gone.as_str(), junk.as_str()]);
    assert_eq!(skipped[0]["error"], "not found");
    assert!(!skipped[1]["error"].as_str().unwrap().is_empty());
    assert_eq!(names(&s), ["b", "a"]);
    // The skipped first file doesn't name the document or set its size.
    assert_eq!((s.active().unwrap().doc.size.width, s.active().unwrap().doc.size.height), (12, 10));
    assert_eq!(s.active().unwrap().doc.name, "gone");
}

#[test]
fn no_usable_file_is_an_error_and_makes_no_document() {
    let dir = tmp("none");
    let junk = format!("{dir}/junk.png");
    std::fs::write(&junk, b"not an image").unwrap();
    let mut s = Session::new();
    let e = s.execute("file.openAsLayers", json!({"paths": [format!("{dir}/gone.png"), junk]})).unwrap_err();
    let EngineError::Other(msg) = e else { panic!("{e:?}") };
    assert!(msg.contains("no file could be opened") && msg.contains("gone.png") && msg.contains("junk.png"), "{msg}");
    assert!(s.documents().is_empty());
}

#[test]
fn bad_params_are_errors() {
    let mut s = Session::new();
    let many: Vec<String> = (0..=256).map(|i| format!("/nonexistent/{i}.png")).collect();
    for p in [
        json!({}),
        json!({"paths": []}),
        json!({"paths": "a.png"}),
        json!({"paths": [1]}),
        json!({"paths": [""]}),
        json!({"paths": ["a.png"], "name": 3}),
        json!({"paths": many}),
    ] {
        assert!(matches!(s.execute("file.openAsLayers", p.clone()), Err(EngineError::BadParams { .. })), "{p}");
    }
    // 256 is the limit: past it nothing is read; at it the files are tried (and none exists).
    let at_limit: Vec<String> = many.into_iter().take(256).collect();
    let e = s.execute("file.openAsLayers", json!({"paths": at_limit})).unwrap_err();
    assert!(e.to_string().contains("no file could be opened"), "{e}");
    assert!(s.documents().is_empty());
}

#[test]
fn runs_as_a_background_job_and_leaves_the_files_alone() {
    let dir = tmp("job");
    let a = png(&dir, "a.png", 9, 9, "#ff0000");
    let b = png(&dir, "b.png", 5, 5, "#0000ff");
    let before = std::fs::read(&a).unwrap();
    let mut s = Session::new();
    let started = s.start("file.openAsLayers", json!({"paths": [a.clone(), b]})).unwrap();
    let crate::jobs::Started::Job(job) = started else { panic!("not a job: {started:?}") };
    let r: Value = s.wait_job(job).unwrap();
    assert_eq!(r["layers"], 2, "{r}");
    assert_eq!(s.documents().len(), 1);
    assert_eq!(std::fs::read(&a).unwrap(), before);
}
