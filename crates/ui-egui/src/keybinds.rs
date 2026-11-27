//! Chords and When conditions for keyboard shortcuts (fork feature `feat/shortcut-upgrade`).
//!
//! - [`chord`]: a chord is two key steps (`Cmd+K C`): parse, spell, show.
//! - [`runtime`]: the fork's key layer in front of craft's keymap (`raw_input_hook` hands it the frame's key
//!   presses): the first step of a chord arms it, the next key finishes it (or Esc, a wrong key or the timeout
//!   cancels); a key of a command with a When condition runs it or runs nothing. Every other key goes on to craft.
//!   [`overlay`] shows what is pending as keycaps.
//! - [`context`]: the When context keys read from the app each frame; [`gate`] decides which
//!   bindings and chords work in it, and [`blocked`] lets the ones that ask for a dialog or a
//!   text box fire inside it.
//!
//! The model (conditions, conflicts, the JSON backup) lives in the engine
//! ([`photocraft_engine::when`], `Preferences::chords` / `shortcut_when`); the dialog parts are in
//! `prefs/prefs_ui/shortcuts_*.rs`.

pub mod blocked;
pub mod chord;
pub mod context;
pub mod focus;
pub mod gate;
pub mod overlay;
pub mod runtime;

pub use photocraft_engine::when;

/// What the shortcut runtime keeps between frames (a field of [`crate::PhotocraftApp`]).
#[derive(Default)]
pub struct State {
    /// The first step of a chord was pressed and the next key is awaited.
    pub pending: Option<runtime::Pending>,
    /// The notice about conditions that could not be read was shown.
    pub damage_noticed: bool,
    /// Commands the layer fired, waiting for `shortcuts::handle` to run them.
    pub fired: Vec<String>,
}

/// Where the Chords and Conditions tab says it is recording a chord (read by the key layer).
pub(crate) fn recording_id() -> egui::Id {
    egui::Id::unique("fork-chord-recording")
}

/// Where the key layer leaves the presses the recording takes: `Vec<(egui::Key, egui::Modifiers)>`.
pub(crate) fn captured_id() -> egui::Id {
    egui::Id::unique("fork-chord-captured")
}

/// Takes the keys of every command with a When condition out of craft's keymap `overrides`: the key layer
/// ([`runtime`]) runs such a command when its condition holds, and when it does not the key goes to whatever else has it.
pub(crate) fn unbind_gated(app: &crate::PhotocraftApp, overrides: &mut craft::keymap::Overrides) {
    use craft::keymap::CommandRef;
    for id in app.session.prefs().shortcut_when.keys() {
        // The saved id of a tool or a command, and the framework command that carries its keys.
        overrides.insert(id.as_str(), Vec::new());
        let command = if let Some(tool) = crate::tool_shortcuts::tool(id) {
            Some(CommandRef::from(crate::panels::tool_commands::command_of(tool)))
        } else {
            crate::shortcut_dispatch::RUNTIME_COMMANDS.iter().find(|(runtime_id, _)| runtime_id == id).map(|(_, bridged)| bridged.command())
        };
        if let Some(command) = command {
            overrides.insert(command.key(), Vec::new());
        }
    }
}

#[cfg(test)]
mod tests;
