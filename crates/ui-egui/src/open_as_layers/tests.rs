use super::*;
use crate::{PhotocraftApp, Services};
use photocraft_engine::Session;

fn manifest(v: Value) -> Result<Manifest, String> {
    parse_manifest(v.to_string().as_bytes())
}

fn good() -> Value {
    json!({"kind": KIND, "version": 1, "name": "DSC01234 + 2", "files": ["/a/top.psd", "/a/bottom.jpg"]})
}

#[test]
fn reads_a_manifest() {
    let m = manifest(good()).unwrap();
    assert_eq!(m, Manifest { name: Some("DSC01234 + 2".into()), files: vec!["/a/top.psd".into(), "/a/bottom.jpg".into()] });
    // The name is optional (and blank is none); a newline and a BOM are fine; extra keys are ignored.
    let mut v = good();
    v["name"] = json!("  ");
    v["extra"] = json!(true);
    assert_eq!(manifest(v).unwrap().name, None);
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(good().to_string().bytes());
    bytes.push(b'\n');
    assert!(parse_manifest(&bytes).is_ok());
}

#[test]
fn refuses_a_bad_manifest() {
    let with = |key: &str, value: Value| {
        let mut v = good();
        v[key] = value;
        manifest(v)
    };
    assert!(with("kind", json!("something-else")).is_err());
    assert!(with("kind", json!(7)).is_err());
    assert!(with("version", json!(2)).unwrap_err().contains("newer"));
    assert!(with("version", json!(0)).is_err());
    assert!(with("version", json!("1")).is_err());
    assert!(with("files", json!([])).unwrap_err().contains("no files"));
    assert!(with("files", json!("a.psd")).is_err());
    assert!(with("files", json!(["a.psd", 3])).is_err());
    assert!(with("files", json!([""])).is_err());
    assert!(with("files", json!((0..257).map(|i| format!("/a/{i}.png")).collect::<Vec<_>>())).unwrap_err().contains("256"));
    assert!(with("files", json!((0..256).map(|i| format!("/a/{i}.png")).collect::<Vec<_>>())).is_ok());
    for bytes in [&b""[..], b"{", b"[]", b"null", b"\xff\xfe", b"{\"kind\": \"photocraft-open-as-layers\"}"] {
        assert!(parse_manifest(bytes).is_err(), "{bytes:?}");
    }
}

#[test]
fn only_pcstack_files_are_manifests() {
    assert!(is_manifest("/x/DSC01234 + 2.pcstack") && is_manifest("/x/A.PCSTACK"));
    assert!(!is_manifest("/x/a.pcraft") && !is_manifest("/x/pcstack") && !is_manifest("/x/a.psd"));
}

fn tmp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("photocraft-ui-open-as-layers-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.to_string_lossy().into_owned()
}

fn png(dir: &str, name: &str, w: u32, h: u32, color: &str) -> String {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "background": color})).unwrap();
    let path = format!("{dir}/{name}");
    s.execute("file.saveACopy", json!({"path": path})).unwrap();
    path
}

fn write(dir: &str, name: &str, contents: &str) -> String {
    let path = format!("{dir}/{name}");
    std::fs::write(&path, contents).unwrap();
    path
}

fn layer_names(app: &PhotocraftApp) -> Vec<String> {
    app.session.active().unwrap().doc.layers.iter().map(|l| l.name.clone()).collect()
}

#[test]
fn opening_a_manifest_makes_one_document_and_names_the_files_left_out() {
    let dir = tmp("open");
    let (a, b) = (png(&dir, "top.png", 30, 20, "#ff0000"), png(&dir, "bottom.png", 20, 40, "#0000ff"));
    let gone = format!("{dir}/gone.png");
    let stack = write(&dir, "set.pcstack", &json!({"kind": KIND, "version": 1, "name": "Two", "files": [a, gone, b]}).to_string());
    let before = std::fs::read(&stack).unwrap();
    let mut app = PhotocraftApp::new(Session::new(), Services::default());
    assert_eq!(app.open_paths(std::slice::from_ref(&stack)), 1);
    assert_eq!(app.session.documents().len(), 1);
    let doc = &app.session.active().unwrap().doc;
    assert_eq!((doc.name.as_str(), doc.size.width, doc.size.height), ("Two", 30, 40));
    assert_eq!(layer_names(&app), ["bottom", "top"]);
    assert!(app.session.active().unwrap().path.is_none());
    // The missing file is named in a notice; the manifest is untouched and not in Open Recent.
    let notice = app.ui.notices.last().expect("a notice for the skipped file");
    assert_eq!(notice.title, "1 file skipped");
    assert_eq!(notice.lines, ["gone.png: not found"]);
    assert_eq!(std::fs::read(&stack).unwrap(), before);
}

#[test]
fn a_bad_manifest_or_no_usable_file_is_an_error_notice_and_no_document() {
    let dir = tmp("bad");
    let wrong = write(&dir, "wrong.pcstack", &json!({"kind": "other", "version": 1, "files": ["/a.png"]}).to_string());
    let newer = write(&dir, "newer.pcstack", &json!({"kind": KIND, "version": 2, "files": ["/a.png"]}).to_string());
    let broken = write(&dir, "broken.pcstack", "{not json");
    let empty = write(&dir, "empty.pcstack", &json!({"kind": KIND, "version": 1, "files": []}).to_string());
    let none = write(&dir, "none.pcstack", &json!({"kind": KIND, "version": 1, "files": [format!("{dir}/gone.png")]}).to_string());
    let mut app = PhotocraftApp::new(Session::new(), Services::default());
    let mut titles = Vec::new();
    for path in [wrong, newer, broken, empty, none] {
        app.ui.notices.clear();
        assert_eq!(app.open_paths(std::slice::from_ref(&path)), 0, "{path}");
        let notice = app.ui.notices.last().unwrap_or_else(|| panic!("no notice for {path}"));
        assert!(notice.error && notice.title.starts_with("Couldn't open"), "{}", notice.title);
        titles.push(notice.title.clone());
    }
    assert!(titles[1].contains("newer PhotoCraft"), "{titles:?}");
    assert!(titles[4].contains("no file could be opened"), "{titles:?}");
    assert!(app.session.documents().is_empty());
    // A manifest that isn't there at all.
    assert_eq!(app.open_paths(&[format!("{dir}/missing.pcstack")]), 0);
}

#[test]
fn in_the_background_the_window_keeps_going_and_the_document_arrives() {
    let dir = tmp("bg");
    let (a, b) = (png(&dir, "a.png", 12, 12, "#ff0000"), png(&dir, "b.png", 8, 16, "#00ff00"));
    let stack = write(&dir, "bg.pcstack", &json!({"kind": KIND, "version": 1, "files": [a, format!("{dir}/gone.png"), b]}).to_string());
    let mut app = PhotocraftApp::new(Session::new(), Services::default());
    app.background_jobs = true;
    app.open_path(&stack).unwrap();
    assert!(app.session.has_jobs() || !app.session.documents().is_empty());
    let ctx = egui::Context::default();
    for _ in 0..500 {
        crate::jobs_ui::tick(&mut app, &ctx);
        if !app.session.documents().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(layer_names(&app), ["b", "a"]);
    assert_eq!(app.session.active().unwrap().doc.name, "a");
    assert_eq!(app.ui.notices.last().map(|n| n.lines.clone()), Some(vec!["gone.png: not found".to_string()]));
    assert_eq!(app.ui.status, "Opened a: 2 layers");
}
