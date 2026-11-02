//! The web build's stand-in for [`settings_folder`](super): a browser has no sync folder, so the
//! row, the commands, the start offer and the save on quit do nothing. The names match the
//! native module's, so the hooks in the shared code need no `cfg`.

use serde_json::{Map, Value};

use super::PhotocraftApp;

/// What the row asked for: nothing, on the web.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Clicks;

pub(super) fn row(_: &mut egui::Ui, _: &PhotocraftApp, _: &mut Map<String, Value>) -> Clicks {
    Clicks
}

pub(super) fn run(_: &mut PhotocraftApp, _: &mut Map<String, Value>, _: Clicks) {}

pub(super) fn saved_line(_: &mut egui::Ui, _: &Map<String, Value>) {}

pub(super) fn note_loaded(_: &mut PhotocraftApp, _: &Map<String, Value>) {}

pub(super) fn invoke(_: &mut PhotocraftApp, _: &str, _: &Value) -> Option<Result<Value, String>> {
    None
}

pub fn offer_newer_file(_: &mut PhotocraftApp) {}

pub fn save_on_exit(_: &mut PhotocraftApp) {}
