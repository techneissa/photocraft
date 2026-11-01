# Keyboard shortcut upgrade

## What it does

Keyboard Shortcuts upgrade in the existing window: chords (⌘K then C) with a keycap hint, When conditions per command (autocomplete, `!`, + builder; keys only: menus and the palette still run the command), a clash icon on the row, export/import JSON icons; shortcut sets carry chords and When (`.kys` stays keys-only)

## Branch

`feat/shortcut-upgrade`

## Own files

engine `when/`, `prefs/chords.rs`, `prefs/chord_cmds.rs`, `shortcut_backup.rs` (+ tests); ui-egui `keybinds.rs` + `keybinds/`, `prefs/prefs_ui/shortcut_target.rs` (`open_shortcuts_for`, the one entry point Assign Shortcut… uses), `shortcuts_when.rs`, `shortcuts_chord.rs`, `shortcuts_backup.rs`, `shortcuts_upgrade_tests.rs`; `assets/icons/download.svg`, `upload.svg` (Lucide)

## Hooks in official files

engine `lib.rs`, `prefs.rs` (`chords`, `shortcut_when`), `prefs/validate.rs`, `prefs/customize.rs`, `shortcut_sets.rs`; ui-egui `lib.rs`, `app.rs`, `app/frame.rs`, `input/shortcuts.rs` (chord poll, `tool_on`), `input/shortcut_dispatch.rs` (When-false → no key), `prefs/prefs_ui.rs`, `prefs_ui/dialogs.rs` (width up to 1000), `prefs_ui/shortcuts.rs` (column, chips, icons, clash icon), `file_dialog.rs` (`pick_file_bytes_limited`), `icon_data.rs`, `prefs_ui/tests.rs` + `shortcut_set_tests.rs`, `ATTRIBUTION.md` (Lucide count)

## Tests

engine `when chords chord_cmds shortcut_backup shortcut_sets`, ui-egui `keybinds shortcuts_upgrade shortcut_capture shortcut_set`

```bash
cargo test -p photocraft-engine --lib -- when chords chord_cmds shortcut_backup shortcut_sets image_cmds mode_cmds
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
