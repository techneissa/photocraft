//! Which bindings and chords work in a context: those whose When condition holds.

use egui::KeyboardShortcut;

use super::chord;
use super::when::Context;
use crate::PhotocraftApp;

/// Whether the keys of command `id` work in `cx`: it has no condition, or its condition holds.
/// A condition that can't be read is ignored, as if there were none.
pub fn holds(app: &PhotocraftApp, cx: &Context, id: &str) -> bool {
    photocraft_engine::prefs::when_of(&app.session.prefs().shortcut_when, id).is_none_or(|e| e.eval(cx))
}

/// The chords that work in `cx`: command id and its two steps, in a stable order.
pub fn chords_in(app: &PhotocraftApp, cx: &Context) -> Vec<(String, [KeyboardShortcut; 2])> {
    let prefs = app.session.prefs();
    prefs.chords.iter().filter(|(id, _)| holds(app, cx, id)).filter_map(|(id, c)| Some((id.clone(), chord::steps(c)?))).collect()
}

/// Whether tool `t` answers its own letter key now: it still has the default key and its When
/// condition (if any) holds. A tool with a key of its own is a binding like any command
/// ([`crate::shortcut_dispatch::bindings_in`]).
pub fn tool_on(app: &PhotocraftApp, ctx: &egui::Context, t: crate::state::Tool) -> bool {
    if !crate::tool_shortcuts::uses_default(app, t) {
        return false;
    }
    let when = &app.session.prefs().shortcut_when;
    if when.is_empty() {
        return true;
    }
    let id = crate::tool_shortcuts::id(t);
    !when.contains_key(&id) || holds(app, &super::context::read(app, crate::shortcut_dispatch::Focus::of(ctx)), &id)
}
