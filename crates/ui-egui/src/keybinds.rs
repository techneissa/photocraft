//! Chords and When conditions for keyboard shortcuts (fork feature `feat/shortcut-upgrade`).
//!
//! - [`chord`]: a chord is two key steps (`Cmd+K C`): parse, spell, show.
//! - [`runtime`]: the leader key: the first step of a chord arms it, the next key finishes it (or
//!   Esc, a wrong key or the timeout cancels). [`overlay`] shows what is pending as keycaps.
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
}

#[cfg(test)]
mod tests;
