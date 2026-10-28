//! Right-click menus the user arranges (Preferences › Context Menus): rows hidden, put in another
//! order, separators placed, and any command of the menu bar added.
//!
//! The arrangement lives in `Preferences::context_menus` (JSON owned by this module, read
//! leniently): per menu key ([`menus`]) the rows shown in order (`cmd:<command id>` or `---`) and the
//! built-in rows taken out. Null means no menu is arranged, so every menu is exactly the one the
//! code builds and a new row upstream adds appears at once. [`apply`] lays an arrangement over the
//! rows a menu builds now, so what depends on state stays intact: the layer kind's rows, the crop and
//! spelling rows, a command that is greyed. A saved id this build does not know is skipped.
//!
//! Hooks: `canvas_tool_menu::show` / `available` ([`canvas_rows`], [`canvas_has`]),
//! `layer_menu_ui::show` ([`layer_rows`]) and the document tab menu ([`tab_rows`]). Three presets
//! keep whole sets of arrangements ([`presets`]), and a `.json` file backs them up ([`backup`]).
//! Agents read and set the arrangement with `prefs.get` / `prefs.set` on `contextMenus`.
//!
//! Not arranged yet (they keep their fixed rows): the Pen and path menu, the Zoom / Hand view menu,
//! the Free Transform menu, the Type menus, the pasteboard, guide and crop menus, the Brush Preset
//! picker, the layer-mask thumbnail menu and the effect rows' menu. Commands cannot be added to the
//! document tab menu (they would act on the active document, not the clicked tab).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::canvas_tool_menu::{CanvasToolMenu, Row};

mod apply;
pub mod backup;
pub mod catalog;
pub mod editor;
pub mod menus;
pub mod presets;
mod presets_ui;

#[cfg(test)]
mod presets_tests;
#[cfg(test)]
mod tests;

pub use apply::{Arrangement, CMD, SEPARATOR, Slot, apply, tidy};
pub use presets::Presets;

/// The Preferences page's id (`edit.preferences.contextMenus`).
pub const PAGE: &str = "contextMenus";
/// The page's title.
pub const PAGE_TITLE: &str = "Context Menus";

/// Everything arranged: each menu's arrangement, and the presets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Custom {
    pub menus: BTreeMap<String, Arrangement>,
    pub presets: Presets,
}

/// Most entries read from one list; the rest are ignored.
const MAX_ITEMS: usize = 1000;

impl Custom {
    /// Read the saved value leniently: anything this build cannot read is left out.
    pub fn from_value(v: &Value) -> Custom {
        let menus = v.get("menus").map(presets::menus_from_value).unwrap_or_default();
        let presets = v.get("presets").map(Presets::from_value).unwrap_or_default();
        Custom { menus, presets }
    }

    /// The value to save: Null when nothing is arranged.
    pub fn to_value(&self) -> Value {
        if self.menus.is_empty() && self.presets.is_empty() {
            return Value::Null;
        }
        let mut out = json!({});
        if !self.menus.is_empty() {
            out["menus"] = presets::menus_to_value(&self.menus);
        }
        if !self.presets.is_empty() {
            out["presets"] = self.presets.to_value();
        }
        out
    }
}

/// The arrangement saved in the preferences.
pub fn load(app: &PhotocraftApp) -> Custom {
    Custom::from_value(&app.session.prefs().context_menus)
}

/// Save `custom` in the preferences (nothing is written when it is what is saved).
pub fn store(app: &mut PhotocraftApp, custom: &Custom) {
    let value = custom.to_value();
    if app.session.prefs().context_menus != value {
        app.session.prefs.edit(|p| p.context_menus = value);
    }
}

/// The rows of menu `key` as arranged in `saved` (the preferences' value), over `default`, the
/// rows the menu builds now. A menu nothing is arranged for, and an arrangement that would leave
/// no row, give `default` back.
pub fn arranged(saved: &Value, key: &str, default: Vec<Row>) -> Vec<Row> {
    let custom = Custom::from_value(saved);
    let Some(arrangement) = custom.menus.get(key) else { return default };
    let rows = apply(arrangement, &default, &menus::universe(key), menus::can_add(key));
    if rows.iter().any(Option::is_some) { rows } else { default }
}

/// The menu key of an open canvas menu: the selection tools' menus only.
pub fn canvas_key(menu: &CanvasToolMenu) -> Option<&'static str> {
    use crate::canvas_tool_menu::{NO_SELECTION_MENU, SELECTION_MENU, rows};
    let built = rows(menu);
    if built.is_empty() {
        None
    } else if built == SELECTION_MENU {
        Some(menus::CANVAS_SELECTION)
    } else if built == NO_SELECTION_MENU {
        Some(menus::CANVAS_NONE)
    } else {
        None
    }
}

/// The rows an open canvas menu shows: [`rows`](crate::canvas_tool_menu::rows), arranged.
pub fn canvas_rows(app: &PhotocraftApp, menu: &CanvasToolMenu) -> Vec<Row> {
    let built = crate::canvas_tool_menu::rows(menu).to_vec();
    match canvas_key(menu) {
        Some(key) => arranged(&app.session.prefs().context_menus, key, built),
        None => built,
    }
}

/// Is `command` a row of the open canvas menu?
pub fn canvas_has(app: &PhotocraftApp, menu: &CanvasToolMenu, command: &str) -> bool {
    canvas_rows(app, menu).iter().flatten().any(|(_, id)| *id == command)
}

/// The Layers panel row menu's entries (`layer_menu_ui::entries`), arranged.
pub fn layer_rows(app: &PhotocraftApp, built: Vec<Row>) -> Vec<Row> {
    arranged(&app.session.prefs().context_menus, menus::LAYERS_ROW, built)
}

/// One document tab menu item: label, command id, params, enabled.
pub type TabItem = (&'static str, &'static str, Value, bool);

/// Which of the tab menu's `items` show, in order (`None` is a separator).
pub fn tab_rows(saved: &Value, items: &[TabItem]) -> Vec<Option<usize>> {
    let built: Vec<Row> = items.iter().map(|(label, id, ..)| Some((*label, *id))).collect();
    arranged(saved, menus::TAB, built)
        .into_iter()
        .filter_map(|row| match row {
            None => Some(None),
            Some((_, id)) => items.iter().position(|item| item.1 == id).map(Some),
        })
        .collect()
}
