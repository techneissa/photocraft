//! The Chords and Conditions tab: OK keeps and writes the chords and conditions (the reset in the save would clear
//! them), a chord is two presses, and the recording takes the presses from the key layer.

use egui::{Event, Key, Modifiers};

use super::shortcuts::press_ok;
use super::shortcuts_chords::ChordDraft;
use crate::PhotocraftApp;

fn app() -> Box<PhotocraftApp> {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    PhotocraftApp::setup_context(app.ctx(), Default::default());
    app
}

/// OK of the dialog with no edit to the tab leaves the saved chords and conditions alone (the save resets the keys,
/// and a reset clears them); an edit in the tab is what OK writes.
#[test]
fn ok_keeps_the_chords_and_conditions_and_writes_the_tabs_edits() {
    let mut app = app();
    let ctx = app.ctx().clone();
    app.session.edit_prefs(|p| {
        p.chords.insert("tools.select.brush".into(), "Cmd+K B".into());
        p.shortcut_when.insert("tools.select.brush".into(), "hasDocument".into());
    });
    crate::prefs_ui::open_shortcuts(&mut app, 0);
    press_ok(&mut app, &ctx);
    assert_eq!(app.session.prefs().chords.get("tools.select.brush").map(String::as_str), Some("Cmd+K B"));
    assert_eq!(app.session.prefs().shortcut_when.get("tools.select.brush").map(String::as_str), Some("hasDocument"));

    crate::prefs_ui::open_shortcuts(&mut app, 0);
    let draft = &mut app.prefs_rt.shortcuts.as_mut().unwrap().chords;
    draft.chords.insert("tools.select.eraser".into(), "Cmd+K E".into());
    draft.whens.remove("tools.select.brush");
    press_ok(&mut app, &ctx);
    assert_eq!(app.session.prefs().chords.get("tools.select.eraser").map(String::as_str), Some("Cmd+K E"));
    assert!(app.session.prefs().shortcut_when.is_empty(), "the removed condition is gone");
}

/// A chord is the two presses in a row; Esc cancels, ⌫ clears.
#[test]
fn a_chord_is_recorded_from_two_presses() {
    let mut draft = ChordDraft { recording: Some(("tools.select.brush".into(), Vec::new())), ..ChordDraft::default() };
    draft.feed(vec![(Key::K, Modifiers::COMMAND), (Key::B, Modifiers::NONE)]);
    assert_eq!(draft.chords.get("tools.select.brush").map(String::as_str), Some("Cmd+K B"));
    assert!(draft.recording.is_none());
    draft.recording = Some(("tools.select.brush".into(), Vec::new()));
    draft.feed(vec![(Key::Escape, Modifiers::NONE)]);
    assert!(draft.chords.contains_key("tools.select.brush") && draft.recording.is_none(), "Esc leaves it as it was");
    draft.recording = Some(("tools.select.brush".into(), Vec::new()));
    draft.feed(vec![(Key::Backspace, Modifiers::NONE)]);
    assert!(draft.chords.is_empty());
}

/// While the tab records, the key layer takes the presses for it and craft's keys run nothing.
#[test]
fn the_key_layer_hands_a_recording_the_presses() {
    let mut app = app();
    let ctx = app.ctx().clone();
    crate::prefs_ui::open_shortcuts(&mut app, 0);
    ctx.data_mut(|d| d.insert_temp(crate::keybinds::recording_id(), true));
    let mut raw = egui::RawInput {
        events: vec![Event::Key { key: Key::B, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }],
        ..Default::default()
    };
    crate::keybinds::runtime::filter(&mut app, &ctx, &mut raw);
    assert!(raw.events.is_empty(), "nothing else sees the press");
    let taken: Vec<(Key, Modifiers)> = ctx.data_mut(|d| d.remove_temp(crate::keybinds::captured_id())).unwrap_or_default();
    assert_eq!(taken, vec![(Key::B, Modifiers::NONE)]);
}

/// Assign Shortcut… opens the dialog on the command.
#[test]
fn open_shortcuts_for_selects_the_command() {
    let mut app = app();
    assert!(crate::prefs_ui::open_shortcuts_for(&mut app, "edit.copy"));
    assert!(crate::prefs_ui::shortcuts_open(&app));
    assert!(!crate::prefs_ui::open_shortcuts_for(&mut app, "no.such.command"));
}
