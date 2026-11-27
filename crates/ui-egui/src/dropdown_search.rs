//! Dropdown lists that narrow as you type. craft's `Select` shows a search field above its rows when it is
//! searchable; the techneissa/craft-libs fork lets the window say that every select with this many entries or more
//! is (fork/craft-libs.md, patch 8), so the blend mode lists, the Mode lists and the font and preset lists of the
//! tools narrow by typed letters, and the short ones stay plain.

use craft::ui::pickers::set_auto_search_from;

/// A select with this many entries or more shows its search field.
const SEARCH_FROM: usize = 8;

/// Per-frame upkeep (`app::framework`): tell the window.
pub(crate) fn sync(ctx: &egui::Context) {
    set_auto_search_from(ctx, Some(SEARCH_FROM));
}

#[cfg(test)]
mod tests;
