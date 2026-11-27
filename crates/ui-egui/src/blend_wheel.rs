//! Interface › "Scroll wheel changes blend mode": off by default, so a wheel turned over the Layers
//! panel (or the options bar) to scroll it never changes a blend mode by accident. On, the wheel
//! over a blend mode box steps the mode, one notch one step, as in Photoshop (#1747, #2061).
//!
//! The boxes are craft's `Select` over `blend_entries`; the techneissa/craft-libs fork leaves the
//! wheel to the panel under it unless the window says otherwise (fork/craft-libs.md, patch 2). This
//! file only tells the window each pass what the preference says.

use craft::ui::pickers::set_wheel_steps_blend_modes;
use photocraft_engine::Session;

/// Per-frame upkeep (`app::framework`): copy the preference into the window.
pub(crate) fn sync(session: &Session, ctx: &egui::Context) {
    set_wheel_steps_blend_modes(ctx, session.prefs().interface.scroll_wheel_changes_blend_mode);
}

#[cfg(test)]
mod tests;
