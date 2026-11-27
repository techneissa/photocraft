use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use photocraft_doc::BlendMode;

use crate::PhotocraftApp;
use crate::options_bar_mode_wheel_tests as bar;
use crate::state::Tool;

fn open_mode_list() -> Harness<'static, Box<PhotocraftApp>> {
    let mut h = bar::harness(Tool::Brush);
    h.get_by_role_and_label(Role::ComboBox, "Mode").click();
    h.run_steps(4);
    h
}

fn rows(h: &Harness<'static, Box<PhotocraftApp>>) -> Vec<String> {
    h.query_all_by_role(Role::ListBoxOption).filter_map(|n| n.accesskit_node().label()).collect()
}

fn type_text(h: &mut Harness<'static, Box<PhotocraftApp>>, text: &str) {
    h.event(egui::Event::Text(text.into()));
    h.run_steps(3);
}

/// The blend mode lists have 29 entries: the typed letters narrow them to the modes that contain them, which the
/// first-letter jump of a plain list cannot do ("burn" starts none of them).
#[test]
fn typing_in_an_open_blend_mode_list_narrows_it_and_enter_picks() {
    let mut h = open_mode_list();
    assert!(rows(&h).len() >= 20, "the whole list shows before anything is typed");
    type_text(&mut h, "burn");
    let shown = rows(&h);
    assert!(shown.iter().any(|r| r == "Color Burn") && shown.iter().any(|r| r == "Linear Burn"), "both burn modes match: {shown:?}");
    assert!(!shown.iter().any(|r| r == "Multiply"), "a mode without the letters is gone: {shown:?}");
    h.key_press(egui::Key::ArrowDown);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(h.state().session.tools.brush.mode, BlendMode::LinearBurn, "the second match is picked");
}

/// Letters typed on the closed box open the list with all of them in the search field (the first is not eaten by a
/// jump to the entry it begins), and Enter picks the first match without an arrow key.
#[test]
fn letters_typed_on_the_closed_box_all_reach_the_search_and_enter_picks_the_first_match() {
    let mut h = bar::harness(Tool::Brush);
    h.get_by_role_and_label(Role::ComboBox, "Mode").focus();
    h.run_steps(2);
    type_text(&mut h, "b");
    type_text(&mut h, "urn");
    assert_eq!(h.get_by_label("Mode Search").accesskit_node().value().as_deref(), Some("burn"), "the first letter stays");
    assert_eq!(rows(&h), ["Color Burn", "Linear Burn"]);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(h.state().session.tools.brush.mode, BlendMode::ColorBurn);
}
