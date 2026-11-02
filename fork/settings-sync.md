# Settings sync

## What it does

Export Settings… / Import Settings… move all settings to another Mac in one `.pcsettings` file (JSON, `kind` `photocraft-settings`, `version` 1): two icons at the top right of Preferences, and engine commands `edit.preferences.exportSettings` / `edit.preferences.importSettings` (Edit ▸ Preferences; `{path}` or `{text}`, `dryRun`). Parts: Preferences (every section, Top Bar items, History Options, Layer Mask Display Options), Color Settings, Keyboard Shortcuts (the shortcuts backup with chords and When, saved sets), Menus, Toolbar, Context Menus, Workspaces and Panels (saved layouts, lock, live layout, no screen positions), tool options and view settings, spelling dictionary. Left out: recent files, folders and files of one Mac (LUT folder, log file, plug-in folder, scratch disks, profile paths, monitor profile), graphics and memory limits (adapter, backend, GPU memory, Memory Usage, tile size), the current tool and brush, last dialog choices, recent colors, script events, and the preset libraries (gradients, styles, shapes, patterns, swatches, tool presets, export and document presets: libraries, moved by Edit ▸ Presets ▸ Export/Import Presets). A file is checked whole first: a bad value for a setting this build knows is a short error and nothing changes. A setting, section or part this build does not have is skipped and named, in a file of any version, so a file from a newer PhotoCraft imports everything this build understands; a part or key the file lacks stays. The prompt names what is replaced and, in one calm line, the skipped settings (the first five, then "and N more"; the full list on hover). The command's reply (`dryRun` too) has `unknown` (the names, at most 200) and `unknownCount` next to `skipped` (other notes). OK applies in one step and saves. Options bar: the brush size under the brush preview stays inside its box (upstream has the same lines)

## Branch

`feat/settings-sync`, `fix/settings-import-skip-unknown`

## Own files

engine `prefs/settings_sync.rs` + `settings_sync/tests.rs`; ui-egui `prefs/prefs_ui/settings_sync.rs` + `settings_sync_tests.rs`, `panels/brush_size_label_tests.rs`

## Hooks in official files

engine `lib.rs` (`Session::settings_checks`), `prefs.rs` (`mod settings_sync`), `prefs/cmds.rs` (two specs added), `shortcut_sets.rs` (`validate` is `pub(crate)`); automation `workspace.rs` (+ test: the two commands with `path` are refused for agents); ui-egui `prefs/prefs_ui.rs`, `prefs_ui/dialogs.rs` (route, OK on the prompt), `prefs_ui/fields.rs` (icons beside the search field, prompt body), `dialogs.rs` (OK reads Import), `menus/items.rs` (menu order), `panels.rs` (test mod), `panels/brush_controls.rs` (brush size text 2 pt higher, preview 1 pt higher)

## Tests

engine `settings_sync`, automation `automation_rejects_ambient_path`, ui-egui `settings_sync brush_size_label`

```bash
cargo test -p photocraft-engine --lib -- settings_sync
cargo test -p photocraft-ui-egui --lib -- settings_sync brush_size_label
cargo test -p photocraft-automation --lib -- automation_rejects_ambient_path
```
