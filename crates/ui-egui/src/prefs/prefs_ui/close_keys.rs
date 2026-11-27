//! Closing Settings from the keyboard: Esc and ⌘, (the shortcut that opens it). With changes
//! Apply or OK would save, a Save / Don't Save / Cancel prompt asks first; without, the window
//! just closes. The Cancel button and the close box still discard.
//!
//! The window runs inside the dialog host and has no app, so the keys it listens for come from
//! `feed` ([`shortcuts_of`]) and the prompt's answer goes through the [`Bridge`](super::settings_sync::Bridge). The
//! framework turns Esc into the window's Cancel before the window sees a key, so `dialogs.rs` tells an Esc close from a
//! click on Cancel (no pointer press in that pass) and, with changes, keeps the window and asks.

use craft::ui::dialog::{PromptChoice, StandardPrompt};
use egui::KeyboardShortcut;

use super::settings_sync::{CloseAnswer, Shared, lock};
use crate::PhotocraftApp;

/// The keys that open a Settings page, which close the window while it is open.
pub(super) fn shortcuts_of(app: &PhotocraftApp) -> Vec<KeyboardShortcut> {
    let mut found = Vec::new();
    for (section, _) in photocraft_engine::prefs::SECTIONS {
        let command = format!("edit.preferences.{section}");
        let main = crate::shortcuts::effective_shortcut(app, &command, crate::shortcuts::default_shortcut(&command).as_deref());
        let keys: Vec<String> = main.into_iter().chain(crate::shortcuts::effective_alternates(app, &command)).collect();
        for shortcut in keys.iter().filter_map(|s| crate::shortcuts::parse(s)) {
            if !found.contains(&shortcut) {
                found.push(shortcut);
            }
        }
    }
    found
}

/// ⌘, (or another key of `keys`) pressed: takes the key.
pub(super) fn close_pressed(ctx: &egui::Context, keys: &[KeyboardShortcut]) -> bool {
    keys.iter().any(|shortcut| ctx.input_mut(|i| i.consume_shortcut(shortcut)))
}

/// The prompt that asks whether to save before closing.
fn prompt() -> StandardPrompt {
    StandardPrompt::custom(tl!("Save your changes to Preferences before closing?"))
        .title(tl!("Unsaved changes"))
        .instance("settings-close")
        .button(PromptChoice::Proceed, tl!("Save"))
        .button(PromptChoice::Alternate, tl!("Don't Save"))
        .button(PromptChoice::Cancel, tl!("Cancel"))
}

/// Draws the prompt while it is up and files the answer (the window's pass acts on it).
pub(super) fn show_prompt(ui: &mut egui::Ui, bridge: &Shared) {
    if !lock(bridge).close_asking {
        return;
    }
    let Some(answer) = prompt().show(ui).answer else { return };
    let mut bridge = lock(bridge);
    bridge.close_asking = false;
    bridge.close_answer = match answer.choice {
        PromptChoice::Proceed => Some(CloseAnswer::Save),
        PromptChoice::Alternate => Some(CloseAnswer::Discard),
        _ => None,
    };
}
