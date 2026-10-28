use egui::accesskit::Role;
use egui::{Event, Pos2, Rect, vec2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;
use photocraft_engine::Session;
use photocraft_engine::prefs::TopBar;
use serde_json::json;

use super::*;
use crate::panels::title_bar;

const WORKSPACE_MIN: f32 = 90.0;
const SLOT: f32 = 34.0;

/// The bar as it was before the items could be hidden.
fn old_fit(room: f32) -> (bool, f32, bool, bool) {
    let icons: u8 = if room >= WORKSPACE_MIN + 2.0 * SLOT {
        2
    } else if room >= WORKSPACE_MIN + SLOT {
        1
    } else {
        0
    };
    (room >= WORKSPACE_MIN, (room - f32::from(icons) * SLOT - 6.0).clamp(WORKSPACE_MIN, 130.0), icons >= 1, icons >= 2)
}

fn hidden(ids: &[&str]) -> TopBar {
    let mut bar = TopBar::default();
    for id in ids {
        assert!(bar.set(id, false), "{id}");
    }
    bar
}

#[test]
fn with_every_item_shown_the_bar_fits_as_it_always_did() {
    let bar = TopBar::default();
    // Every width from a sliver to a wide window, and the odd values (NaN, infinity, negative).
    let rooms = (0..1600).map(|n| n as f32 * 0.5).chain([f32::NAN, f32::INFINITY, -5.0, 89.99, 90.0, 123.99, 124.0, 157.99, 158.0]);
    for room in rooms {
        let (workspace, width, search, appearance) = old_fit(room);
        let got = fit(room, &bar, WORKSPACE_MIN, SLOT);
        assert_eq!(got.workspace.is_some(), workspace, "{room}");
        if let Some(w) = got.workspace {
            assert_eq!(w, width, "{room}");
        }
        assert_eq!((got.search, got.appearance), (search, appearance), "{room}");
    }
}

#[test]
fn a_hidden_item_gives_its_room_to_the_rest_and_nothing_overlaps() {
    // No workspace switcher: the icons need only their own room.
    let no_ws = hidden(&["workspace"]);
    assert_eq!(fit(2.0 * SLOT, &no_ws, WORKSPACE_MIN, SLOT), Fit { workspace: None, search: true, appearance: true });
    assert_eq!(fit(SLOT, &no_ws, WORKSPACE_MIN, SLOT), Fit { workspace: None, search: true, appearance: false });
    assert_eq!(fit(SLOT - 1.0, &no_ws, WORKSPACE_MIN, SLOT), Fit { workspace: None, search: false, appearance: false });
    // No Search icon: the Appearance icon is the one that stays or goes, and it stays one slot sooner.
    let no_search = hidden(&["search"]);
    let f = fit(WORKSPACE_MIN + SLOT, &no_search, WORKSPACE_MIN, SLOT);
    assert_eq!((f.workspace, f.search, f.appearance), (Some(WORKSPACE_MIN), false, true));
    assert!(!fit(WORKSPACE_MIN + SLOT - 1.0, &no_search, WORKSPACE_MIN, SLOT).appearance);
    // Nothing shown: nothing drawn at any width.
    let none = hidden(&["workspace", "search", "appearance"]);
    for room in [0.0, 50.0, 400.0] {
        let f = fit(room, &none, WORKSPACE_MIN, SLOT);
        assert!(!f.any(), "{room}");
    }
    // The switcher takes the room the icons leave, up to its widest.
    assert_eq!(fit(400.0, &hidden(&["search", "appearance"]), WORKSPACE_MIN, SLOT).workspace, Some(130.0));
    assert_eq!(fit(100.0, &hidden(&["search", "appearance"]), WORKSPACE_MIN, SLOT).workspace, Some(94.0));
    // Whatever is hidden, what is drawn fits in the room (a room too small for the switcher draws none).
    for mask in 0..8u8 {
        let bar = TopBar { workspace: mask & 1 == 0, search: mask & 2 == 0, appearance: mask & 4 == 0, ..TopBar::default() };
        for room in (0..600).map(|n| n as f32) {
            let f = fit(room, &bar, WORKSPACE_MIN, SLOT);
            let used = f.workspace.unwrap_or(0.0) + (usize::from(f.search) + usize::from(f.appearance)) as f32 * SLOT;
            assert!(used <= room + 1e-3, "{mask:03b} {room}: {f:?} uses {used}");
        }
    }
}

/// The title bar alone in a `width` pt wide window, with `bar` as the saved items.
fn harness(width: f32, bar: &TopBar) -> Harness<'static, PhotocraftApp> {
    let mut app = PhotocraftApp::new(Session::new(), crate::Services::default());
    for id in TopBar::IDS {
        app.run("prefs.set", json!({"path": format!("interface.topBar.{id}"), "value": bar.shown(id)})).unwrap();
    }
    let mut h = Harness::builder()
        .with_size(vec2(width, 80.0))
        .with_context_setup(|ctx| PhotocraftApp::setup_context(ctx, crate::theme::ThemeKind::ALL[0]))
        .build_ui_state(|ui, app| title_bar(app, ui), app);
    h.run_steps(3);
    h
}

