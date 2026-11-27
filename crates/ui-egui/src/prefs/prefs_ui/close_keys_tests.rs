//! Esc and ⌘, close Settings; with unsaved changes a Save / Don't Save / Cancel prompt asks first
//! (fork/settings-close-keys.md).

use craft::ui::patterns::preferences::PrefValue;
use egui_kittest::Harness;

use super::settings_sync::{self, CloseAnswer};
use super::*;
use crate::PhotocraftApp;

fn harness(change: bool) -> Harness<'static, Box<PhotocraftApp>> {
    let files = std::sync::Arc::new(craft::settings::testing::MemoryFiles::new());
    let mut h = super::tests::prefs_harness(&files, "general", egui::vec2(1280.0, 860.0), |_| {});
    if change {
        super::tests::edit(h.state_mut(), "export.jpegQuality", PrefValue::Int(44));
        h.run_steps(2);
    }
    h
}

fn asking(h: &Harness<'static, Box<PhotocraftApp>>) -> bool {
    settings_sync::lock(&h.state().prefs_rt.bridge).close_asking
}

#[test]
fn esc_closes_settings_without_changes() {
    let mut h = harness(false);
    h.key_press(egui::Key::Escape);
    h.run_steps(6);
    assert!(!window_open(h.state()));
}

#[test]
fn esc_with_changes_asks_and_the_answers_save_discard_or_keep() {
    let mut h = harness(true);
    h.key_press(egui::Key::Escape);
    h.run_steps(6);
    assert!(window_open(h.state()), "it asks first");
    assert!(asking(&h));
    // Cancel keeps the window and the changes.
    {
        let mut bridge = settings_sync::lock(&h.state().prefs_rt.bridge);
        bridge.close_asking = false;
    }
    h.run_steps(4);
    assert!(window_open(h.state()));
    // Don't Save closes and drops them.
    settings_sync::lock(&h.state().prefs_rt.bridge).close_answer = Some(CloseAnswer::Discard);
    h.run_steps(6);
    assert!(!window_open(h.state()));
    assert_eq!(h.state().session.prefs().export.jpeg_quality, 85);
}

#[test]
fn saving_from_the_prompt_writes_the_changes_like_ok_and_closes() {
    let mut h = harness(true);
    settings_sync::lock(&h.state().prefs_rt.bridge).close_answer = Some(CloseAnswer::Save);
    h.run_steps(6);
    assert!(!window_open(h.state()));
    assert_eq!(h.state().session.prefs().export.jpeg_quality, 44);
}

#[test]
fn the_keys_that_open_settings_close_it() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    let keys = super::close_keys::shortcuts_of(&app);
    let comma = crate::shortcuts::parse("Cmd+,").unwrap();
    assert!(keys.contains(&comma), "⌘, is among the keys of Edit › Preferences");
}

#[test]
fn cmd_comma_closes_settings_or_asks_when_it_holds_changes() {
    let mut h = harness(false);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(6);
    assert!(!window_open(h.state()));
    let mut h = harness(true);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(6);
    assert!(window_open(h.state()) && asking(&h));
}
