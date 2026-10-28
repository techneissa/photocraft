use egui::{Event, Key, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;

use super::super::nav::{self, Hit, MAX_ROWS, PAGE};
use super::super::scroll_id;
use super::all_hits;
use crate::PhotocraftApp;

fn app_harness() -> Harness<'static, PhotocraftApp> {
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
    });
    h.state_mut().run("file.new", json!({"width": 64, "height": 64})).unwrap();
    h.state_mut().sync_views();
    h
}

/// An app with the palette open on an empty query (no recent commands: the full list).
fn open_palette() -> Harness<'static, PhotocraftApp> {
    let mut h = app_harness();
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(3);
    h
}

fn sel_id() -> egui::Id {
    egui::Id::unique("palette-sel")
}

fn sel(h: &Harness<'_, PhotocraftApp>) -> usize {
    h.ctx.data(|d| d.get_temp(sel_id()).unwrap_or(0))
}

fn scroll(h: &Harness<'_, PhotocraftApp>) -> f32 {
    h.ctx.data(|d| d.get_temp(scroll_id()).unwrap_or(0.0))
}

fn hits(h: &Harness<'_, PhotocraftApp>, q: &str) -> Vec<Hit> {
    all_hits(h.state(), q, &[], crate::i18n::Lang::EN)
}

fn press(h: &mut Harness<'_, PhotocraftApp>, key: Key) {
    h.key_press(key);
    h.run_steps(2);
}

fn drawn(h: &Harness<'_, PhotocraftApp>, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn labels(hits: &[Hit]) -> Vec<&str> {
    hits.iter().map(|h| h.2.as_str()).collect()
}

#[test]
fn more_than_twelve_results_scroll_instead_of_being_cut() {
    let mut h = app_harness();
    // Labels the window shows without the palette (toolbar tools, panels): not palette rows.
    let all = hits(&h, "");
    let n = all.len();
    assert!(n > MAX_ROWS * 4, "the full list has far more than {MAX_ROWS} rows (got {n})");
    h.run_steps(2);
    let before: Vec<bool> = all.iter().map(|x| drawn(&h, x.2.as_str())).collect();
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(3);
    // Only the rows on show are drawn…
    let rows = all.iter().zip(&before).filter(|(x, was)| !**was && drawn(&h, x.2.as_str())).count();
    assert!((MAX_ROWS - 3..=MAX_ROWS + 1).contains(&rows), "{rows} new rows drawn: twelve on show, the rest scroll");
    // …and the highlight can reach the rest, scrolling the list with it.
    for _ in 0..4 {
        press(&mut h, Key::PageDown);
    }
    assert_eq!(sel(&h), 4 * PAGE);
    assert!(scroll(&h) > 0.0, "the list scrolled to keep row {} in view", sel(&h));
    let hit_labels = labels(&all).iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(drawn(&h, hit_labels[sel(&h)].as_str()), "the highlighted row is drawn");
}

#[test]
fn page_up_and_page_down_move_a_page_and_stop_at_the_ends() {
    let mut h = open_palette();
    let last = hits(&h, "").len() - 1;
    press(&mut h, Key::PageDown);
    assert_eq!(sel(&h), PAGE);
    press(&mut h, Key::PageUp);
    assert_eq!(sel(&h), 0);
    press(&mut h, Key::PageUp);
    assert_eq!(sel(&h), 0, "PageUp stops at the top");
    for _ in 0..last / PAGE + 2 {
        press(&mut h, Key::PageDown);
    }
    assert_eq!(sel(&h), last, "PageDown stops at the bottom");
}

#[test]
fn up_and_down_wrap_around() {
    let mut h = open_palette();
    let last = hits(&h, "").len() - 1;
    press(&mut h, Key::ArrowUp);
    assert_eq!(sel(&h), last, "Up from the top goes to the last row");
    assert!(scroll(&h) > 0.0, "and scrolls there");
    press(&mut h, Key::ArrowDown);
    assert_eq!(sel(&h), 0, "Down from the last row goes to the first");
    assert_eq!(scroll(&h), 0.0);
    press(&mut h, Key::ArrowDown);
    assert_eq!(sel(&h), 1);
}

#[test]
fn nav_steps() {
    assert_eq!(nav::step(0, -1, 5), 4);
    assert_eq!(nav::step(4, 1, 5), 0);
    assert_eq!(nav::step(2, 1, 5), 3);
    assert_eq!(nav::step(0, 1, 0), 0, "an empty list has no row to move to");
    assert_eq!(nav::page(3, -10, 20), 0);
    assert_eq!(nav::page(15, 10, 20), 19);
    assert_eq!(nav::page(0, 10, 0), 0);
    let h = nav::ROW_H * 4.0;
    assert_eq!(nav::reveal(0.0, 2, h), 0.0, "a row in view doesn't scroll");
    assert_eq!(nav::reveal(0.0, 6, h), nav::ROW_H * 7.0 - h, "a row below scrolls it to the bottom edge");
    assert_eq!(nav::reveal(nav::ROW_H * 10.0, 3, h), nav::ROW_H * 3.0, "a row above scrolls it to the top edge");
}

#[test]
fn the_pointer_moving_onto_a_row_moves_the_highlight() {
    let mut h = open_palette();
    h.event(Event::Text("layer".into()));
    h.run_steps(3);
    // A menu command row on show whose name is nowhere else in the window.
    let all = hits(&h, "layer");
    let at = (3..MAX_ROWS).find(|&i| h.query_all_by_label(all[i].2.as_str()).count() == 1).expect("a row with a name of its own");
    let target = all[at].2.clone();
    assert_eq!(sel(&h), 0);
    h.get_by_label(target.as_str()).hover();
    h.run_steps(3);
    assert_eq!(sel(&h), at, "hovering {target:?} highlights it");
    // A pointer that keeps still doesn't take the highlight back from the keys.
    press(&mut h, Key::ArrowDown);
    assert_eq!(sel(&h), at + 1);
    h.run_steps(3);
    assert_eq!(sel(&h), at + 1);
}

