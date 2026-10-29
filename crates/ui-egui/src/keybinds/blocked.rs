//! Shortcuts that work while a dialog, the Command Palette or a text box owns the keyboard. They
//! are the ones whose condition needs it: it reads `textFocus`, `widgetFocus`, `dialogOpen` or
//! `paletteOpen` and would not hold without it (`dialogOpen` itself, not `!dialogOpen || tool == brush`).

use egui::KeyboardShortcut;

use super::when::keys::{DIALOG_OPEN, PALETTE_OPEN, TEXT_FOCUS, WIDGET_FOCUS};
use super::when::{Context, Expr};
use crate::PhotocraftApp;
use crate::shortcut_dispatch::Focus;
use crate::shortcuts::key_matches;

const BLOCKERS: [usize; 4] = [TEXT_FOCUS, WIDGET_FOCUS, DIALOG_OPEN, PALETTE_OPEN];

/// Whether `when` holds in `cx` only because the keyboard is blocked: it holds now and not with
/// the blockers off.
fn asks(when: &Expr, cx: &Context) -> bool {
    let mut calm = *cx;
    BLOCKERS.iter().for_each(|k| calm.set_flag(*k, false));
    BLOCKERS.iter().any(|k| when.mentions(*k)) && when.eval(cx) && !when.eval(&calm)
}

/// Whether command `id` asks to work with the keyboard blocked, in `cx`.
pub fn wants(app: &PhotocraftApp, cx: &Context, id: &str) -> bool {
    photocraft_engine::prefs::when_of(&app.session.prefs().shortcut_when, id).is_some_and(|e| asks(&e, cx))
}

/// Run the command whose key was pressed this frame and whose condition asks for a blocked
/// keyboard (a dialog is open, a text box has the keys…). Returns true when one ran.
pub fn fire(app: &mut PhotocraftApp, ctx: &egui::Context, focus: Focus) -> bool {
    if app.session.prefs().shortcut_when.is_empty() || !ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))) {
        return false;
    }
    let cx = super::context::read(app, focus);
    let table: Vec<(String, KeyboardShortcut)> =
        crate::shortcut_dispatch::bindings_in(app, Some(&cx)).into_iter().filter(|(id, _)| wants(app, &cx, id)).collect();
    let hit = ctx.input_mut(|i| {
        let at = i.events.iter().position(
            |e| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if table.iter().any(|(_, sc)| key_matches(sc, *key, *modifiers))),
        )?;
        match i.events.remove(at) {
            egui::Event::Key { key, modifiers, .. } => table.iter().find(|(_, sc)| key_matches(sc, key, modifiers)).map(|(id, _)| id.clone()),
            _ => None,
        }
    });
    match hit {
        Some(id) => {
            crate::shortcut_dispatch::dispatch(app, ctx, &id);
            true
        }
        None => false,
    }
}
