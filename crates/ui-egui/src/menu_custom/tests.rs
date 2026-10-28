use serde_json::{Value, json};

use super::menus::{self, CANVAS_NONE, CANVAS_SELECTION, LAYERS_ROW, MENUS, TAB};
use super::*;
use crate::canvas_tool_menu::{self, Row};
use crate::state::Tool;

fn app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 32, "height": 32})).unwrap();
    app
}

fn arrangement(items: &[&str], hidden: &[&str]) -> Arrangement {
    Arrangement { items: items.iter().map(|s| s.to_string()).collect(), hidden: hidden.iter().map(|s| s.to_string()).collect() }
}

/// An arrangement showing just `items` (slots), every other row of menu `key` taken out.
fn only(key: &str, items: &[&str]) -> Arrangement {
    let hidden: Vec<&str> = menus::universe(key).into_iter().filter(|id| !items.contains(&format!("{CMD}{id}").as_str())).collect();
    arrangement(items, &hidden)
}

fn ids(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|r| r.map_or("---", |(_, id)| id)).collect()
}

fn save(app: &mut PhotocraftApp, key: &str, a: Arrangement) {
    let mut custom = load(app);
    custom.menus.insert(key.to_string(), a);
    store(app, &custom);
}

fn selection_menu(app: &mut PhotocraftApp) -> canvas_tool_menu::CanvasToolMenu {
    app.ui.tool = Tool::RectMarquee;
    app.run("select.rect", json!({"x": 1, "y": 1, "width": 8, "height": 8})).unwrap();
    assert!(canvas_tool_menu::open(app, &egui::Context::default(), Tool::RectMarquee, [10.0, 10.0]));
    app.ui.canvas_tool_menu.clone().unwrap()
}

#[test]
fn hide_reorder_add_and_separators_apply_over_the_defaults() {
    let default = menus::rows(CANVAS_SELECTION);
    let universe = menus::universe(CANVAS_SELECTION);
    let first = universe[0];
    // The default order, less Deselect, with Select Inverse first, Copy added, a separator at the end of the first group.
    let mut items: Vec<String> = default_items(&default).into_iter().filter(|i| i != "cmd:select.deselect").collect();
    items.insert(0, "cmd:select.inverse".into());
    items.dedup();
    items.retain({
        let mut seen = std::collections::HashSet::new();
        move |i| i == "---" || seen.insert(i.clone())
    });
    items.insert(1, "cmd:edit.copy".into());
    items.insert(2, "---".into());
    let a = Arrangement { hidden: vec!["select.deselect".into()], items };
    let rows = apply(&a, &default, &universe, true);
    let got = ids(&rows);
    assert_eq!(got[0], "select.inverse");
    assert_eq!(got[1], "edit.copy", "a command of the menu bar is added");
    assert_eq!(got[2], "---");
    assert!(!got.contains(&"select.deselect"), "hidden");
    assert_eq!(first, "select.deselect");
    assert!(got.contains(&"layer.new.layer"), "the rest stays");
    // the added command carries the menu bar's label
    assert_eq!(rows[1].map(|r| r.0), catalog::find("edit.copy").map(|c| c.label));
}

fn default_items(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| r.map_or_else(|| SEPARATOR.to_string(), |(_, id)| format!("{CMD}{id}"))).collect()
}

#[test]
fn no_arrangement_changes_nothing_and_a_default_one_is_the_default() {
    for (key, _) in MENUS {
        let default = menus::rows(key);
        assert_eq!(arranged(&Value::Null, key, default.clone()), default, "{key}");
        let a = Arrangement { items: default_items(&default), hidden: vec![] };
        assert_eq!(apply(&a, &default, &menus::universe(key), true), default, "{key}");
    }
}

#[test]
fn an_unknown_or_removed_command_is_skipped() {
    let default = menus::rows(CANVAS_SELECTION);
    let a = only(CANVAS_SELECTION, &["cmd:select.deselect", "cmd:no.such.command", "cmd:", "junk", "cmd:edit.copy"]);
    let rows = apply(&a, &default, &menus::universe(CANVAS_SELECTION), true);
    assert_eq!(ids(&rows), ["select.deselect", "edit.copy"]);
}

