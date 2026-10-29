//! Closing Preferences from the keyboard: Esc and ⌘, (the shortcut that opens it). With changes
//! Apply or OK would save, a Save / Don't Save / Cancel prompt asks first; without, the dialog
//! just closes.

use super::{Map, PhotocraftApp, Value, json};
use crate::widgets::{ButtonRole, DialogButton};

/// The Preferences dialog's field that is set while the prompt is up.
const PROMPT: &str = "__closePrompt";

/// The open Preferences dialog, if any.
pub(super) fn open_dialog(app: &PhotocraftApp) -> Option<u64> {
    app.ui.dialogs.iter().find(|d| super::is_preferences(&d.fields)).map(|d| d.id)
}

/// Esc or ⌘, on Preferences: close it, or ask first when it holds unsaved changes.
pub fn request_close(app: &mut PhotocraftApp, id: u64) {
    let Some(fields) = app.ui.dialogs.iter().find(|d| d.id == id).map(|d| d.fields.clone()) else { return };
    if super::preferences_changed(app, &fields) {
        if let Some(d) = app.ui.dialog_mut(id) {
            d.fields.insert(PROMPT.into(), json!(true));
        }
    } else {
        let _ = crate::dialogs::cancel(app, id);
    }
}

/// Is the close prompt up for these fields?
pub fn prompting(fields: &Map<String, Value>) -> bool {
    fields.get(PROMPT).and_then(Value::as_bool) == Some(true)
}

/// Whether a text field had keyboard focus at the end of the last frame. egui lets go of the focus
/// when Esc arrives, before any widget looks, so the frame's own answer would always be "no".
fn typing_id() -> egui::Id {
    egui::Id::new("prefs-close-keys-typing")
}

/// Call once at the end of a frame of dialogs, for [`esc_closes`].
pub fn remember_typing(ctx: &egui::Context) {
    let typing = ctx.egui_wants_keyboard_input();
    ctx.data_mut(|d| d.insert_temp(typing_id(), typing));
}

/// Whether Esc may close the dialog: not while a text field is being edited, a dropdown is open or
/// a shortcut is being captured, where Esc has its own job.
pub fn esc_closes(ctx: &egui::Context, fields: &Map<String, Value>) -> bool {
    let typing = ctx.data(|d| d.get_temp::<bool>(typing_id())).unwrap_or(false);
    !typing && !egui::Popup::is_any_open(ctx) && fields.get("capture").and_then(Value::as_bool) != Some(true)
}

enum Answer {
    Save,
    Discard,
    Cancel,
}

/// Draw the prompt for dialog `id` and act on the answer.
pub fn show_prompt(app: &mut PhotocraftApp, ctx: &egui::Context, id: u64) {
    let buttons = [
        (ButtonRole::Default, tl!("Save"), 84.0, Answer::Save),
        (ButtonRole::Alternate, tl!("Don't Save"), 100.0, Answer::Discard),
        (ButtonRole::Cancel, tl!("Cancel"), 84.0, Answer::Cancel),
    ];
    let row: Vec<DialogButton> = buttons.iter().map(|b| DialogButton::new(b.0, b.1, b.2).focus(b.0 == ButtonRole::Default)).collect();
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::new(("prefs-close-prompt", id))).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.label(egui::RichText::new(tl!("Unsaved changes")).font(crate::theme::semibold(15.0)));
        ui.add_space(4.0);
        crate::widgets::hairline(ui);
        ui.add_space(8.0);
        ui.label(tl!("Save your changes to Preferences before closing?"));
        ui.add_space(12.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if let Some(role) = crate::widgets::dialog_buttons(ui, &row) {
                answer = buttons.iter().find(|b| b.0 == role).map(|b| &b.3);
            }
        });
    });
    // Esc cancels; Enter takes the default answer.
    let answer = match answer {
        Some(Answer::Save) => Answer::Save,
        Some(Answer::Discard) => Answer::Discard,
        Some(Answer::Cancel) => Answer::Cancel,
        None if modal.should_close() => Answer::Cancel,
        None if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) => Answer::Save,
        None => return,
    };
    match answer {
        Answer::Save => {
            if let Err(error) = crate::dialogs::confirm(app, id) {
                app.ui.status = error;
                clear(app, id);
            }
        }
        Answer::Discard => {
            let _ = crate::dialogs::cancel(app, id);
        }
        Answer::Cancel => clear(app, id),
    }
}

fn clear(app: &mut PhotocraftApp, id: u64) {
    if let Some(d) = app.ui.dialog_mut(id) {
        d.fields.remove(PROMPT);
    }
}

/// ⌘, (any Preferences page's shortcut) pressed while Preferences is the dialog on top: close it,
/// asking first when it holds unsaved changes. True when the key was taken.
pub fn close_shortcut(app: &mut PhotocraftApp, ctx: &egui::Context) -> bool {
    let Some(top) = app.ui.dialogs.last() else { return false };
    if !super::is_preferences(&top.fields) || prompting(&top.fields) {
        return false;
    }
    let id = top.id;
    let pressed = super::SECTIONS.iter().any(|(section, _)| {
        let command = format!("edit.preferences.{section}");
        let main = crate::shortcuts::effective_shortcut(app, &command, crate::shortcuts::default_shortcut(&command).as_deref());
        let keys: Vec<String> = main.into_iter().chain(crate::shortcuts::effective_alternates(app, &command)).collect();
        keys.iter().filter_map(|s| crate::shortcuts::parse(s)).any(|sc| crate::shortcuts::consume(ctx, &sc))
    });
    if pressed {
        request_close(app, id);
    }
    pressed
}