/// Whether the bar has an icon named `name`, not counting the right-click menu's check boxes.
fn has_icon(h: &Harness<'_, PhotocraftApp>, name: &str) -> bool {
    h.query_all_by_label_contains(name).any(|n| n.accesskit_node().role() != Role::CheckBox)
}

/// Which items the bar draws: (logo, title, workspace, search, appearance).
fn drawn(h: &Harness<'_, PhotocraftApp>) -> [bool; 5] {
    let title = {
        fn walk(s: &egui::Shape) -> bool {
            match s {
                egui::Shape::Text(t) => t.galley.text() == "PhotoCraft",
                egui::Shape::Vec(v) => v.iter().any(walk),
                _ => false,
            }
        }
        h.output().shapes.iter().any(|c| walk(&c.shape))
    };
    [
        crate::brand::mark_rect(&h.ctx).is_some(),
        title,
        h.query_by_role_and_label(Role::ComboBox, "Workspace").is_some(),
        has_icon(h, "Search commands"),
        has_icon(h, "Appearance Mode"),
    ]
}

#[test]
fn every_item_is_drawn_by_default_and_each_hides_alone() {
    let h = harness(1440.0, &TopBar::default());
    assert_eq!(drawn(&h), [true; 5]);
    for (n, id) in TopBar::IDS.iter().enumerate() {
        let h = harness(1440.0, &hidden(&[id]));
        let mut want = [true; 5];
        want[n] = false;
        assert_eq!(drawn(&h), want, "hiding {id}");
        // The menus stay.
        assert!(h.query_by_label("File").is_some() && h.query_by_label("Help").is_some(), "the menus stay when {id} is hidden");
    }
}

#[test]
fn every_item_can_be_hidden_at_once_and_the_menus_stay() {
    for width in [760.0, 1100.0, 1440.0] {
        let h = harness(width, &hidden(&TopBar::IDS));
        assert_eq!(drawn(&h), [false; 5], "{width}");
        assert!(h.query_by_label("File").is_some() && h.query_by_label("Help").is_some(), "{width}");
    }
}

#[test]
fn hiding_the_switcher_moves_the_icons_to_the_end_and_the_rest_keeps_its_place() {
    let all = harness(1440.0, &TopBar::default());
    let search_all = all.get_by_label_contains("Search commands").rect();
    let appearance_all = all.get_by_label_contains("Appearance Mode").rect();
    let file_all = all.get_by_label("File").rect();
    let h = harness(1440.0, &hidden(&["workspace"]));
    let (search, appearance) = (h.get_by_label_contains("Search commands").rect(), h.get_by_label_contains("Appearance Mode").rect());
    assert!(search.left() > search_all.left() + 100.0, "Search takes the end of the bar: {search:?} vs {search_all:?}");
    assert!((search.left() - appearance.left() - 34.0).abs() < 1.0 && appearance.left() < search.left(), "the icons keep their order and spacing");
    assert!(appearance.left() > appearance_all.left());
    assert_eq!(h.get_by_label("File").rect(), file_all, "the menus do not move");
    assert!(search.right() <= 1440.0 - 10.0 + 1.0, "{search:?} stays inside the bar's margin");
}

#[test]
fn a_hidden_title_or_logo_leaves_the_drag_area_whole() {
    // The gap between the menus and the controls is what drags the window; it is the same gap
    // whether or not a title is drawn in it, and a hidden logo only widens the room.
    let span = |h: &Harness<'_, PhotocraftApp>| {
        let rects: Vec<Rect> = h.ctx.viewport(|v| {
            v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter()).filter(|w| w.sense.senses_drag() && w.rect.top() < 30.0).map(|w| w.rect).collect()
        });
        assert_eq!(rects.len(), 1, "one drag area: {rects:?}");
        rects[0]
    };
    let all = span(&harness(1440.0, &TopBar::default()));
    let no_title = span(&harness(1440.0, &hidden(&["title"])));
    assert_eq!(all, no_title);
    let no_logo = span(&harness(1440.0, &hidden(&["brand"])));
    assert!(no_logo.left() < all.left(), "the menus move left by the logo's room: {no_logo:?} vs {all:?}");
    let empty = span(&harness(1440.0, &hidden(&TopBar::IDS)));
    assert!(empty.right() > all.right() + 100.0, "with nothing to its right the gap runs on to the margin: {empty:?} vs {all:?}");
}

fn right_click(h: &mut Harness<'_, PhotocraftApp>, at: Pos2) {
    h.event(Event::PointerMoved(at));
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() });
        h.run_steps(1);
    }
    h.run_steps(2);
}

/// A point in the bar's free gap (left of the title, right of the menus).
fn gap(h: &Harness<'_, PhotocraftApp>) -> Pos2 {
    let menus = h.get_by_label("Help").rect().right();
    Pos2::new(menus + 60.0, 15.0)
}

