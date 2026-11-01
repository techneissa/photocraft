//! The size of Edit › Keyboard Shortcuts and Menus: one height for its three tabs, resizable, the
//! size the user gives it kept. The lists fill what the window leaves them (like Preferences), so
//! a tab never makes the window taller or shorter than another.

use serde_json::{Map, Value};

/// The size the window opens at: wide enough for the Toolbar tab's four columns, and tall enough
/// for its 15 rows without scrolling (the tools in four columns). A smaller screen gets the
/// largest the window may be.
pub const SIZE: egui::Vec2 = egui::vec2(1000.0, 640.0);

/// The smallest the window is dragged to: the command, key, further keys and When columns still fit.
pub const MIN: egui::Vec2 = egui::vec2(820.0, 420.0);

/// Is this dialog Keyboard Shortcuts and Menus?
pub fn is_window(fields: &Map<String, Value>) -> bool {
    fields.get("__prefsui").and_then(Value::as_str) == Some("shortcuts")
}

/// Where the size the user gave the window is kept.
pub fn size_id() -> egui::Id {
    egui::Id::unique("shortcuts-window-size")
}

/// How tall what sits under a list of `slot` is: what it needed the frame before (a guess taller
/// than it will be until it has been drawn once).
fn footer_height(ctx: &egui::Context, slot: &str) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(footer_id(slot))).unwrap_or(110.0)
}

/// The height a list of `slot` gets: what the window has left, less the dialog's own button row
/// and the footer under the list ([`footer`]). The window (an egui `Resize`) grows to hold
/// whatever overflows and never shrinks again, so the sum is kept at what is left, never over it.
/// A list that may scroll sideways (`wide`) keeps room for its scroll bar, which egui adds below it
/// (all three tabs keep it, so they are one height).
pub(super) fn list_height(ui: &egui::Ui, slot: &str, wide: bool) -> f32 {
    let spacing = ui.spacing().item_spacing.y;
    let left = super::dialogs::settings_height(ui.available_height(), spacing, crate::widgets::button_height(ui.ctx()));
    let bar = if wide { ui.spacing().scroll.allocated_width() } else { 0.0 };
    (left - footer_height(ui.ctx(), slot) - spacing - bar).max(120.0)
}

/// What sits under the list (buttons, the condition box, messages): drawn in a space of the
/// height it needed last frame, and the window is told only that height, so a footer that grows
/// for a frame (a message appears) can't make the window taller for good.
pub(super) fn footer<R>(ui: &mut egui::Ui, slot: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let reserved = footer_height(ui.ctx(), slot);
    let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), reserved));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    let out = add(&mut child);
    let needed = child.min_rect().height();
    let id = footer_id(slot);
    if ui.ctx().data(|d| d.get_temp::<f32>(id)).is_none_or(|before| (before - needed).abs() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(id, needed));
        ui.ctx().request_repaint();
    }
    ui.allocate_rect(rect, egui::Sense::hover());
    out
}

fn footer_id(slot: &str) -> egui::Id {
    egui::Id::unique(("shortcuts-footer", slot))
}

/// Menu items that exist only while something is open or saved: the recent files, the open
/// documents and the saved workspaces. They are not commands to give a key or hide.
pub fn is_dynamic_item(id: &str) -> bool {
    let numbered = |prefix: &str| id.strip_prefix(prefix).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    numbered("file.openRecent.") || id.starts_with(crate::menus::DOCUMENT_ITEM) || id.starts_with(crate::workspace_ui::SAVED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_per_file_and_per_document_items_are_dynamic() {
        for id in ["file.openRecent.0", "file.openRecent.12", "window.document.3", "window.workspace.saved.0"] {
            assert!(is_dynamic_item(id), "{id}");
        }
        for id in ["file.clearRecent", "file.open", "window.workspace.motion", "window.nextDocument", "file.openRecent."] {
            assert!(!is_dynamic_item(id), "{id}");
        }
    }
}
