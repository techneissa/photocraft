//! Shortcuts that work while a dialog, the Command Palette or a text box owns the keyboard ([`super::runtime`]
//! lets them through). They are the ones whose condition needs it: it reads `textFocus`, `widgetFocus`, `dialogOpen` or
//! `paletteOpen` and would not hold without it (`dialogOpen` itself, not `!dialogOpen || tool == brush`).

use super::when::keys::{DIALOG_OPEN, PALETTE_OPEN, TEXT_FOCUS, WIDGET_FOCUS};
use super::when::{Context, Expr};
use crate::PhotocraftApp;

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