#[test]
fn a_new_query_starts_at_the_top() {
    let mut h = open_palette();
    for _ in 0..3 {
        press(&mut h, Key::PageDown);
    }
    assert!(sel(&h) > 0);
    h.event(Event::Text("zoom".into()));
    h.run_steps(3);
    assert_eq!(sel(&h), 0);
    assert_eq!(scroll(&h), 0.0);
}

#[test]
fn rows_show_the_users_own_shortcut() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let detail = |app: &PhotocraftApp, id: &str| hits_for(app, "").into_iter().find(|h| h.1 == id).and_then(|h| h.3).unwrap_or_default();
    // Zoom Tool and Undo: the built-in keys first…
    assert!(detail(&app, "tool:Zoom").ends_with(&crate::shortcuts::pretty("Z")), "{}", detail(&app, "tool:Zoom"));
    assert!(detail(&app, "edit.undo").ends_with(&crate::shortcuts::pretty("Cmd+Z")));
    // …then the user's own.
    set_keys(&mut app, "Alt+9", "Cmd+Alt+9");
    assert!(detail(&app, "tool:Zoom").ends_with(&crate::shortcuts::pretty("Alt+9")), "{}", detail(&app, "tool:Zoom"));
    assert!(detail(&app, "edit.undo").ends_with(&crate::shortcuts::pretty("Cmd+Alt+9")), "{}", detail(&app, "edit.undo"));
    assert!(!detail(&app, "edit.undo").contains(&crate::shortcuts::pretty("Cmd+Z")));
    // A key the user removed shows no key.
    set_keys(&mut app, "", "");
    assert_eq!(detail(&app, "tool:Zoom"), "Tool");
    assert!(!detail(&app, "edit.undo").contains(&crate::shortcuts::pretty("Cmd+Z")));
}

fn set_keys(app: &mut PhotocraftApp, zoom: &str, undo: &str) {
    app.session.prefs.edit(|p| {
        p.shortcuts.insert("tools.select.zoom".into(), zoom.into());
        p.shortcuts.insert("edit.undo".into(), undo.into());
    });
}

fn hits_for(app: &PhotocraftApp, q: &str) -> Vec<Hit> {
    all_hits(app, q, &[], crate::i18n::Lang::EN)
}

#[test]
fn label_word_starts_rank_before_looser_matches_and_disabled_rows_come_last() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let hits = hits_for(&app, "blur");
    let first_loose = hits.iter().position(|h| !h.2.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with("blur")));
    let last_word_start = hits.iter().rposition(|h| h.2.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with("blur")) && h.4);
    if let (Some(loose), Some(word)) = (first_loose, last_word_start) {
        assert!(word < loose || !hits[loose].4, "{:?}: a label with a word starting 'blur' comes before {:?}", hits[word].2, hits[loose].2);
    }
    // Disabled rows (no document: most Filter items) after every enabled one.
    let first_disabled = hits.iter().position(|h| !h.4).expect("a disabled hit for blur");
    assert!(hits[first_disabled..].iter().all(|h| !h.4), "a disabled row is followed by an enabled one");
    // Word starts, in any order.
    assert!(nav::word_start_bonus("sel inv", "Select Inverse") > 0);
    assert!(nav::word_start_bonus("inv sel", "Select Inverse") > 0);
    assert_eq!(nav::word_start_bonus("lect", "Select Inverse"), 0, "a substring in the middle of a word is not a word start");
    assert!(nav::word_start_bonus("sel", "Select Inverse") > nav::word_start_bonus("inv", "Select Inverse"), "the label's own start is best");
    assert_eq!(nav::word_start_bonus("", "Select"), 0);
    // "Zoom" finds Zoom Tool and View ▸ Zoom In before a command that only has the letters.
    let zoom = hits_for(&app, "zoom");
    assert!(zoom[0].2.to_lowercase().contains("zoom"), "{:?}", labels(&zoom[..3]));
}

#[test]
fn a_palette_opened_under_a_resting_pointer_keeps_the_highlight_on_the_first_row() {
    let mut h = app_harness();
    // The pointer rests where a lower row will be.
    h.event(egui::Event::PointerMoved(egui::pos2(700.0, 330.0)));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.search", json!({})).unwrap();
    h.run_steps(4);
    assert_eq!(sel(&h), 0, "the row under a pointer that hasn't moved isn't highlighted");
    // Moving it does.
    h.event(egui::Event::PointerMoved(egui::pos2(702.0, 331.0)));
    h.run_steps(3);
    assert!(sel(&h) > 0);
}

#[test]
fn a_list_of_one_row_wraps_and_pages_in_place() {
    let mut h = open_palette();
    let all = hits(&h, "");
    let q = all.iter().map(|x| x.2.clone()).find(|l| l.len() > 12 && hits(&h, l).len() == 1).expect("a label that matches only itself");
    h.event(Event::Text(q));
    h.run_steps(3);
    for key in [Key::ArrowDown, Key::ArrowUp, Key::PageDown, Key::PageUp] {
        press(&mut h, key);
        assert_eq!(sel(&h), 0, "{key:?} on a one-row list stays on it");
    }
    assert!(h.state().ui.palette_open);
}
