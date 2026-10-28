use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{Input, Search, matches};

const BLEND: [&str; 13] = [
    "Normal",
    "Dissolve",
    "Darken",
    "Multiply",
    "Linear Burn",
    "Lighten",
    "Linear Dodge",
    "Lighter Color",
    "Soft Light",
    "Linear Light",
    "Pin Light",
    "Hard Mix",
    "Delimit",
];

fn names(query: &str, enabled: &[bool]) -> Vec<&'static str> {
    matches(query, &BLEND, enabled).into_iter().filter_map(|i| BLEND.get(i).copied()).collect()
}

#[test]
fn label_starts_come_first_then_word_starts_then_the_rest() {
    let found = names("li", &[]);
    assert_eq!(found, ["Linear Burn", "Lighten", "Linear Dodge", "Lighter Color", "Linear Light", "Soft Light", "Pin Light", "Delimit"]);
}

#[test]
fn case_is_ignored_and_a_blank_query_matches_everything() {
    assert_eq!(names("DARK", &[]), ["Darken"]);
    assert_eq!(names("  ", &[]).len(), BLEND.len());
    assert_eq!(names("zz", &[]), Vec::<&str>::new());
    assert_eq!(names("linear b", &[]), ["Linear Burn"], "a space inside the text counts");
}

#[test]
fn a_disabled_option_never_matches() {
    let mut enabled = [true; 13];
    enabled[5] = false; // Lighten
    assert_eq!(names("lighten", &enabled), Vec::<&str>::new());
    assert!(!names("li", &enabled).contains(&"Lighten"));
    assert!(!names("", &enabled).contains(&"Lighten"));
    // An empty label (a gap in the list) is never picked either.
    assert_eq!(matches("", &["", "A"], &[]), [1]);
}

#[test]
fn letters_beyond_ascii_match_and_never_panic() {
    let labels = ["Normal", "Éclaircir", "Ébène", "Нормальный", "İstanbul", "ǅ", "a\u{301}b"];
    assert_eq!(matches("é", &labels, &[]), [1, 2]);
    assert_eq!(matches("ÉB", &labels, &[]), [2]);
    assert_eq!(matches("но", &labels, &[]), [3]);
    for q in ["i\u{307}", "ǆ", "\u{301}", "ß", "😀", "é\u{301}"] {
        let _ = matches(q, &labels, &[]);
    }
}

#[test]
fn typing_moves_the_highlight_and_enter_picks_it() {
    let mut s = Search::default();
    let r = s.apply(Input::Text("L"), &BLEND, &[]);
    assert!(r.consumed);
    s.apply(Input::Text("i"), &BLEND, &[]);
    assert_eq!(s.query, "Li");
    assert_eq!(s.apply(Input::Enter, &BLEND, &[]).pick, Some(4), "the first match, Linear Burn");
    s.apply(Input::Down, &BLEND, &[]);
    s.apply(Input::Down, &BLEND, &[]);
    assert_eq!(s.apply(Input::Enter, &BLEND, &[]).pick, Some(6), "the third match, Linear Dodge");
    for _ in 0..20 {
        s.apply(Input::Down, &BLEND, &[]);
    }
    assert_eq!(s.apply(Input::Enter, &BLEND, &[]).pick, Some(12), "↓ stops at the last match, Delimit");
    s.apply(Input::PageUp, &BLEND, &[]);
    s.apply(Input::Up, &BLEND, &[]);
    assert_eq!(s.active, 1);
    s.apply(Input::Text("x"), &BLEND, &[]);
    assert_eq!(s.apply(Input::Enter, &BLEND, &[]).pick, None, "nothing matches \"lix\"");
}

#[test]
fn backspace_and_escape_clear_before_anything_closes() {
    let mut s = Search::default();
    // Nothing typed: the keys are the list's.
    for key in [Input::Backspace, Input::Esc, Input::Enter, Input::Up, Input::Down] {
        assert!(!s.apply(key, &BLEND, &[]).consumed, "{key:?}");
    }
    s.apply(Input::Text(" "), &BLEND, &[]);
    assert!(!s.is_active(), "a space doesn't start the text");
    s.apply(Input::Text("dis"), &BLEND, &[]);
    s.apply(Input::Backspace, &BLEND, &[]);
    assert_eq!(s.query, "di");
    assert!(s.apply(Input::Esc, &BLEND, &[]).consumed);
    assert_eq!(s.query, "", "the first Esc clears the text");
    assert!(!s.apply(Input::Esc, &BLEND, &[]).consumed, "the second one is the list's");
    s.apply(Input::Text("dis"), &BLEND, &[]);
    s.apply(Input::Clear, &BLEND, &[]);
    assert!(!s.is_active());
    s.apply(Input::Text("é\u{7}"), &BLEND, &[]);
    assert_eq!(s.query, "é", "control characters are left out");
    s.apply(Input::Backspace, &BLEND, &[]);
    assert!(!s.is_active());
}

