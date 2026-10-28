//! Which items of the top bar are shown (Preferences › Interface › Top Bar, or a right-click on
//! the bar). The in-window menus and whatever the window needs to be moved or closed (its drag
//! area, the Windows and Linux caption buttons) are not items: they always stay.

use serde::{Deserialize, Serialize};

/// `interface.topBar`: one flag per item, every item shown by default (so settings files from
/// before it, and items added later, show everything).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TopBar {
    /// The PhotoCraft mark at the left of the menus.
    pub brand: bool,
    /// The document name (or "PhotoCraft") centred in the bar.
    pub title: bool,
    /// The workspace switcher dropdown.
    pub workspace: bool,
    /// The Search icon (Edit › Search).
    pub search: bool,
    /// The Appearance Mode icon (Preferences › Interface › Appearance Mode).
    pub appearance: bool,
}

impl Default for TopBar {
    fn default() -> Self {
        Self { brand: true, title: true, workspace: true, search: true, appearance: true }
    }
}

impl TopBar {
    /// The item ids, in the bar's order from the left; each is also its key under `interface.topBar`.
    pub const IDS: [&'static str; 5] = ["brand", "title", "workspace", "search", "appearance"];

    /// Whether the bar shows item `id`; `None` for an id that is not an item.
    pub fn shown(&self, id: &str) -> Option<bool> {
        Some(match id {
            "brand" => self.brand,
            "title" => self.title,
            "workspace" => self.workspace,
            "search" => self.search,
            "appearance" => self.appearance,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
