//! Interface › "Scroll wheel changes blend mode": off by default, so a wheel turned over the Layers
//! panel (or the options bar) to scroll it never changes a blend mode by accident. On, the wheel
//! over a blend mode box steps the mode, one notch one step, as in Photoshop (#1747, #2061).

use egui::{Context, Id, Ui};

use crate::PhotocraftApp;
use crate::widgets;

/// The preference, mirrored into the context each frame so the dropdowns, which only have a `Ui`,
/// can read it.
fn flag() -> Id {
    Id::unique("blend-wheel-pref")
}

/// Per-frame upkeep (`prefs_ui::save::tick`): copy the preference into the context.
pub(crate) fn sync(app: &PhotocraftApp, ctx: &Context) {
    let on = app.session.prefs().interface.scroll_wheel_changes_blend_mode;
    ctx.data_mut(|d| d.insert_temp(flag(), on));
}

fn enabled(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(flag())).unwrap_or(false)
}

/// A blend mode dropdown: [`widgets::dropdown_wheel_hovered`] when the preference is on, else the
/// same dropdown that the wheel doesn't touch. Returns the options chosen this frame in order
/// (a click or arrow key gives one; each wheel notch one more) and the option under the pointer.
pub(crate) fn dropdown<T: PartialEq + Clone>(ui: &mut Ui, id: &str, name: &str, current: &mut T, options: &[(T, &str)], width: f32) -> (Vec<T>, Option<T>) {
    if enabled(ui.ctx()) {
        return widgets::dropdown_wheel_hovered(ui, id, name, current, options, width);
    }
    let (changed, hovered) = widgets::dropdown_hovered(ui, id, name, current, options, width);
    (if changed { vec![current.clone()] } else { Vec::new() }, hovered)
}

#[cfg(test)]
mod tests;