const OPTIONS: [(usize, &str); 6] = [(0, "Normal"), (1, "Dissolve"), (2, "Darken"), (3, "Multiply"), (4, "Darker Color"), (5, "Linear Light")];

fn dropdown_harness() -> Harness<'static, usize> {
    Harness::builder().with_size(egui::vec2(300.0, 400.0)).build_ui_state(
        |ui, selected: &mut usize| {
            crate::widgets::dropdown(ui, "blend-mode", tl!("Blend Mode"), selected, &OPTIONS, 120.0);
        },
        0,
    )
}

fn open(h: &mut Harness<'static, usize>) {
    h.get_by_role(Role::ComboBox).click();
    h.run();
}

fn type_text(h: &mut Harness<'static, usize>, text: &str) {
    h.event(egui::Event::Text(text.into()));
    h.run();
}

fn shown(h: &Harness<'static, usize>, label: &str) -> bool {
    h.query_by_label(label).is_some()
}

#[test]
fn typing_narrows_the_open_list_and_enter_picks() {
    let mut h = dropdown_harness();
    open(&mut h);
    assert!(shown(&h, "Normal") && shown(&h, "Multiply"), "no field and every option before typing");
    type_text(&mut h, "d");
    assert!(shown(&h, "Darken") && shown(&h, "Dissolve") && shown(&h, "Darker Color"));
    assert!(!shown(&h, "Normal") && !shown(&h, "Multiply"));
    type_text(&mut h, "ar");
    assert!(shown(&h, "Darken") && shown(&h, "Darker Color") && !shown(&h, "Dissolve"));
    assert_eq!(*h.state(), 0, "typing alone picks nothing");
    h.key_press(egui::Key::ArrowDown);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert_eq!(*h.state(), 4, "↓ then ↩ picks the second match, Darker Color");
    assert!(!crate::widgets::dropdown_list_open(&h.ctx), "↩ closes the list");
    // Opened again: the text typed before is gone.
    open(&mut h);
    assert!(shown(&h, "Normal") && shown(&h, "Multiply"));
}

#[test]
fn enter_with_a_search_picks_the_first_match() {
    let mut h = dropdown_harness();
    open(&mut h);
    type_text(&mut h, "LI");
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert_eq!(*h.state(), 5, "\"li\" ranks Linear Light first of the matches");
}

#[test]
fn a_click_on_a_match_picks_it() {
    let mut h = dropdown_harness();
    open(&mut h);
    type_text(&mut h, "mul");
    h.get_by_label("Multiply").click();
    h.run();
    h.run();
    assert_eq!(*h.state(), 3);
    assert!(!crate::widgets::dropdown_list_open(&h.ctx) || !shown(&h, "Multiply"), "the list closes on a click");
}

#[test]
fn escape_clears_the_text_and_then_closes_the_list() {
    let mut h = dropdown_harness();
    open(&mut h);
    type_text(&mut h, "dar");
    assert!(!shown(&h, "Normal"));
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    assert!(shown(&h, "Normal") && shown(&h, "Darken"), "the first Esc clears the text and keeps the list open");
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    h.run();
    assert!(!shown(&h, "Darken"), "the second closes it");
    assert_eq!(*h.state(), 0);
}

#[test]
fn nothing_matching_shows_one_dim_line() {
    let mut h = dropdown_harness();
    open(&mut h);
    type_text(&mut h, "zzz");
    assert!(shown(&h, "No match"));
    assert!(!shown(&h, "Normal") && !shown(&h, "Darken"));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(*h.state(), 0, "↩ with no match picks nothing");
    type_text(&mut h, "é");
    assert!(shown(&h, "No match"), "a letter beyond ASCII doesn't panic");
    h.key_press(egui::Key::Backspace);
    h.key_press(egui::Key::Backspace);
    h.key_press(egui::Key::Backspace);
    h.key_press(egui::Key::Backspace);
    h.run();
    assert!(shown(&h, "Normal"), "Backspace takes the text back out");
}

#[test]
fn the_arrows_still_step_the_value_before_anything_is_typed() {
    let mut h = dropdown_harness();
    open(&mut h);
    h.key_press(egui::Key::ArrowDown);
    h.run();
    assert_eq!(*h.state(), 1);
}

