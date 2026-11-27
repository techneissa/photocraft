//! The shortcut a right-click menu row shows at its right, as the menu bar does.

use craft::ui::contract::Entry;

use crate::PhotocraftApp;

/// The pretty shortcut now in effect for `id`: the user's own assignment when there is one (an
/// emptied one means none), else the command's default. `None` shows nothing.
pub fn text(app: &PhotocraftApp, id: &str) -> Option<String> {
    let prefs = app.session.prefs();
    // The menu bar's Create Clipping Mask row turns into Release when its key would release: the
    // key is the Create command's.
    let id = if id == "layer.releaseClippingMask" { "layer.createClippingMask" } else { id };
    let sc = if prefs.shortcuts.contains_key(id) { prefs.shortcut(id, None).map(str::to_string) } else { crate::shortcuts::default_shortcut(id) };
    sc.filter(|s| !s.is_empty()).map(|s| crate::shortcuts::pretty(&s))
}

/// `entry`, the row of command `id` in a right-click menu, with the command's shortcut at its right and an Assign
/// Shortcut… item on its own right-click. A row that is no command stays as it is.
pub fn entry<T>(app: &PhotocraftApp, entry: Entry<T>, id: &str) -> Entry<T> {
    if photocraft_engine::commands::find(id).is_none() && !crate::menu_catalog::CATALOG.iter().any(|m| m.3 == id) {
        return entry;
    }
    let entry = match text(app, id) {
        Some(shortcut) => entry.shortcut(shortcut),
        None => entry,
    };
    entry.row_action(tl!("Assign Shortcut…"), id)
}

/// Per-frame upkeep (`app::framework`): the word of the Assign Shortcut… item that the tool rail, the Layers footer and
/// the Paths panel's row menu offer on a right click (craft-libs patch 12, `fork/craft-libs.md`).
pub fn sync(ctx: &egui::Context) {
    craft::ui::menu::set_row_action_label(ctx, Some(tl!("Assign Shortcut…").to_string()));
}

/// Runs the Assign Shortcut… item a menu or the palette reported: opens Keyboard Shortcuts on its command.
pub fn assign(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if let Some(command) = craft::ui::menu::take_row_action(ctx) {
        crate::prefs_ui::open_shortcuts_for(app, &dialog_id(&command));
    }
}

/// The id Keyboard Shortcuts lists for `command`: a rail tool's `toolSelect.<tool>` is listed as `tools.select.<tool>`.
pub(crate) fn dialog_id(command: &str) -> String {
    use craft::ui::contract::CommandSet as _;
    use craft::ui::runtime::commands::CommandRef;
    match crate::panels::tool_commands::ToolCommand::from_ref(&CommandRef::new(command)) {
        Some(tool) => crate::input::tool_shortcuts::id(crate::panels::tool_commands::tool_of(tool)),
        None => command.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> Box<PhotocraftApp> {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width": 32, "height": 32})).unwrap();
        app
    }

    #[test]
    fn a_command_with_a_shortcut_shows_it() {
        let app = app();
        assert_eq!(text(&app, "file.close"), Some(crate::shortcuts::pretty("Cmd+W")));
    }

    #[test]
    fn a_reassigned_shortcut_shows_the_new_one_and_a_cleared_one_nothing() {
        let mut app = app();
        app.session.prefs.edit(|p| p.shortcuts.insert("file.close".into(), "Cmd+Alt+J".into()));
        assert_eq!(text(&app, "file.close"), Some(crate::shortcuts::pretty("Cmd+Alt+J")));
        app.session.prefs.edit(|p| p.shortcuts.insert("file.close".into(), String::new()));
        assert_eq!(text(&app, "file.close"), None);
    }

    #[test]
    fn a_command_without_a_shortcut_shows_nothing() {
        assert_eq!(text(&app(), "layer.linkLayers"), None);
    }
}