#[test]
fn a_command_of_the_menu_bar_is_not_added_to_the_tab_menu() {
    let items: Vec<TabItem> = vec![("Close", "file.close", Value::Null, true), ("Close All", "file.closeAll", Value::Null, true)];
    let saved = Custom { menus: [(TAB.to_string(), arrangement(&["cmd:edit.copy", "cmd:file.closeAll", "---", "cmd:file.close"], &[]))].into(), ..Default::default() }.to_value();
    assert_eq!(tab_rows(&saved, &items), vec![Some(1), None, Some(0)]);
}

#[test]
fn a_row_an_update_adds_shows_after_the_row_before_it() {
    let default = menus::rows(CANVAS_NONE);
    // an arrangement made before the last two rows existed: they are neither listed nor hidden
    let ids_all = menus::universe(CANVAS_NONE);
    let old: Vec<String> = ids_all[..ids_all.len() - 1].iter().map(|id| format!("{CMD}{id}")).collect();
    let a = Arrangement { items: old, hidden: vec![] };
    let rows = apply(&a, &default, &ids_all, true);
    assert_eq!(rows.iter().flatten().count(), ids_all.len(), "the new row shows");
    // a row hidden on purpose stays out
    let hidden = Arrangement { items: vec![], hidden: vec![ids_all[0].to_string()] };
    let rows = apply(&hidden, &default, &ids_all, true);
    assert!(!ids(&rows).contains(&ids_all[0]));
    assert_eq!(rows.iter().flatten().count(), ids_all.len() - 1);
}

#[test]
fn separators_never_start_end_or_double() {
    let default = menus::rows(CANVAS_SELECTION);
    let a = only(CANVAS_SELECTION, &["---", "---", "cmd:select.inverse", "---", "---", "cmd:edit.copy", "---"]);
    let rows = apply(&a, &default, &menus::universe(CANVAS_SELECTION), true);
    assert_eq!(ids(&rows), ["select.inverse", "---", "edit.copy"]);
    // separators of the default menu go with the rows around them
    let a = only(CANVAS_SELECTION, &["cmd:select.deselect", "cmd:edit.fill"]);
    assert_eq!(ids(&apply(&a, &default, &menus::universe(CANVAS_SELECTION), true)), ["select.deselect", "edit.fill"]);
}

#[test]
fn hiding_everything_gives_the_default_menu_back() {
    let mut app = app();
    let all: Vec<&str> = menus::universe(CANVAS_NONE);
    save(&mut app, CANVAS_NONE, arrangement(&[], &all));
    let default = menus::rows(CANVAS_NONE);
    assert_eq!(arranged(&app.session.prefs().context_menus, CANVAS_NONE, default.clone()), default);
}

#[test]
fn the_canvas_menu_shows_the_arrangement_and_a_greyed_command_stays_greyed() {
    let mut app = app();
    // Redo has nothing to redo: it is added, shown and greyed
    save(&mut app, CANVAS_SELECTION, only(CANVAS_SELECTION, &["cmd:edit.redo", "cmd:select.inverse", "cmd:select.deselect"]));
    let menu = selection_menu(&mut app);
    let rows = canvas_rows(&app, &menu);
    let got = ids(&rows);
    assert_eq!(got, ["edit.redo", "select.inverse", "select.deselect"]);
    assert!(canvas_has(&app, &menu, "edit.redo"));
    assert!(!canvas_tool_menu::entry_enabled(&app, &menu, "edit.redo"), "greyed");
    assert!(!canvas_tool_menu::available(&app, &menu, "edit.redo"), "a greyed row never runs");
    assert!(canvas_tool_menu::available(&app, &menu, "select.inverse"));
    // a hidden row cannot be chosen either
    save(&mut app, CANVAS_SELECTION, only(CANVAS_SELECTION, &["cmd:select.inverse"]));
    assert!(!canvas_has(&app, &menu, "select.deselect"));
    assert!(!canvas_tool_menu::available(&app, &menu, "select.deselect"));
    // choosing a row runs its command
    canvas_tool_menu::choose(&mut app, &egui::Context::default(), "select.inverse");
    assert!(app.ui.canvas_tool_menu.is_none());
}

