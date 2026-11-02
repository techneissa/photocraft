//! The list Keyboard Shortcuts shows is built once and kept between frames, and built again when
//! what it reads changes.

use std::rc::Rc;

use serde_json::json;

use super::shortcuts_items::cached;
use crate::PhotocraftApp;

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
}

#[test]
fn the_list_is_built_once_while_nothing_it_reads_changes() {
    let app = app();
    let first = cached(&app);
    assert!(!first.is_empty());
    for _ in 0..50 {
        assert!(Rc::ptr_eq(&first, &cached(&app)), "the same list every frame");
    }
    assert_eq!(*first, super::shortcuts::shortcut_items(&app), "the kept list is the built one");
}

#[test]
fn the_window_keeps_the_list_across_frames() {
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        app()
    });
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.keyboardShortcuts", json!({})).expect("the window");
    h.run_steps(4);
    let first = cached(h.state());
    h.run_steps(20);
    assert!(Rc::ptr_eq(&first, &cached(h.state())), "twenty frames with the window open didn't rebuild the list");
}

#[test]
fn a_shortcut_set_in_the_window_rebuilds_the_list() {
    let mut app = app();
    let before = cached(&app);
    // No default of its own: the catalogue's shortcut shows only while the user has set none.
    let id = before.iter().find(|i| crate::shortcuts::default_shortcut(&i.0).is_none() && i.3.is_some()).map(|i| i.0.clone());
    app.session.edit_prefs(|p| _ = p.shortcuts.insert("file.new".into(), "Cmd+J".into()));
    let after = cached(&app);
    assert!(!Rc::ptr_eq(&before, &after), "a changed shortcut rebuilds the list");
    assert_eq!(*after, super::shortcuts::shortcut_items(&app));
    if let Some(id) = id {
        app.session.edit_prefs(|p| _ = p.shortcuts.insert(id.clone(), "Cmd+Alt+J".into()));
        let item = cached(&app).iter().find(|i| i.0 == id).cloned().expect("the item");
        assert_eq!(item.3, None, "the catalogue's shortcut is gone once the user sets one");
    }
}

#[test]
fn a_hidden_menu_item_rebuilds_the_list() {
    let mut app = app();
    let before = cached(&app);
    // An item Edit › Menus can hide, that has no shortcut of its own (one with a shortcut stays in the
    // list through its command).
    let id = before
        .iter()
        .find(|i| i.3.is_none() && photocraft_engine::prefs::MenuCustomization::hideable(&i.0) && photocraft_engine::commands::find(&i.0).is_some())
        .map(|i| i.0.clone())
        .expect("a hideable item without a shortcut");
    app.session.edit_prefs(|p| p.menus.hidden.push(id.clone()));
    let after = cached(&app);
    assert!(!Rc::ptr_eq(&before, &after));
    assert_eq!(*after, super::shortcuts::shortcut_items(&app));
    assert!(after.iter().all(|i| i.0 != id), "Edit › Menus hid {id}, so the list doesn't show it");
}

#[test]
fn the_selected_layers_mask_changes_the_words_of_its_menu_item() {
    let mut app = app();
    app.run("file.new", json!({"width": 32, "height": 32})).expect("document");
    app.run("layer.new.layer", json!({})).expect("layer");
    let label = |app: &PhotocraftApp| cached(app).iter().find(|i| i.0 == "layer.layerMask.enabled").map(|i| i.1.clone());
    let before = cached(&app);
    app.run("layer.layerMask.revealAll", json!({})).expect("mask");
    assert!(!Rc::ptr_eq(&before, &cached(&app)), "a layer mask rebuilds the list");
    assert_eq!(label(&app).as_deref(), Some(crate::layer_menu_ui::mask_toggle_label(true)), "an enabled mask offers Disable");
    assert_eq!(*cached(&app), super::shortcuts::shortcut_items(&app));
}
