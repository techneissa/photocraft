//! The web build's stand-in for [`settings_folder`](super): a browser has no sync folder, so the
//! block, the commands, the start offer and the save on quit do nothing. The names match the
//! native module's, so the hooks in the shared code need no `cfg`.

use serde_json::Value;

use super::PhotocraftApp;
use super::settings_sync::Request;

/// Who saved a settings file: never known in a browser, which has no sync folder.
#[derive(Clone, Debug)]
pub(super) struct SavedBy {
    pub(super) computer: String,
}

impl SavedBy {
    pub(super) fn when_text(&self) -> String {
        String::new()
    }
}

pub(super) fn run(_: &mut PhotocraftApp, _: Request) -> Result<Value, String> {
    Ok(Value::Null)
}

pub(super) fn invoke(_: &mut PhotocraftApp, _: &str, _: &Value) -> Option<Result<Value, String>> {
    None
}

pub fn offer_newer_file(_: &mut PhotocraftApp) {}

pub fn save_on_exit(_: &mut PhotocraftApp) {}
