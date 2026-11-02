//! A sync folder for Settings sync (fork/settings-sync.md): Save to Folder and Load from Folder
//! keep every setting in `photocraft.pcsettings` in a folder the user chose (one in iCloud Drive,
//! say), and Keep in Sync saves there on quit and offers a newer file from another Mac at start.
//!
//! The file is the Export Settings… file plus a top-level `savedBy` object,
//! `{"computer": "<the Mac's name>", "at": "<RFC 3339 time>"}`; [`super::settings_sync`] reads it
//! like any other file (an older build skips it and names it, a build before that refuses it).
//! Other ArtCraft apps keep their own file in the same folder (`lightcraft.lcsettings`); only
//! `FILE_NAME` is ever read or written here.
//!
//! The chosen folder is also kept in `~/Library/Application Support/ArtCraft/sync-folder.json`
//! (`{"path": "<folder>"}`), shared by the ArtCraft apps: an app with no folder yet offers it.
//!
//! Native only: the folder, file, clock and commands are in `native` (the web build has no folders
//! and no clock, and does not register the two commands); `SyncPrefs` is on every target because
//! `Preferences` holds it.
//!
//! A file is written to a hidden temporary name in the same folder and renamed over the old one,
//! so a sync service never sees half a file, and a failed write leaves the old file alone.

use serde::{Deserialize, Serialize};

/// `sync.*` of the preferences: this Mac's sync folder and state. Not part of the settings file
/// (each Mac has its own).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncPrefs {
    /// The chosen sync folder; empty when none is chosen.
    pub folder: String,
    /// Keep in Sync: save to the folder on quit, and offer a newer file from another Mac at start.
    pub keep: bool,
    /// The `savedBy.at` of the file this Mac saved to the folder or loaded from it last.
    pub last: String,
}

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
