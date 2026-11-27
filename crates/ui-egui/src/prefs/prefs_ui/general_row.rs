//! What Settings › General adds to Automatically Check for Updates, the last row of the page: the
//! fork's update check (fork/fork-update-check.md), Export Settings… and Import Settings…, and the
//! Sync Folder block (fork/settings-sync.md). The buttons file a [`Request`]; the app does the work
//! after the pass.

use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::choice::Checkbox;
use craft::ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft::ui::display::Text;
use craft::ui::layout_components::Row;
use craft::ui::patterns::preferences::SettingsDraft;
use craft::ui::style::TypeRole;

use super::settings_sync::{Request, Shared, lock};

/// The page's key the block hangs on.
pub(super) const KEY: &str = "general.automaticallyCheckUpdates";

/// What the block reads of the app, taken before each pass.
#[derive(Clone, Default)]
pub(super) struct Facts {
    /// The sync folder, empty until one is chosen.
    pub(super) folder: String,
    pub(super) keep: bool,
    /// The folder the other ArtCraft apps use, offered while none is chosen here.
    pub(super) offered: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) update: crate::fork_update::State,
}

/// The row: the check box, then the update check, the export and import buttons and the sync folder.
pub(super) fn show(ui: &mut egui::Ui, draft: &mut SettingsDraft, facts: &Facts, bridge: &Shared) {
    let stored: bool = draft.get(KEY).unwrap_or(true);
    let mut on = stored;
    Checkbox::new(Bind::bound(&mut on)).label(tl!("Automatically Check for Updates")).show(ui);
    if on != stored {
        draft.set(KEY, &on);
    }
    let mut asked = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    match crate::fork_update::settings_row(ui, &facts.update) {
        Some(crate::fork_update::Click::Check) => asked.push(Request::CheckUpdate),
        Some(crate::fork_update::Click::Update) => asked.push(Request::Update),
        None => {}
    }
    Row::new().show(ui, |ui, _| {
        if Button::new(tl!("Export Settings…")).show(ui).clicked() {
            asked.push(Request::Export);
        }
        if Button::new(tl!("Import Settings…")).show(ui).clicked() {
            asked.push(Request::Import);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    sync_folder(ui, facts, &mut asked);
    let message = lock(bridge).message.clone();
    if let Some((text, warning)) = message.filter(|(text, _)| !text.is_empty()) {
        let line = Text::new(text);
        if warning { line.tone(Tone::Warn) } else { line.role(TypeRole::Caption) }.show(ui);
    }
    lock(bridge).requests.extend(asked);
}

/// The Sync Folder block: its title, the chosen folder (or the one the other ArtCraft apps use, to take in one click)
/// on its own line, and the buttons under it.
#[cfg(not(target_arch = "wasm32"))]
fn sync_folder(ui: &mut egui::Ui, facts: &Facts, asked: &mut Vec<Request>) {
    let chosen = !facts.folder.is_empty();
    Text::new(tl!("Sync Folder").to_string()).role(TypeRole::BodyStrong).show(ui);
    let shown = match (&facts.folder, &facts.offered) {
        (folder, _) if chosen => super::settings_folder::tilde(folder),
        (_, Some(offered)) => super::settings_folder::tilde(offered),
        _ => tl!("None chosen").to_string(),
    };
    Text::new(shown).role(TypeRole::Caption).show(ui);
    Row::new().show(ui, |ui, _| {
        if !chosen && facts.offered.is_some() && Button::new(tl!("Use This Folder")).show(ui).clicked() {
            asked.push(Request::UseOffered);
        }
        if Button::new(tl!("Choose…")).show(ui).clicked() {
            asked.push(Request::Choose);
        }
        if Button::new(tl!("Save to Folder")).enabled(chosen).show(ui).clicked() {
            asked.push(Request::Save);
        }
        if Button::new(tl!("Load from Folder")).enabled(chosen).show(ui).clicked() {
            asked.push(Request::Load);
        }
        let mut keep = facts.keep;
        Checkbox::new(Bind::bound(&mut keep)).label(tl!("Keep in Sync")).enabled(chosen).show(ui);
        if keep != facts.keep {
            asked.push(Request::Keep(keep));
        }
    });
}
