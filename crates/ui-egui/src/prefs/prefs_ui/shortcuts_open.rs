//! Open Keyboard Shortcuts on one command ("Assign Shortcut…" on a right-click).

use super::{PhotocraftApp, Value, json, open_shortcuts};

/// Open Keyboard Shortcuts (or reuse the open one) with `command` selected and key capture on.
/// The search box is set to the command's name so its row is in view. A command the dialog
/// doesn't list gets no capture: the next key would be stored for a row nobody can see.
pub(crate) fn open_shortcuts_for(app: &mut PhotocraftApp, command: &str) -> u64 {
    let listed = super::shortcuts::shortcut_items(app).into_iter().find(|i| i.0 == command);
    let label = listed.as_ref().map(|i| i.1.trim_end_matches('…').to_string()).unwrap_or_default();
    let open = app.ui.dialogs.iter().find(|d| d.fields.get("__prefsui").and_then(Value::as_str) == Some("shortcuts")).map(|d| d.id);
    let id = open.unwrap_or_else(|| open_shortcuts(app, 0));
    if let Some(d) = app.ui.dialogs.iter_mut().find(|d| d.id == id) {
        for (k, v) in [
            ("tab", json!(0)),
            ("selected", json!(if listed.is_some() { command } else { "" })),
            ("filter", json!(label)),
            ("capture", json!(listed.is_some())),
            ("captureAlt", json!(false)),
            ("conflict", Value::Null),
            ("message", json!("")),
        ] {
            d.fields.insert(k.into(), v);
        }
    }
    id
}