#[test]
fn the_other_canvas_menus_are_not_arranged() {
    let mut app = app();
    save(&mut app, CANVAS_SELECTION, arrangement(&["cmd:select.inverse"], &[]));
    app.ui.tool = Tool::Pen;
    assert!(canvas_tool_menu::open(&mut app, &egui::Context::default(), Tool::Pen, [10.0, 10.0]));
    let menu = app.ui.canvas_tool_menu.clone().unwrap();
    assert_eq!(canvas_key(&menu), None);
    assert_eq!(canvas_rows(&app, &menu), canvas_tool_menu::PEN_MENU);
}

fn layer_of(app: &PhotocraftApp, id: photocraft_doc::LayerId) -> photocraft_doc::Layer {
    app.session.active().unwrap().doc.layer(id).unwrap().clone()
}

/// Layers of every kind the row menu trims for, with and without a mask, clip and style.
fn layers(app: &mut PhotocraftApp) -> Vec<photocraft_doc::Layer> {
    let mut out = Vec::new();
    let mut push = |app: &mut PhotocraftApp| {
        let st = app.session.active().unwrap();
        out.push(layer_of(app, st.active_layer.unwrap()));
    };
    app.run("layer.new.layer", json!({})).unwrap();
    push(app);
    app.run("layer.layerMask.revealAll", json!({})).unwrap();
    push(app);
    app.run("layer.layerStyle.dropShadow", json!({"size": 2})).unwrap();
    push(app);
    app.run("layer.createClippingMask", json!({})).unwrap();
    push(app);
    app.run("type.create", json!({"x": 2, "y": 20, "text": "Hi", "size": 12})).unwrap();
    push(app);
    app.run("shape.create", json!({"kind": "rect", "rect": [2, 2, 10, 10], "fill": "#ff0000"})).unwrap();
    push(app);
    app.run("layer.smartObjects.convertToSmartObject", json!({})).unwrap();
    push(app);
    app.run("layer.new.group", json!({})).unwrap();
    push(app);
    out
}

#[test]
fn every_layer_row_the_menu_builds_is_known_to_the_editor() {
    let mut app = app();
    let known = menus::universe(LAYERS_ROW);
    for layer in layers(&mut app) {
        for multi in [false, true] {
            for selection in [false, true] {
                let built = crate::layer_menu_ui::entries(&layer, multi, selection);
                let mut at = 0;
                for (_, id) in built.iter().flatten() {
                    // a row the editor does not know: add it to menus::LAYER_ROWS
                    let Some(p) = known.iter().skip(at).position(|k| k == id) else {
                        panic!("`{id}` ({:?}, multi {multi}) is not in menu_custom::menus::LAYER_ROWS, or out of its order", layer.name);
                    };
                    at += p + 1;
                }
            }
        }
    }
}

#[test]
fn every_tab_row_the_menu_builds_is_known_to_the_editor() {
    // `canvas/tests.rs` pins tab_context_items; here the editor's list is what must match it
    let known = menus::universe(TAB);
    assert_eq!(known, ["file.close", "file.closeOthers", "file.closeAll", "file.revealInFinder"]);
}

#[test]
fn the_layer_menu_keeps_the_rows_of_its_layer_kind() {
    let mut app = app();
    save(&mut app, LAYERS_ROW, arrangement(&["cmd:layer.mergeVisible", "---", "cmd:layer.rasterize.type", "cmd:layer.delete"], &["layer.duplicate"]));
    for layer in layers(&mut app) {
        let built = crate::layer_menu_ui::entries(&layer, false, false);
        let got = layer_rows(&app, built.clone());
        let ids = ids(&got);
        assert_eq!(ids[0], "layer.mergeVisible");
        assert!(!ids.contains(&"layer.duplicate"));
        // the type layer's own row stays; others never get it
        assert_eq!(ids.contains(&"layer.rasterize.type"), built.iter().flatten().any(|r| r.1 == "layer.rasterize.type"));
        // a mask's rows follow the layer, not the arrangement
        assert_eq!(ids.contains(&"layer.layerMask.apply"), layer.mask.is_some());
        // nothing the layer did not build is added, unless the arrangement names it
        for id in ids.iter().filter(|i| **i != "---") {
            assert!(built.iter().flatten().any(|r| r.1 == *id) || ["layer.mergeVisible", "layer.rasterize.type"].contains(id));
        }
    }
}

