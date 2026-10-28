use serde_json::json;

use super::*;
use crate::menus::{menu_items, search_items};

const CMD: &str = "layer.doubleExposure";

/// A `w`×`h` document with two pixel layers, the Move tool selected and red/blue as colours.
fn app(w: u64, h: u64) -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.session.execute("file.new", json!({"width": w, "height": h, "background": "transparent"})).unwrap();
    app.session.execute("layer.new.layer", json!({"name": "Scene"})).unwrap();
    app.sync_views();
    crate::tool_memory::select(&mut app, Tool::Move);
    app.session.tools.foreground = [1.0, 0.0, 0.0, 1.0];
    app.session.tools.background = [0.0, 0.0, 1.0, 1.0];
    app
}

#[test]
fn the_brush_is_soft_and_the_mask_gets_black_over_white() {
    let mut app = app(200, 160);
    app.run(CMD, json!({})).unwrap();

    assert_eq!(app.ui.tool, Tool::Brush);
    let t = &app.session.tools;
    assert_eq!((t.foreground, t.background), ([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]));
    assert_eq!(t.brush.hardness, 0.0);
    assert_eq!(t.brush.size, 20.0, "an eighth of the shorter side (160)");
    assert!(!t.brush.erase);
    assert!(t.mask_targeted, "the mask is the edit target");
    assert_eq!(t.other_colors, [[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]], "the pixels' colours are kept for ⌘2");
    assert!(app.layer_mask_targeted());
    assert!(app.ui.mask_target);
    assert!(!app.ui.status_error);
    assert!(app.ui.status.contains("Paint black to hide"), "{}", app.ui.status);
}

#[test]
fn the_brush_size_stays_in_the_brush_range() {
    assert_eq!(brush_size(1600, 1200), 150.0);
    assert_eq!(brush_size(3, 3), 1.0, "never below 1 px");
    assert_eq!(brush_size(0, 500), 1.0);
    assert_eq!(brush_size(100_000, 100_000), 5000.0, "never above the largest brush");
}

#[test]
fn a_refused_command_leaves_the_tool_and_colours_alone() {
    let mut app = app(64, 64);
    app.session.execute("layer.delete", json!({})).unwrap();
    assert!(app.run(CMD, json!({})).is_err());
    assert_eq!(app.ui.tool, Tool::Move);
    assert_eq!(app.session.tools.foreground, [1.0, 0.0, 0.0, 1.0]);
    assert!(app.ui.status_error);
}

#[test]
fn undo_leaves_the_tool_but_drops_the_mask_target() {
    let mut app = app(64, 64);
    app.run(CMD, json!({})).unwrap();
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.ui.tool, Tool::Brush, "the tool is not part of the step");
    assert!(!app.layer_mask_targeted(), "the mask is gone");
}

#[test]
fn the_menu_item_sits_in_the_layer_menu_and_the_search_finds_it() {
    let mut app = app(64, 64);
    let items = menu_items(&app);
    let at = items.iter().position(|i| i.id == CMD).expect("a Layer menu item");
    let item = &items[at];
    assert_eq!((item.label.as_str(), item.path.as_slice()), ("Double Exposure", ["Layer".to_string()].as_slice()));
    assert!(item.enabled);
    assert_eq!(items[at - 1].id, "layer.maskAllObjects", "right after the Layer Mask items");

    let hits = search_items(&items, "double exp", crate::i18n::Lang::EN);
    assert!(hits.iter().any(|i| i.id == CMD), "the Help search lists it");

    app.session.execute("layer.delete", json!({})).unwrap();
    let off = menu_items(&app).into_iter().find(|i| i.id == CMD).unwrap();
    assert!(!off.enabled, "greyed with one pixel layer");
}