#[test]
fn right_clicking_the_bar_lists_every_item_and_toggles_them() {
    let mut h = harness(1440.0, &TopBar::default());
    assert!(h.query_by_role_and_label(Role::CheckBox, "Document Title").is_none(), "no menu before the click");
    let at = gap(&h);
    right_click(&mut h, at);
    for item in &ITEMS {
        assert!(h.query_by_role_and_label(Role::CheckBox, item.label).is_some(), "the menu lists {}", item.label);
    }
    h.get_by_label("Top Bar Settings…");
    // A click turns an item off, keeps the menu open, and the bar follows.
    h.get_by_role_and_label(Role::CheckBox, "Appearance Mode").click_accesskit();
    h.run_steps(3);
    assert!(!h.state().session.prefs().interface.top_bar.appearance);
    assert_eq!(drawn(&h), [true, true, true, true, false]);
    h.get_by_role_and_label(Role::CheckBox, "Document Title").click_accesskit();
    h.run_steps(3);
    assert!(!h.state().session.prefs().interface.top_bar.title);
    assert_eq!(drawn(&h), [true, false, true, true, false]);
    assert!(h.query_by_role_and_label(Role::CheckBox, "Logo").is_some(), "the menu stays open while items are toggled");
    // And on again.
    h.get_by_role_and_label(Role::CheckBox, "Appearance Mode").click_accesskit();
    h.run_steps(3);
    assert_eq!(h.state().session.prefs().interface.top_bar, hidden(&["title"]));
}

#[test]
fn the_menu_opens_even_when_every_item_is_hidden() {
    let mut h = harness(1440.0, &hidden(&TopBar::IDS));
    let at = gap(&h);
    right_click(&mut h, at);
    for item in &ITEMS {
        assert!(h.query_by_role_and_label(Role::CheckBox, item.label).is_some(), "the menu lists {}", item.label);
    }
    h.get_by_role_and_label(Role::CheckBox, "Workspace").click_accesskit();
    h.run_steps(3);
    assert!(h.state().session.prefs().interface.top_bar.workspace);
    assert!(drawn(&h)[2], "the switcher is back");
}

#[test]
fn the_menu_opens_the_interface_page_of_preferences() {
    let mut h = harness(1440.0, &TopBar::default());
    let at = gap(&h);
    right_click(&mut h, at);
    h.get_by_label("Top Bar Settings…").click_accesskit();
    h.run_steps(3);
    let dialog = h.state().ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(|v| v.as_str()) == Some("prefs")).expect("Preferences opened");
    assert_eq!(dialog.fields.get("section").and_then(|v| v.as_str()), Some("interface"));
}

#[test]
fn the_preferences_page_has_a_top_bar_group_that_edits_the_items() {
    let mut app = PhotocraftApp::new(Session::new(), crate::Services::default());
    crate::prefs_ui::open_preferences(&mut app, "interface");
    let mut h = Harness::builder()
        .with_size(vec2(1000.0, 1600.0))
        .with_context_setup(|ctx| PhotocraftApp::setup_context(ctx, crate::theme::ThemeKind::ALL[0]))
        .build_eframe(move |_| app);
    h.run_steps(4);
    h.get_by_label("Top Bar");
    for item in &ITEMS {
        h.get_by_role_and_label(Role::CheckBox, item.label);
    }
    // A click on an item edits the dialog's working copy; OK saves it.
    h.get_by_role_and_label(Role::CheckBox, "Search").click_accesskit();
    h.run_steps(2);
    assert!(h.state().session.prefs().interface.top_bar.search, "nothing is saved before OK");
    h.get_by_label("OK").click_accesskit();
    h.run_steps(3);
    assert_eq!(h.state().session.prefs().interface.top_bar, hidden(&["search"]));
}

#[test]
fn a_preferences_search_shows_or_skips_the_group() {
    let render = |search: &str| {
        let mut app = PhotocraftApp::new(Session::new(), crate::Services::default());
        let id = crate::prefs_ui::open_preferences(&mut app, "interface");
        app.ui.dialog_mut(id).unwrap().fields.insert("__search".into(), json!(search));
        let mut h = Harness::builder()
            .with_size(vec2(1000.0, 900.0))
            .with_context_setup(|ctx| PhotocraftApp::setup_context(ctx, crate::theme::ThemeKind::ALL[0]))
            .build_eframe(move |_| app);
        h.run_steps(4);
        h
    };
    // A key of the page must match for the page to stay open ("title": Show File Name In Title).
    for hit in ["top bar", "top", "title"] {
        assert!(render(hit).query_by_label("Top Bar").is_some(), "{hit}");
    }
    assert!(render("language").query_by_label("Top Bar").is_none());
    assert!(render("").query_by_label("Top Bar").is_some());
}

#[test]
fn the_items_are_the_engines_items() {
    // The page and the menu list exactly what `interface.topBar` stores, in the bar's order.
    assert_eq!(ITEMS.map(|i| i.id), TopBar::IDS);
}
