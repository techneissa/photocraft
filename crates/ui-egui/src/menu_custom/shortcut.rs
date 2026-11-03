//! The shortcut a right-click menu row shows at its right, as the menu bar does.

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

/// `button` with the shortcut of `id` at its right, when it has one.
pub fn show<'a>(app: &PhotocraftApp, id: &str, button: egui::Button<'a>) -> egui::Button<'a> {
    match text(app, id) {
        Some(t) => button.shortcut_text(t),
        None => button,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> PhotocraftApp {
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
