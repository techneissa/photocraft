//! Fork: where a dialog opens. One rule for every dialog (they all pass through `dialogs::show`):
//! the first time, centred on the app window; a dialog the user moved reopens where they left it,
//! unless that place is now off screen (a smaller window or display), then centred again; short
//! confirm and alert prompts always open centred and never remember a place.

use egui::{Rect, Vec2};
use serde_json::Value;

use crate::state::{Dialog, DialogKind};

/// How much of a remembered dialog's top-left corner must lie inside the window for the place to
/// count as on screen: enough of the title bar to grab.
const MIN_VISIBLE: Vec2 = Vec2::new(80.0, 48.0);

/// `__prefsui` kinds that are short prompts, not windows.
const PROMPT_KINDS: [&str; 2] = ["settingsImport", "mismatch"];

/// Whether `d` is a short confirm or alert: it always opens centred and is never remembered.
pub(crate) fn is_alert(d: &Dialog) -> bool {
    let f = &d.fields;
    d.kind == DialogKind::Error
        || f.get("__prefsui").and_then(Value::as_str).is_some_and(|k| PROMPT_KINDS.contains(&k))
        || crate::open_prompt::owns(f)
        || crate::delete_layer_prompt::owns(f)
        || crate::replace_prompt::owns(f)
        || crate::disk_change_prompt::owns(f)
        || crate::mask_trash_prompt::owns(f)
        || crate::preset_group_prompt::owns(f)
        || crate::rasterize_prompt::owns(f)
}

/// Where a dialog's last position is remembered: per command for command dialogs (each filter has
/// its own), per Preferences-style window (Settings, Shortcuts…), per kind for the rest. Settings
/// and the other `__prefsui` windows used to share the key of every other command-less dialog, so
/// moving one moved them all.
pub(crate) fn place_id(d: &Dialog) -> egui::Id {
    if let Some(cmd) = d.fields.get("__command").and_then(Value::as_str) {
        egui::Id::unique(("dialog-place", cmd))
    } else if let Some(kind) = d.fields.get("__prefsui").and_then(Value::as_str) {
        egui::Id::unique(("dialog-place", "prefsui", kind))
    } else {
        egui::Id::unique(("dialog-place", format!("{:?}", d.kind)))
    }
}

/// The top-left offset (from the window's top-left) to open at, or `None` to centre: the
/// remembered `saved` place if its title bar is still inside `screen`.
pub(crate) fn start_offset(saved: Option<Vec2>, screen: Rect) -> Option<Vec2> {
    let o = saved.filter(|o| o.is_finite())?;
    let size = screen.size();
    (o.x >= 0.0 && o.y >= 0.0 && o.x + MIN_VISIBLE.x <= size.x && o.y + MIN_VISIBLE.y <= size.y).then_some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0))
    }

    #[test]
    fn no_saved_place_centres() {
        assert_eq!(start_offset(None, screen()), None);
    }

    #[test]
    fn a_saved_place_is_kept() {
        assert_eq!(start_offset(Some(vec2(300.0, 120.0)), screen()), Some(vec2(300.0, 120.0)));
    }

    #[test]
    fn an_off_screen_place_centres_again() {
        for o in [vec2(1400.0, 100.0), vec2(100.0, 880.0), vec2(-5.0, 10.0), vec2(f32::NAN, 0.0), vec2(5000.0, 5000.0)] {
            assert_eq!(start_offset(Some(o), screen()), None, "{o:?}");
        }
        // The same place fits a big window but not a small one.
        let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        assert!(start_offset(Some(vec2(900.0, 100.0)), screen()).is_some());
        assert_eq!(start_offset(Some(vec2(900.0, 100.0)), small), None);
    }

    fn dialog(kind: DialogKind, fields: Value) -> Dialog {
        Dialog { id: 1, kind, fields: fields.as_object().cloned().unwrap_or_default() }
    }

    #[test]
    fn prompts_are_alerts_and_windows_are_not() {
        assert!(is_alert(&dialog(DialogKind::Error, serde_json::json!({}))));
        assert!(is_alert(&dialog(DialogKind::Command, serde_json::json!({"__prefsui": "settingsImport"}))));
        assert!(!is_alert(&dialog(DialogKind::Command, serde_json::json!({"__prefsui": "prefs"}))));
        assert!(!is_alert(&dialog(DialogKind::About, serde_json::json!({}))));
    }

    #[test]
    fn settings_and_other_prefs_windows_remember_apart() {
        let prefs = dialog(DialogKind::Command, serde_json::json!({"__prefsui": "prefs"}));
        let shortcuts = dialog(DialogKind::Command, serde_json::json!({"__prefsui": "shortcuts"}));
        let plain = dialog(DialogKind::Command, serde_json::json!({}));
        assert_ne!(place_id(&prefs), place_id(&shortcuts));
        assert_ne!(place_id(&prefs), place_id(&plain));
    }
}
