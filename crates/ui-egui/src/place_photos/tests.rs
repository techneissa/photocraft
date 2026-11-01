use std::cell::RefCell;
use std::rc::Rc;

use egui::{Key, Modifiers};
use serde_json::json;

use super::*;
use crate::file_dialog::FileDialogReply;
use crate::menus::menu_items;
use crate::{Services, menus};

/// The requests the fake dialog service was given, with their replies: dialogs still open.
type Open = Rc<RefCell<Vec<(FileDialogRequest, FileDialogReply)>>>;

/// An app with a 40×30 document whose file dialogs stay open until the test answers them.
fn app() -> (PhotocraftApp, Open) {
    let open: Open = Rc::default();
    let held = open.clone();
    let services = Services { file_dialog: Some(Box::new(move |request, _parent, reply| held.borrow_mut().push((request, reply)))), ..Default::default() };
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
    app.run("file.new", json!({"width": 40, "height": 30})).unwrap();
    (app, open)
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut s = photocraft_engine::Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "background": "white"})).unwrap();
    photocraft_io::export(&s.active().unwrap().doc, "p.png", &Default::default()).unwrap().bytes
}

/// A fresh folder holding one PNG per name; the paths in order.
fn files(tag: &str, names: &[&str]) -> (std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("photocraft-place-photos-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths = names
        .iter()
        .map(|n| {
            let path = dir.join(n);
            std::fs::write(&path, png(8, 6)).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    (dir, paths)
}

/// Run Place Embedded from the menu, show the picker and answer it (`None`: Cancel).
fn place_via_picker(app: &mut PhotocraftApp, open: &Open, ctx: &egui::Context, reply: Option<FileDialogAnswer>) -> Request {
    menus::invoke(app, ctx, COMMAND, json!({})).unwrap();
    app.poll_file_dialog(ctx, None);
    let (request, answer) = open.borrow_mut().pop().expect("the picker is open");
    std::thread::spawn(move || answer.send(reply)).join().unwrap();
    app.poll_file_dialog(ctx, None);
    request
}

type Request = FileDialogRequest;

fn layer_names(app: &PhotocraftApp) -> Vec<String> {
    app.session.active().unwrap().doc.layers.iter().map(|l| l.name.clone()).collect()
}

fn active_name(app: &PhotocraftApp) -> String {
    let st = app.session.active().unwrap();
    st.doc.layer(st.active_layer.unwrap()).unwrap().name.clone()
}

fn press(app: &mut PhotocraftApp, key: Key, modifiers: Modifiers) {
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        events: vec![egui::Event::ModifiersChanged(modifiers), egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }],
        ..Default::default()
    };
    ctx.begin_pass(raw);
    crate::shortcuts::handle(app, &ctx);
    ctx.end_pass().textures_delta.clear();
}

// ---- 1. ⇧⌘P ----

#[test]
fn place_embedded_is_on_shift_command_p_in_the_engine_and_the_menus() {
    let spec = photocraft_engine::command_specs().iter().find(|c| c.id == COMMAND).unwrap();
    assert_eq!(spec.shortcut, Some("Cmd+Shift+P"));
    let catalog = crate::menu_catalog::CATALOG.iter().find(|row| row.3 == COMMAND).unwrap();
    assert_eq!(catalog.2, Some("Cmd+Shift+P"));
    let (app, _) = app();
    let item = menu_items(&app).into_iter().find(|i| i.id == COMMAND).unwrap();
    assert_eq!(item.shortcut.as_deref(), Some("Cmd+Shift+P"));
}

#[test]
fn nothing_else_uses_shift_command_p() {
    let wanted = photocraft_engine::prefs::normalize_shortcut("Cmd+Shift+P").unwrap();
    let users: Vec<&str> = photocraft_engine::prefs::bindable_items()
        .filter(|(.., key, _)| key.and_then(photocraft_engine::prefs::normalize_shortcut).as_deref() == Some(wanted.as_str()))
        .map(|(id, ..)| id)
        .collect();
    assert_eq!(users, [COMMAND], "a default key is bound to one command");
    let alternates: Vec<_> = photocraft_engine::prefs::ALTERNATE_SHORTCUTS
        .iter()
        .filter(|(_, key)| photocraft_engine::prefs::normalize_shortcut(key).as_deref() == Some(wanted.as_str()))
        .collect();
    assert!(alternates.is_empty(), "{alternates:?}");
    assert!(!photocraft_engine::prefs::reserved_shortcut("Cmd+Shift+P"), "the system does not take it");
    // Print One Copy keeps ⌥⇧⌘P, Print ⌘P.
    let key = |id: &str| photocraft_engine::command_specs().iter().find(|c| c.id == id).and_then(|c| c.shortcut);
    assert_eq!(key("file.printOneCopy"), Some("Cmd+Alt+Shift+P"));
}

#[test]
fn the_key_opens_the_picker_for_several_photos() {
    let (mut app, open) = app();
    press(&mut app, Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
    app.poll_file_dialog(&egui::Context::default(), None);
    assert!(matches!(open.borrow().as_slice(), [(FileDialogRequest::Open { multiple: true, .. }, _)]));
    // The same keys with Alt are Print One Copy, not Place.
    let (mut app, open) = self::app();
    press(&mut app, Key::P, Modifiers::COMMAND | Modifiers::SHIFT | Modifiers::ALT);
    app.poll_file_dialog(&egui::Context::default(), None);
    assert!(open.borrow().is_empty());
}

// ---- 3. several photos ----

#[test]
fn the_picker_allows_several_files() {
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    let request = place_via_picker(&mut app, &open, &ctx, None);
    assert!(matches!(request, FileDialogRequest::Open { multiple: true, extensions: None, .. }), "{request:?}");
}

#[test]
fn one_photo_is_placed_as_before() {
    for reply in [FileDialogAnswer::Contents("lone.png".into(), png(8, 6)), FileDialogAnswer::ContentsMany(vec![("lone.png".into(), png(8, 6))])] {
        let (mut app, open) = app();
        let ctx = egui::Context::default();
        let before = layer_names(&app).len();
        place_via_picker(&mut app, &open, &ctx, Some(reply));
        assert_eq!(layer_names(&app).len(), before + 1);
        assert_eq!(active_name(&app), "lone");
        assert_eq!(app.ui.transform.as_ref().and_then(|t| t.made), Some(crate::state::MadeLayer::Place));
        assert!(app.drop_places.is_empty());
        assert!(!app.ui.status_error, "{}", app.ui.status);
    }
    // A desktop answer with one path.
    let (dir, paths) = files("one", &["solo.png"]);
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    place_via_picker(&mut app, &open, &ctx, Some(FileDialogAnswer::Paths(paths)));
    assert_eq!(active_name(&app), "solo");
    assert!(app.ui.transform.is_some() && app.drop_places.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn several_photos_become_layers_one_after_another_in_free_transform() {
    let (dir, paths) = files("many", &["a.png", "b.png", "c.png"]);
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    let (layers, steps) = (layer_names(&app).len(), app.session.active().unwrap().history.past_len());
    place_via_picker(&mut app, &open, &ctx, Some(FileDialogAnswer::Paths(paths)));
    // The first is in Free Transform at once; the other two wait.
    assert_eq!(active_name(&app), "a");
    assert_eq!(layer_names(&app).len(), layers + 1);
    assert_eq!(app.drop_places.len(), 2);
    assert_eq!(app.ui.transform.as_ref().and_then(|t| t.made), Some(crate::state::MadeLayer::Place));
    // Nothing moves while the box is open.
    app.place_next_dropped(&ctx);
    assert_eq!(layer_names(&app).len(), layers + 1);
    // ↩ applies it, and the next frame places the second.
    crate::transform_tool::commit(&mut app);
    app.place_next_dropped(&ctx);
    assert_eq!(active_name(&app), "b");
    // Esc takes the second back, the third follows.
    crate::transform_tool::cancel(&mut app);
    assert_eq!(layer_names(&app).len(), layers + 1);
    app.place_next_dropped(&ctx);
    assert_eq!(active_name(&app), "c");
    crate::transform_tool::commit(&mut app);
    app.place_next_dropped(&ctx);
    assert!(app.ui.transform.is_none() && app.drop_places.is_empty());
    // a and c stayed: the layers are on top of each other in the order chosen, one step each.
    assert_eq!(layer_names(&app)[layers..], ["a", "c"]);
    assert_eq!(app.session.active().unwrap().history.past_len(), steps + 2);
    assert!(!app.ui.notices.iter().any(|n| n.error), "{:?}", app.ui.notices);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_photo_that_cannot_be_placed_is_reported_and_the_rest_are_placed() {
    let (dir, mut paths) = files("bad", &["one.png", "three.png"]);
    let broken = dir.join("two.png");
    std::fs::write(&broken, b"not an image").unwrap();
    paths.insert(1, broken.to_string_lossy().into_owned());
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    let layers = layer_names(&app).len();
    place_via_picker(&mut app, &open, &ctx, Some(FileDialogAnswer::Paths(paths)));
    crate::transform_tool::commit(&mut app);
    app.place_next_dropped(&ctx);
    assert_eq!(active_name(&app), "three", "the broken one is skipped");
    assert!(app.ui.notices.iter().any(|n| n.error && n.title.contains("two.png")), "{:?}", app.ui.notices);
    assert_eq!(layer_names(&app).len(), layers + 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_browser_answer_with_contents_queues_the_bytes() {
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    let files = vec![("x.png".to_string(), png(4, 4)), ("y.png".to_string(), png(4, 4))];
    place_via_picker(&mut app, &open, &ctx, Some(FileDialogAnswer::ContentsMany(files)));
    assert_eq!(active_name(&app), "x");
    assert_eq!(app.drop_places.len(), 1);
    crate::transform_tool::commit(&mut app);
    app.place_next_dropped(&ctx);
    assert_eq!(active_name(&app), "y");
}

#[test]
fn cancel_places_nothing() {
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    let before = app.session.active().unwrap().doc.clone();
    place_via_picker(&mut app, &open, &ctx, None);
    assert_eq!(app.session.active().unwrap().doc, before);
    assert!(app.ui.transform.is_none() && app.drop_places.is_empty());
    assert!(!app.ui.status_error, "{}", app.ui.status);
}

#[test]
fn an_open_transform_blocks_the_menu_and_the_queue_stays_empty() {
    let (dir, paths) = files("blocked", &["p.png", "q.png"]);
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    app.run("edit.fill", json!({"color": "#ff0000"})).unwrap();
    app.run("layer.new.layerFromBackground", json!({})).unwrap();
    crate::transform_tool::begin(&mut app, &ctx).unwrap();
    assert!(menus::invoke(&mut app, &ctx, COMMAND, json!({})).is_err());
    assert!(!app.file_dialog_open());
    // The transform opened while the picker was up: its answer is refused.
    crate::transform_tool::cancel(&mut app);
    assert!(app.ui.transform.is_none());
    menus::invoke(&mut app, &ctx, COMMAND, json!({})).unwrap();
    app.poll_file_dialog(&ctx, None);
    crate::transform_tool::begin(&mut app, &ctx).unwrap();
    let (_, answer) = open.borrow_mut().pop().unwrap();
    std::thread::spawn(move || answer.send(Some(FileDialogAnswer::Paths(paths)))).join().unwrap();
    app.poll_file_dialog(&ctx, None);
    assert!(app.drop_places.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn place_linked_still_takes_one_file() {
    let (mut app, open) = app();
    let ctx = egui::Context::default();
    menus::invoke(&mut app, &ctx, "file.placeLinked", json!({})).unwrap();
    app.poll_file_dialog(&ctx, None);
    assert!(matches!(open.borrow().as_slice(), [(FileDialogRequest::Open { multiple: false, .. }, _)]));
}

// ---- 2. the Layers panel button ----

/// The Layers panel over a document, with `file_dialog` answering through `open`.
fn panel(with_picker: bool) -> (egui_kittest::Harness<'static, PhotocraftApp>, Open) {
    let open: Open = Rc::default();
    let held = open.clone();
    let mut s = photocraft_engine::Session::new();
    s.execute("file.new", json!({"width": 80, "height": 64})).unwrap();
    s.execute("layer.new.layer", json!({"name": "L1"})).unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let file_dialog: Option<crate::file_dialog::FileDialogFn> =
            with_picker.then(|| Box::new(move |request, _: Option<&eframe::Frame>, reply| held.borrow_mut().push((request, reply))) as _);
        PhotocraftApp::new(s, Services { file_dialog, ..Default::default() })
    });
    h.run_steps(4);
    (h, open)
}

#[test]
fn the_footer_button_runs_place_embedded_and_names_the_key() {
    use egui_kittest::kittest::Queryable;
    let (mut h, open) = panel(true);
    let tip = crate::shortcuts::tip_label(h.state(), "Place Photo", COMMAND);
    assert!(tip.starts_with("Place Photo") && tip.ends_with(&format!("({})", crate::shortcuts::pretty("Cmd+Shift+P"))), "{tip}");
    h.get_by_label(&tip).click();
    h.run_steps(4);
    assert!(matches!(open.borrow().as_slice(), [(FileDialogRequest::Open { multiple: true, .. }, _)]), "the click opens the multi-select picker");
}

#[test]
fn the_footer_button_is_greyed_without_a_file_picker_and_sits_next_to_double_exposure() {
    use egui_kittest::kittest::Queryable;
    let (mut h, open) = panel(false);
    h.get_by_label_contains("Place Photo").click();
    h.run_steps(4);
    assert!(!h.state().file_dialog_open() && open.borrow().is_empty());
    let (h, _) = panel(true);
    let place = h.get_by_label_contains("Place Photo").rect();
    let double = h.get_by_label("Double Exposure").rect();
    assert!((place.center().y - double.center().y).abs() < 0.5, "same row");
    assert!((double.min.x - place.max.x).abs() < 8.0, "left beside it (the footer runs right to left): {place:?} {double:?}");
}