#[test]
fn the_tooltip_dropdown_searches_too() {
    let options = [(0, "Normal", "tip a"), (1, "Dissolve", "tip b"), (2, "Darken", "tip c")];
    let mut h = Harness::builder().with_size(egui::vec2(300.0, 400.0)).build_ui_state(
        move |ui, selected: &mut usize| {
            crate::widgets::dropdown_with_tooltips(ui, "tips", tl!("Mode"), selected, &options, 120.0);
        },
        0,
    );
    h.get_by_role(Role::ComboBox).click();
    h.run();
    h.event(egui::Event::Text("dar".into()));
    h.run();
    assert!(h.query_by_label("Darken").is_some() && h.query_by_label("Normal").is_none());
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert_eq!(*h.state(), 2);
}

#[test]
fn the_field_is_named_search_and_takes_ime_text() {
    let mut h = dropdown_harness();
    open(&mut h);
    type_text(&mut h, "d");
    assert!(h.query_by_label("Search").is_some(), "the field has the accessible name Search");
    h.event(egui::Event::Ime(egui::ImeEvent::Preedit { text: "a".into(), active_range_chars: None }));
    h.run();
    h.event(egui::Event::Ime(egui::ImeEvent::Commit("ar".into())));
    h.run();
    h.run();
    assert!(h.query_by_label("Darken").is_some() && h.query_by_label("Dissolve").is_none(), "an IME commit lands in the field");
}

#[test]
fn a_composing_ime_shows_the_field_before_any_text_is_typed() {
    let mut h = dropdown_harness();
    open(&mut h);
    assert!(h.query_by_label("Search").is_none(), "no field before typing");
    h.event(egui::Event::Ime(egui::ImeEvent::Preedit { text: "d".into(), active_range_chars: None }));
    h.run();
    h.run();
    assert!(h.query_by_label("Search").is_some(), "the field shows to take the preedit");
    assert!(h.query_by_label("Dissolve").is_some() && h.query_by_label("Multiply").is_none(), "the rows follow the preedit text");
    h.event(egui::Event::Ime(egui::ImeEvent::Commit("dar".into())));
    h.run();
    h.run();
    assert!(h.query_by_label("Darken").is_some() && h.query_by_label("Normal").is_none());
}

/// Space typed in the search ("linear b") must not turn the canvas into the Hand tool or pan it.
#[test]
fn space_while_typing_neither_switches_tools_nor_pans() {
    use crate::PhotocraftApp;
    use crate::state::Tool;
    use egui_kittest::kittest::NodeT;
    use serde_json::json;

    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width": 64, "height": 64})).unwrap();
        app.run("layer.new.layer", json!({})).unwrap();
        app.ui.tool = Tool::Brush;
        app
    });
    h.run_steps(6);
    let named = |n: &egui_kittest::Node<'_>| n.accesskit_node().value().as_deref() == Some("Normal");
    h.query_all_by_role(Role::ComboBox).find(|n| named(n)).expect("the Blend Mode dropdown").click();
    h.run_steps(2);
    let center = h.state().ui.views[0].center;
    h.event(egui::Event::Text("linear".into()));
    h.run_steps(2);
    let space =
        |pressed| egui::Event::Key { key: egui::Key::Space, physical_key: Some(egui::Key::Space), pressed, repeat: false, modifiers: egui::Modifiers::NONE };
    h.event(space(true));
    h.event(egui::Event::Text(" ".into()));
    h.run_steps(2);
    assert_eq!(crate::hold_keys::held_tool(h.state(), &h.ctx), None, "Space held in the open list is not the Hand tool");
    assert!(!crate::hold_keys::reposition_held(h.state(), &h.ctx));
    h.event(egui::Event::Text("b".into()));
    h.run_steps(2);
    assert!(h.query_by_label("Linear Burn").is_some() && h.query_by_label("Linear Dodge (Add)").is_none(), "\"linear b\" narrows to Linear Burn");
    assert_eq!(h.state().ui.tool, Tool::Brush);
    assert_eq!(h.state().ui.views[0].center, center, "the canvas didn't pan");
    // Closed, the held Space is the Hand tool's again.
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!crate::widgets::dropdown_list_open(&h.ctx));
    assert_eq!(crate::hold_keys::held_tool(h.state(), &h.ctx), Some(crate::hold_keys::Temporary::Hand));
    h.event(space(false));
    h.run_steps(2);
}
