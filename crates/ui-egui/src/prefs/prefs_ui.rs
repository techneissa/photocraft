//! Edit › Preferences, Keyboard Shortcuts and Menus, Color Settings and the rest of the Edit
//! menu's dialogs, plus the shell side of preferences: loading and saving them through
//! [`crate::Services`], applying the interface theme, autosave and crash recovery, and the
//! history log.
//!
//! The values live in the engine ([`photocraft_engine::prefs::Preferences`], so agents read and
//! change them with `prefs.get` / `prefs.set`); this module only edits a working copy in a dialog
//! and commits it with those commands on Apply or OK.

use std::collections::VecDeque;

use egui::Color32;
use photocraft_engine::jobs::{JobEvent, JobId, JobOutcome, Started};
use photocraft_engine::prefs::{self, AppearanceMode};
use serde_json::{Value, json};

use crate::PhotocraftApp;

/// Shell runtime state for preferences, autosave and snapping (not serialised).
#[derive(Default)]
pub struct Runtime {
    loaded: bool,
    recovery_started: bool,
    recovery_queue: VecDeque<crate::Recoverable>,
    recovery_pending: Option<(JobId, String)>,
    log_len: usize,
    /// The preferences revision last handed to the settings writer.
    saved_rev: u64,
    #[cfg(target_arch = "wasm32")]
    pub(super) saved_colors: Option<Value>,
    /// Settings › AI Models: the models' downloads, kept here so they outlive the window (fork/select-subject-ai.md).
    #[cfg(not(target_arch = "wasm32"))]
    ai_models: std::sync::Arc<std::sync::Mutex<ai_models::Runtime>>,
    /// The Brush's own brush while Double Exposure's soft one stands in for it (fork/double-exposure.md).
    pub(crate) double_exposure_brush: Option<crate::double_exposure::Stash>,
    /// What the Settings window and the app say to each other: its buttons' requests and prompts (fork/settings-sync.md).
    bridge: settings_sync::Shared,
    /// The open Keyboard Shortcuts and Menus window.
    pub(crate) shortcuts: Option<shortcuts::ShortcutsSession>,
    /// Style last sent to the GPU canvas.
    pub(crate) gpu_style: Option<crate::gpu_canvas::CanvasStyle>,
    /// `unitsAndRulers.showRulers` as last applied to View › Rulers.
    show_rulers: Option<bool>,
    /// Tool options as last remembered in the preferences (see `tool_memory`).
    pub(crate) tool_options: Option<crate::state::ToolOptions>,
    pub(crate) selected_tool: Option<crate::state::Tool>,
    pub(crate) tool_colors: Option<[[[f32; 4]; 2]; 2]>,
    pub(crate) tool_cmyk_picks: crate::tool_memory::CmykPicks,
    pub(crate) tool_cmyk_roles: Option<crate::tool_memory::CmykRoles>,
    pub(crate) group_tools: Option<Vec<crate::state::Tool>>,
    /// The tool owning the session brush and the other tools' brushes, as last remembered.
    pub(crate) tool_brushes: Option<crate::tool_memory::ToolBrushes>,
}

// ------------------------------------------------------------------ shortcuts

pub use crate::shortcuts::{default_shortcut, effective_shortcut};

// fork: Settings › AI Models (fork/select-subject-ai.md, birefnet-full.md, arabic-ai-models.md).
#[cfg(not(target_arch = "wasm32"))]
mod agents;
#[cfg(not(target_arch = "wasm32"))]
mod ai_models;
mod appearance;
mod close_keys;
mod custom_rows;
mod dialogs;
mod fields;
mod general_row;
mod presets;
mod save;
// fork: Settings sync (fork/settings-sync.md); the web has no sync folder.
#[cfg(not(target_arch = "wasm32"))]
mod settings_folder;
#[cfg(target_arch = "wasm32")]
#[path = "prefs_ui/settings_folder_web.rs"]
mod settings_folder;
mod settings_sync;
pub(crate) mod shortcuts;
mod shortcuts_chords;
mod shortcuts_menus;
mod startup;

// PLAN: assumed top-w18's appearance file keeps `display_scale` and `selected_theme` and exports `theme_menu`.
pub use appearance::{canvas_style, pasteboard_color};
pub(crate) use appearance::{cycle_appearance, interface_zoom, show_selected_theme, step_brightness, sync_appearance, theme_menu};
#[cfg(test)]
pub(crate) use dialogs::set_draft;
pub use dialogs::{apply_window, invoke, open_preferences, window_open};
pub(crate) use dialogs::{context_menus_draft, feed, set_context_menus_draft, take as take_window};
pub(crate) use presets::{PRESETS_FAMILY, take as take_presets};
pub(crate) use save::save_preferences;
pub use save::{autosave_now, tick};
pub use shortcuts::{open as open_shortcuts, open_shortcuts_for, recording, shortcuts_open, show as show_shortcuts};
pub use startup::load;
// fork: Keep in Sync at start and on quit (fork/settings-sync.md).
pub use settings_folder::{offer_newer_file as offer_newer_sync_file, save_on_exit as save_sync_on_exit};
pub(crate) use settings_sync::answer as import_answer;
pub(crate) use startup::{on_recovery_event, remember_rulers};

// Test modules stay after the production code: the prefs_usage test reads each file up to
// its first `#[cfg(test)] mod`.
// fork: tests of the Settings hooks (fork/settings-sync.md, fork/settings-close-keys.md).
#[cfg(test)]
mod agents_tests;
#[cfg(test)]
mod close_keys_tests;
#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod settings_sync_tests;

#[cfg(test)]
mod shortcut_capture_tests;
#[cfg(test)]
mod shortcut_set_tests;
#[cfg(test)]
mod shortcuts_chords_tests;
#[cfg(test)]
mod shortcuts_snapshots;

#[cfg(test)]
mod tests;
