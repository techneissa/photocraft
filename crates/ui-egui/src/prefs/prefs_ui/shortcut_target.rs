//! Open Keyboard Shortcuts on one command with key capture on: the entry point for "Assign
//! Shortcut…" (a right-click on a menu item, a palette row).

use serde_json::json;

use super::shortcuts::shortcut_items;
use super::{PhotocraftApp, Value};

/// Open Keyboard Shortcuts (or reuse the open one) with `command_id` selected, scrolled into view
/// and waiting for a key press. An id the dialog doesn't list opens it plain. Returns the dialog id.
#[allow(dead_code)] // callers: Assign Shortcut… (right-click, palette)
pub(crate) fn open_shortcuts_for(app: &mut PhotocraftApp, command_id: &str) -> u64 {
    let known = shortcut_items(app).iter().any(|i| i.0 == command_id);
    let open = app.ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(Value::as_str) == Some("shortcuts")).map(|d| d.id);
    let id = open.unwrap_or_else(|| super::open_shortcuts(app, 0));
    if known && let Some(d) = app.ui.dialog_mut(id) {
        let f = &mut d.fields;
        f.insert("tab".into(), json!(0));
        f.insert("filter".into(), json!(""));
        f.insert("selected".into(), json!(command_id));
        f.insert("capture".into(), json!(true));
        f.insert("captureAlt".into(), json!(false));
        f.insert("reveal".into(), json!(true));
        f.insert("conflict".into(), Value::Null);
        f.insert("message".into(), json!(""));
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(app: &PhotocraftApp, id: u64, name: &str) -> Value {
        app.ui.dialogs.iter().find(|d| d.id == id).and_then(|d| d.fields.get(name).cloned()).unwrap_or(Value::Null)
    }

    #[test]
    fn selects_the_command_and_starts_capture() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let id = open_shortcuts_for(&mut app, "edit.copy");
        assert_eq!(field(&app, id, "selected"), json!("edit.copy"));
        assert_eq!(field(&app, id, "capture"), json!(true));
        assert_eq!(field(&app, id, "tab"), json!(0));
    }

    #[test]
    fn reuses_the_open_dialog_and_ignores_unknown_ids() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let first = open_shortcuts_for(&mut app, "edit.copy");
        let again = open_shortcuts_for(&mut app, "edit.paste");
        assert_eq!(first, again);
        assert_eq!(app.ui.dialogs.len(), 1);
        assert_eq!(field(&app, again, "selected"), json!("edit.paste"));
        let before = field(&app, again, "selected");
        open_shortcuts_for(&mut app, "no.such.command");
        assert_eq!(field(&app, again, "selected"), before);
        assert_eq!(field(&app, again, "capture"), json!(true));
    }
}
