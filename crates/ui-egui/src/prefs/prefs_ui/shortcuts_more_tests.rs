//! What the fork adds to craft's Keyboard Shortcuts table (fork/shortcuts-window.md): Remove Shortcut on a row's
//! right-click, and the command a clash mark names.

use craft::keymap::{Binding, CommandRef, Policy};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::shortcut_capture_tests::harness;
use crate::PhotocraftApp;

/// Opens the dialog and narrows the table to the commands matching `text`.
fn search(h: &mut Harness<'static, Box<PhotocraftApp>>, text: &str) {
    crate::prefs_ui::open_shortcuts(h.state_mut(), 0);
    h.run_steps(4);
    let field = h.get_all_by_label("Search:").find(|n| format!("{:?}", n.accesskit_node().role()) == "TextInput").expect("the search box");
    field.click();
    h.run_steps(2);
    h.get_all_by_label("Search:").find(|n| format!("{:?}", n.accesskit_node().role()) == "TextInput").unwrap().type_text(text);
    h.run_steps(4);
}

/// The table row named `name`.
fn row<'a>(h: &'a Harness<'static, Box<PhotocraftApp>>, name: &'a str) -> egui_kittest::Node<'a> {
    h.get_all_by_label_contains(name).find(|n| format!("{:?}", n.accesskit_node().role()) == "Row").expect("the row is listed")
}

fn keys_of(h: &Harness<'static, Box<PhotocraftApp>>, command: &CommandRef) -> usize {
    h.state().prefs_rt.shortcuts.as_ref().unwrap().keymap.bindings(command).len()
}

/// Right-click on a row, then Remove Shortcut: the row has no shortcut in the dialog's draft (OK would save that).
#[test]
fn right_click_remove_shortcut_clears_the_rows_keys() {
    let mut h = harness();
    search(&mut h, "Eyedropper Tool");
    let command = CommandRef::new("tools.select.eyedropper");
    assert!(keys_of(&h, &command) > 0, "the Eyedropper starts with a key");
    let rect = row(&h, "Eyedropper Tool").rect();
    // On the command's name, clear of the chips.
    let at = egui::pos2(rect.left() + 40.0, rect.center().y);
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        h.run_steps(2);
    }
    h.get_by_label("Remove Shortcut").click();
    h.run_steps(4);
    assert_eq!(keys_of(&h, &command), 0, "the key is gone");
}

/// A row in a clash names the command it clashes with on its mark.
#[test]
fn a_clash_mark_names_the_clashing_command() {
    let mut h = harness();
    crate::prefs_ui::open_shortcuts(h.state_mut(), 0);
    let keymap = &mut h.state_mut().prefs_rt.shortcuts.as_mut().unwrap().keymap;
    let key = Binding::parse("Cmd+Alt+Shift+J").unwrap();
    keymap.assign(&CommandRef::new("tools.select.eyedropper"), key.clone(), Policy::Keep).unwrap();
    keymap.assign(&CommandRef::new("file.open"), key, Policy::Keep).unwrap();
    search(&mut h, "Eyedropper Tool");
    let rect = row(&h, "Eyedropper Tool").rect();
    // The mark sits before the name.
    h.event(egui::Event::PointerMoved(egui::pos2(rect.left() + 12.0, rect.center().y)));
    h.run_steps(30);
    assert!(h.query_by_label_contains("Open…").is_some(), "the tooltip names the clashing command");
}

/// Search finds a binding by its modifier word, as the fork's old window did: File › New has only ⌘N, so "cmd" lists
/// it and "shift" and "opt" do not; nor does "ctrl" on a Mac (elsewhere ⌘ is Control, so "ctrl" lists it too).
#[test]
fn search_finds_bindings_by_modifier_words() {
    let lists = |word: &str, row: &str| {
        let mut h = harness();
        search(&mut h, word);
        h.query_by_label_contains(&format!("Add a shortcut to {row}")).is_some()
    };
    assert!(lists("cmd", "New…"));
    assert!(!lists("shift", "New…") && !lists("opt", "New…"));
    assert_eq!(lists("ctrl", "New…"), !cfg!(target_os = "macos"), "\"ctrl\" lists ⌘N only where ⌘ is Control");
}