#[test]
fn old_preferences_load_with_nothing_arranged() {
    let old = json!({"general": {}, "shortcuts": {}});
    let p: photocraft_engine::prefs::Preferences = serde_json::from_value(old).unwrap();
    assert!(p.context_menus.is_null());
    assert_eq!(Custom::from_value(&p.context_menus), Custom::default());
    assert!(Custom::default().to_value().is_null());
}

#[test]
fn a_broken_saved_value_loads_as_far_as_it_reads() {
    for junk in [json!(5), json!("x"), json!([1, 2]), json!({"menus": 4}), json!({"menus": {"layers.row": 3, "nope": {"items": ["cmd:a"]}}}), json!({"presets": {"slots": "x"}})] {
        let c = Custom::from_value(&junk);
        assert!(c.menus.is_empty() && c.presets.is_empty(), "{junk}");
    }
    let c = Custom::from_value(&json!({"menus": {"layers.row": {"items": ["cmd:layer.delete", 4, null], "hidden": ["layer.exportAs", 7]}}}));
    assert_eq!(c.menus["layers.row"], arrangement(&["cmd:layer.delete"], &["layer.exportAs"]));
}

#[test]
fn the_arrangement_round_trips_through_the_preferences() {
    let mut app = app();
    let mut custom = Custom::default();
    custom.menus.insert(LAYERS_ROW.into(), arrangement(&["cmd:layer.delete", "---", "cmd:edit.copy"], &["layer.exportAs"]));
    custom.save_to(0).unwrap();
    store(&mut app, &custom);
    let saved = photocraft_engine::prefs::Preferences::get(app.session.prefs(), "contextMenus").unwrap();
    assert_eq!(Custom::from_value(&saved), custom);
    // agents set it with prefs.set too
    app.run("prefs.set", json!({"path": "contextMenus", "value": Value::Null})).unwrap();
    assert_eq!(load(&app), Custom::default());
    app.run("prefs.set", json!({"path": "contextMenus", "value": saved})).unwrap();
    assert_eq!(load(&app), custom);
}

#[test]
fn the_editor_lists_and_commits_by_menu() {
    let key = LAYERS_ROW;
    let mut custom = Custom::default();
    let mut shown = editor::slots(&custom, key);
    assert_eq!(shown, editor::default_slots(key));
    // taking a row out makes an arrangement that lists it as hidden
    shown.retain(|s| *s != Slot::Cmd("layer.exportAs".into()));
    editor::set_slots(&mut custom, key, &shown);
    assert_eq!(custom.menus[key].hidden, ["layer.exportAs"]);
    assert!(editor::available(key, &shown, "").iter().any(|a| a.id == "layer.exportAs" && a.path == "Menu row"));
    // that row first among the offers, then the menu bar's commands; a search narrows both
    assert_eq!(editor::available(key, &shown, "export as").first().map(|a| a.id), Some("layer.exportAs"));
    assert!(editor::available(key, &shown, "zzzz nothing").is_empty());
    // putting the menu back as it was is no arrangement
    editor::set_slots(&mut custom, key, &editor::default_slots(key));
    assert!(custom.menus.is_empty());
    // the tab menu offers only its own rows
    assert!(editor::available(TAB, &[], "").iter().all(|a| a.path == "Menu row"));
}

#[test]
fn every_menu_has_rows_and_a_title() {
    for (key, title) in MENUS {
        assert!(menus::is_key(key) && !title.is_empty());
        assert!(menus::universe(key).len() >= 4, "{key}");
        for id in menus::universe(key) {
            assert!(menus::label(key, id).is_some());
        }
    }
    assert!(!menus::is_key("nope"));
}

#[test]
fn the_catalog_has_the_menu_bars_live_commands() {
    assert!(catalog::find("edit.copy").is_some_and(|c| c.path.starts_with("Edit")));
    assert!(catalog::find("no.such").is_none());
    let all = catalog::all();
    let mut ids: Vec<&str> = all.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), all.len(), "each command once");
    assert!(all.iter().all(|c| crate::menus::is_live(c.id) && !c.label.is_empty()));
}

mod page {
    use egui_kittest::{Harness, kittest::Queryable};

    use super::*;

