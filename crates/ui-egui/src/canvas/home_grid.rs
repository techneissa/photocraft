//! Fork (`fork/home-recent-grid.md`): what the fork adds to the framework's Home screen. The framework draws the
//! recent files as a grid of cards or a list, with the × on a card, the context menu and the previews decoded off
//! the main thread. This module gives it the fork's list: as many files as Preferences › Interface › Recent files on
//! Home allows, each with a line of its size in pixels and when it was last opened, and big PSDs shown by the
//! picture inside them.

mod caption;
mod preview;
#[cfg(test)]
mod snapshots;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use craft::ui::shell::RecentFile;

use super::PhotocraftApp;
pub(crate) use preview::Sizes;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use preview::load;

/// How many recent files Home shows: Preferences › Interface › Recent files on Home, 1 to 100.
pub(super) fn count(app: &PhotocraftApp) -> usize {
    app.session.prefs().interface.home_recent_count.clamp(1, 100) as usize
}

/// The recent files Home lists, newest first as File › Open Recent keeps them: each with its last-opened time (for
/// the sort) and the dim line under its name (size and when it was opened).
pub(super) fn recent_files(app: &PhotocraftApp) -> Vec<RecentFile> {
    let now = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let sizes = app.thumbnail_cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).sizes();
    let opened = &app.session.prefs().file_handling.recent_opened;
    let mut files = crate::home_recent_ui::recent_files(app, count(app));
    for file in &mut files {
        let path = file.path.to_string_lossy().into_owned();
        let opened = opened.get(&path).copied();
        file.opened = opened.and_then(|t| i64::try_from(t).ok());
        // An open document's size is its own; a closed file's comes from its preview's decode.
        let size = open_size(app, &path).or_else(|| sizes.get(&path));
        let line = caption::line(size, opened, now);
        file.note = (!line.is_empty()).then_some(line);
    }
    files
}

/// The size in pixels of the open document whose path is `path`.
fn open_size(app: &PhotocraftApp, path: &str) -> Option<(u32, u32)> {
    let path = PathBuf::from(path);
    app.session.documents().iter().find(|d| d.path.as_deref().is_some_and(|p| std::path::Path::new(p) == path)).map(|d| (d.doc.size.width, d.doc.size.height))
}
