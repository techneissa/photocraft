//! Placing photos on an open picture, the quick way: the Layers panel footer has a Place Photo
//! button that runs File › Place Embedded… (⇧⌘P). Place Embedded itself takes several photos at
//! once in the official code (`file_dialog::pick_files_bytes`, queued by `place_picked`).

use crate::PhotocraftApp;

/// The command the footer button and the shortcut run.
pub(crate) const COMMAND: &str = "file.placeEmbedded";
/// The icon of the footer button (`assets/icons/place-photo.svg`).
const ICON: &str = "place-photo";

/// The Layers footer's Place Photo button: its icon, its tooltip (with the key in effect) and whether it is
/// enabled, greyed as the menu item is. A press runs Place Embedded with no params, which opens the picker.
pub(crate) fn footer_button(app: &PhotocraftApp) -> (&'static str, String, bool) {
    (ICON, crate::shortcuts::tip_label(app, "Place Photo", COMMAND), crate::menus::is_enabled(app, COMMAND))
}