    fn harness(app: PhotocraftApp, height: f32) -> Harness<'static, PhotocraftApp> {
        let mut h = Harness::builder().with_size(egui::vec2(900.0, height + 40.0)).build_ui_state(
            move |ui, app: &mut PhotocraftApp| {
                if !ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                    return;
                }
                egui::CentralPanel::default().show(ui, |ui| editor::page(app, ui, height));
            },
            app,
        );
        PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::default());
        h.run_steps(4);
        h
    }

    fn click(h: &mut Harness<'static, PhotocraftApp>, label: &str) {
        h.get_by_label(label).click();
        h.run_steps(3);
    }

    #[test]
    fn the_page_takes_a_row_out_adds_a_command_and_resets() {
        let mut h = harness(app(), 520.0);
        click(&mut h, "Deselect");
        click(&mut h, "Take out of the menu");
        let custom = load(h.state());
        let a = &custom.menus[CANVAS_SELECTION];
        assert_eq!(a.hidden, ["select.deselect"]);
        assert!(!a.items.contains(&"cmd:select.deselect".to_string()) && a.items.contains(&"cmd:select.inverse".to_string()));
        // the right-click menu follows at once
        let menu = selection_menu(h.state_mut());
        assert!(!canvas_has(h.state(), &menu, "select.deselect") && canvas_has(h.state(), &menu, "select.inverse"));
        // + on the first offer (the row taken out) puts it back, below the picked row ...
        h.get_all_by_label("Add to the menu").next().unwrap().click();
        h.run_steps(3);
        assert!(load(h.state()).menus[CANVAS_SELECTION].items.contains(&"cmd:select.deselect".to_string()));
        // ... and the next offer is a command of the menu bar, the first of its File menu
        h.get_all_by_label("Add to the menu").next().unwrap().click();
        h.run_steps(3);
        assert!(load(h.state()).menus[CANVAS_SELECTION].items.contains(&"cmd:file.new".to_string()));
        click(&mut h, "Add Separator");
        assert!(load(h.state()).menus[CANVAS_SELECTION].items.iter().any(|i| i == "---"));
        // Reset puts the menu back as it came
        click(&mut h, "Reset");
        assert!(load(h.state()).menus.is_empty());
    }

    #[test]
    fn the_preset_dropdown_saves_switches_clears_and_undoes() {
        let mut h = harness(app(), 520.0);
        click(&mut h, "Deselect");
        click(&mut h, "Take out of the menu");
        let arranged = load(h.state()).menus;
        assert!(!arranged.is_empty());
        click(&mut h, "Presets");
        click(&mut h, "Save to Preset");
        click(&mut h, "1");
        assert!(load(h.state()).presets.get(0).is_some());
        assert_eq!(load(h.state()).menus, arranged, "saving changes no menu");
        // the label names the active preset
        h.get_by_label("Preset 1");
        // back to the default menus, then switch to Preset 1 and undo the switch
        click(&mut h, "Reset");
        assert!(load(h.state()).menus.is_empty());
        click(&mut h, "Preset 1 •");
        click(&mut h, "Preset 1");
        assert_eq!(load(h.state()).menus, arranged);
        click(&mut h, "Preset 1");
        click(&mut h, "Undo");
        assert!(load(h.state()).menus.is_empty());
        // clear the slot
        click(&mut h, "Preset 1 •");
        click(&mut h, "Clear Preset");
        // the switch entry and the submenu's entry are both called Preset 1: the submenu's is last
        h.get_all_by_label("Preset 1").last().unwrap().click();
        h.run_steps(3);
        assert!(load(h.state()).presets.is_empty());
    }

    #[test]
    fn the_presets_control_is_one_line_with_the_heading_and_the_lists_fill_the_height() {
        let tall = harness(app(), 700.0);
        let title = tall.get_by_label("Context Menus").rect();
        let presets = tall.get_by_label("Presets").rect();
        let export = tall.get_by_label("Export Context Menus…").rect();
        let import = tall.get_by_label("Import Context Menus…").rect();
        assert!((title.center().y - presets.center().y).abs() < 6.0, "{title:?} {presets:?}");
        assert!((export.center().y - presets.center().y).abs() < 6.0 && (import.center().y - presets.center().y).abs() < 6.0);
        assert!(presets.left() < export.left() && export.left() < import.left(), "dropdown, export, import from the left");
        // the lists grow with the room the page has
        assert!(editor::list_height(700.0, 80.0) > editor::list_height(400.0, 80.0));
        assert_eq!(editor::list_height(100.0, 80.0), 160.0, "never under the least height");
    }
}
