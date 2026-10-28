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
