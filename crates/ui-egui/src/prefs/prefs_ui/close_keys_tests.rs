//! Esc and ⌘, close Preferences; with unsaved changes a Save / Don't Save / Cancel prompt asks first.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::*;
use crate::PhotocraftApp;

fn harness(change: bool) -> (Harness<'static, PhotocraftApp>, u64) {
    let (mut app, _) = super::tests::app_with_store();
    app.run("prefs.set", json!({"values": {"interface.language": "en"}})).unwrap();
    let id = open_preferences(&mut app, "general");
    if change {
        app.ui.dialog_mut(id).unwrap().fields.get_mut("values").unwrap()["cursors"]["painting"] = json!("fullSizeTip");
    }
    let mut h = Harness::builder().with_size(egui::vec2(1280.0, 860.0)).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        app
    });
    h.run_steps(4);
    (h, id)
}

fn open(h: &Harness<'static, PhotocraftApp>) -> bool {
    h.state().ui.dialogs.iter().any(|d| is_preferences(&d.fields))
}

fn esc(h: &mut Harness<'static, PhotocraftApp>) {
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
}

fn comma(h: &mut Harness<'static, PhotocraftApp>) {
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(4);
}

fn painting(h: &Harness<'static, PhotocraftApp>) -> prefs::PaintingCursor {
    h.state().session.prefs().cursors.painting
}

#[test]
fn esc_closes_preferences_without_changes() {
    let (mut h, _) = harness(false);
    esc(&mut h);
    assert!(!open(&h));
    assert!(h.query_by_label("Unsaved changes").is_none());
}

#[test]
fn cmd_comma_closes_preferences_without_changes() {
    let (mut h, _) = harness(false);
    comma(&mut h);
    assert!(!open(&h), "⌘, closes it, and opens no second one");
}

#[test]
fn esc_with_changes_asks_and_cancel_keeps_them() {
    let (mut h, id) = harness(true);
    esc(&mut h);
    assert!(open(&h) && h.query_by_label("Unsaved changes").is_some());
    h.get_all_by_label("Cancel").last().unwrap().click();
    h.run_steps(4);
    assert!(h.query_by_label("Unsaved changes").is_none());
    assert!(open(&h));
    let d = h.state().ui.dialogs.iter().find(|d| d.id == id).unwrap();
    assert_eq!(d.fields["values"]["cursors"]["painting"], "fullSizeTip", "the changes are kept");
    assert_ne!(painting(&h), prefs::PaintingCursor::FullSizeTip, "and not saved");
}

#[test]
fn esc_in_the_prompt_is_cancel() {
    let (mut h, _) = harness(true);
    comma(&mut h);
    assert!(h.query_by_label("Unsaved changes").is_some());
    esc(&mut h);
    assert!(open(&h) && h.query_by_label("Unsaved changes").is_none());
}

#[test]
fn save_in_the_prompt_saves_as_ok_does_and_closes() {
    let (mut h, _) = harness(true);
    comma(&mut h);
    h.get_all_by_label("Save").last().unwrap().click();
    h.run_steps(4);
    assert!(!open(&h));
    assert_eq!(painting(&h), prefs::PaintingCursor::FullSizeTip);
}

#[test]
fn dont_save_in_the_prompt_closes_and_drops_the_changes() {
    let (mut h, _) = harness(true);
    esc(&mut h);
    h.get_all_by_label("Don't Save").last().unwrap().click();
    h.run_steps(4);
    assert!(!open(&h));
    assert_ne!(painting(&h), prefs::PaintingCursor::FullSizeTip);
}

#[test]
fn esc_in_a_text_field_does_not_close_preferences() {
    let (mut h, _) = harness(false);
    h.get_all_by_role(egui::accesskit::Role::TextInput).last().unwrap().click();
    h.run_steps(3);
    assert!(h.ctx.egui_wants_keyboard_input(), "the search field has focus");
    esc(&mut h);
    assert!(open(&h), "Esc belongs to the focused text field");
}

#[test]
fn esc_closes_an_open_dropdown_and_not_preferences() {
    let (mut app, _) = super::tests::app_with_store();
    open_preferences(&mut app, "tools");
    let mut h = Harness::builder().with_size(egui::vec2(1280.0, 860.0)).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        app
    });
    h.run_steps(4);
    let combo = h.get_all_by_role(egui::accesskit::Role::ComboBox).find(|n| n.rect().top() > 100.0 && n.rect().left() > 250.0).expect("a dropdown");
    combo.click();
    h.run_steps(3);
    assert!(egui::Popup::is_any_open(&h.ctx) || crate::widgets::dropdown_list_open(&h.ctx), "the list is open");
    esc(&mut h);
    assert!(open(&h), "the first Esc closes the dropdown only");
    esc(&mut h);
    assert!(!open(&h), "the next one closes Preferences");
}
