//! The top bar's items, which a person shows or hides (Preferences › Interface › Top Bar, or a
//! right-click on an empty part of the bar): the logo, the document title, the workspace
//! switcher, the Search icon and the Appearance Mode icon ([`ITEMS`]).
//!
//! The in-window menus and what the window needs to be moved, maximized or closed (the bar's drag
//! area, the caption buttons of Windows and Linux) are not items: they always stay, so a bar with
//! every item hidden still moves the window and still has this menu.
//!
//! What is saved is `interface.topBar` (engine `prefs::TopBar`, every item shown by default), read
//! and set with `prefs.get` and `prefs.set` like any preference. A hidden item only leaves the
//! bar: Edit › Search, Window › Workspace and Preferences › Interface still do what it did.

use egui::{Popup, PopupCloseBehavior};
use photocraft_engine::prefs::TopBar;
use serde_json::{Map, Value, json};

use super::{PhotocraftApp, widgets};

#[cfg(test)]
mod tests;

/// One item of the bar a person can hide.
pub struct Item {
    /// The item's key under `interface.topBar`.
    pub id: &'static str,
    pub label: &'static str,
}

/// The items in the order of the bar, from the left.
pub const ITEMS: [Item; 5] = [
    Item { id: "brand", label: "Logo" },
    Item { id: "title", label: "Document Title" },
    Item { id: "workspace", label: "Workspace" },
    Item { id: "search", label: "Search" },
    Item { id: "appearance", label: "Appearance Mode" },
];

/// What the bar's far-end controls draw in `room` points, given the items that are shown.
#[derive(Debug, PartialEq)]
pub struct Fit {
    /// The workspace switcher, and its width.
    pub workspace: Option<f32>,
    pub search: bool,
    pub appearance: bool,
}

impl Fit {
    /// Whether the controls draw anything: with nothing drawn the bar's whole gap is free.
    pub fn any(&self) -> bool {
        self.workspace.is_some() || self.search || self.appearance
    }
}

/// A narrow bar drops what is also in a menu, the Appearance icon first (Preferences), then
/// Search (Edit › Search), and narrows the switcher (Window › Workspace) before anything runs
/// over. An item hidden by choice gives its room to the ones left: they drop later. With every
/// item shown this is the bar as it always was. `workspace_min` is the switcher's narrowest
/// width and `slot` one icon with its spacing.
pub fn fit(room: f32, bar: &TopBar, workspace_min: f32, slot: f32) -> Fit {
    let reserved = if bar.workspace { workspace_min } else { 0.0 };
    let wanted = usize::from(bar.search) + usize::from(bar.appearance);
    let icons = (0..=wanted).rev().find(|n| room >= reserved + *n as f32 * slot).unwrap_or(0);
    let workspace = (bar.workspace && room >= workspace_min).then(|| (room - icons as f32 * slot - 6.0).clamp(workspace_min, 130.0));
    Fit { workspace, search: bar.search && icons >= 1, appearance: bar.appearance && icons > usize::from(bar.search) }
}

/// The items now, as the bar draws them.
pub fn current(app: &PhotocraftApp) -> TopBar {
    app.session.prefs().interface.top_bar.clone()
}

/// Show or hide one item; the error is for the status bar.
pub fn set_shown(app: &mut PhotocraftApp, id: &str, on: bool) -> Result<(), String> {
    app.run("prefs.set", json!({"path": format!("interface.topBar.{id}"), "value": on})).map(|_| ())
}

/// Whether a drag that just started on the bar's gap moves the window. With the items menu open it
/// only closes the menu: the window would move while the menu stays at the old place until the
/// mouse is released.
pub fn drag_moves_window(ctx: &egui::Context, gap: &egui::Response) -> bool {
    let menu = Popup::context_menu(gap);
    if !menu.is_open() {
        return true;
    }
    Popup::close_id(ctx, menu.get_id());
    false
}

/// A right-click on an empty part of the bar: every item with a check box, then the way to the
/// Preferences page. The menu stays open while items are turned on and off.
pub fn bar_menu(app: &mut PhotocraftApp, response: &egui::Response) {
    Popup::context_menu(response).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let bar = current(app);
        for item in &ITEMS {
            let mut on = bar.shown(item.id).unwrap_or(true);
            if widgets::checkbox(ui, &mut on, item.label).changed()
                && let Err(e) = set_shown(app, item.id, on)
            {
                app.ui.status = e;
            }
        }
        ui.separator();
        if ui.button(tl!("Top Bar Settings…")).on_hover_text(tl!("Menus and window buttons always stay.")).clicked() {
            crate::prefs_ui::open_preferences(app, "interface");
            ui.close();
        }
    });
}

/// Whether a Preferences search (`query`, lowercased; empty matches everything) shows the group.
pub fn group_matches(query: &str) -> bool {
    query.is_empty()
        || "top bar".contains(query)
        || tl!("Top Bar").to_lowercase().contains(query)
        || ITEMS.iter().any(|i| tl!(i.label).to_lowercase().contains(query))
}

/// The Top Bar group of Preferences › Interface: a check box per item, on the dialog's working
/// copy of the section (`obj`), which Preferences saves with OK like the rest.
pub fn editor(ui: &mut egui::Ui, obj: &mut Map<String, Value>) {
    let mut bar: TopBar = obj.get("topBar").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    ui.spacing_mut().item_spacing.y = 7.0;
    for item in &ITEMS {
        let mut on = bar.shown(item.id).unwrap_or(true);
        widgets::checkbox(ui, &mut on, item.label);
        bar.set(item.id, on);
    }
    obj.insert("topBar".into(), json!(bar));
}
