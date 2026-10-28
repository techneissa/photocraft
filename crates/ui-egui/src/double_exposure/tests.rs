use serde_json::json;

use super::*;
use crate::control::{ControlRequest, handle};
use crate::menus::{invoke, menu_items, search_items};

const CMD: &str = COMMAND;

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

/// The command as the menu, the palette and the Layers panel button run it.
fn from_menu(app: &mut PhotocraftApp) -> Result<serde_json::Value, String> {
    invoke(app, &egui::Context::default(), CMD, json!({}))
}

#[test]
fn the_menu_sets_the_brush_soft_and_the_mask_gets_black_over_white() {
    let mut app = app(200, 160);
    from_menu(&mut app).unwrap();

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
fn a_script_keeps_the_tool_the_brush_and_the_status() {
    let mut app = app(200, 160);
    let brush = app.session.tools.brush.clone();
    let ctx = egui::Context::default();
    let (req, _rx) = ControlRequest::new("engine.execute", json!({"command": CMD, "params": {}}));
    handle(&mut app, &ctx, &req);
    assert_eq!(app.session.active().unwrap().doc.layers.iter().filter(|l| l.mask.is_some()).count(), 1, "the command ran");
    assert_eq!(app.ui.tool, Tool::Move, "the person's tool stays");
    assert!(app.session.tools.brush == brush, "so does the brush");
    assert!(app.ui.status.is_empty(), "{}", app.ui.status);
    assert!(app.prefs_rt.double_exposure_brush.is_none());

    // Straight through the app's command runner (CLI, MCP, action replay) is the same.
    let mut app = self::app(200, 160);
    app.run(CMD, json!({})).unwrap();
    assert_eq!(app.ui.tool, Tool::Move);
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
    assert!(from_menu(&mut app).is_err());
    assert_eq!(app.ui.tool, Tool::Move);
    assert_eq!(app.session.tools.foreground, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn undo_leaves_the_tool_but_drops_the_mask_target_and_redo_brings_it_back() {
    let mut app = app(64, 64);
    from_menu(&mut app).unwrap();
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.ui.tool, Tool::Brush, "the tool is not part of the step");
    assert!(!app.layer_mask_targeted(), "the mask is gone");
    app.run("edit.redo", json!({})).unwrap();
    assert!(app.layer_mask_targeted(), "the redone step targets its mask again");
}

#[test]
fn the_saved_brush_is_not_replaced_and_comes_back_after_another_tool() {
    let ctx = egui::Context::default();
    let mut app = app(200, 160);
    app.session.execute("tools.setBrush", json!({"brush": {"size": 33, "hardness": 0.8}})).unwrap();
    crate::last_brush::persist(&mut app, &ctx);
    let saved = app.session.prefs().last_brush.clone();
    assert!(!saved.is_null(), "the person's brush is remembered");

    from_menu(&mut app).unwrap();
    assert_eq!(app.session.tools.brush.hardness, 0.0);
    crate::last_brush::persist(&mut app, &ctx);
    crate::tool_memory::persist(&mut app, &ctx);
    assert_eq!(app.session.prefs().last_brush, saved, "the soft brush is not what is remembered");

    // Running it again keeps the same brush to give back.
    from_menu(&mut app).unwrap();

    // Another painting tool takes over: the Brush's own brush goes back to it.
    crate::tool_memory::select(&mut app, Tool::Eraser);
    crate::paint_mouse::sync_tool_brush(&mut app);
    let own = app.ui.tool_brushes.iter().find(|(t, _)| *t == Tool::Brush).map(|(_, b)| (b.size, b.hardness));
    assert_eq!(own, Some((33.0, 0.8)));
    crate::last_brush::persist(&mut app, &ctx);
    crate::tool_memory::persist(&mut app, &ctx);
    let tool_brushes = app.session.prefs().tool_brushes.to_string();
    assert!(tool_brushes.contains("0.8"), "{tool_brushes}");

    crate::tool_memory::select(&mut app, Tool::Brush);
    crate::paint_mouse::sync_tool_brush(&mut app);
    assert_eq!((app.session.tools.brush.size, app.session.tools.brush.hardness), (33.0, 0.8));
    assert!(app.prefs_rt.double_exposure_brush.is_none());
}

#[test]
fn a_soft_brush_the_person_edited_stays_theirs() {
    let mut app = app(200, 160);
    from_menu(&mut app).unwrap();
    app.session.execute("tools.setBrush", json!({"brush": {"size": 50}})).unwrap();
    crate::tool_memory::select(&mut app, Tool::Eraser);
    crate::paint_mouse::sync_tool_brush(&mut app);
    let own = app.ui.tool_brushes.iter().find(|(t, _)| *t == Tool::Brush).map(|(_, b)| (b.size, b.hardness));
    assert_eq!(own, Some((50.0, 0.0)), "their edit is not undone");
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
