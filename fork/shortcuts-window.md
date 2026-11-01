# Keyboard Shortcuts and Menus window

## What it does

One resizable size for the three tabs (opens 1000×640, remembered; the Toolbar tab fits without scrolling); recording a shortcut like VS Code (click a shortcut or Add Shortcut, each combination pressed is added, two make a chord, Enter accepts, Esc cancels, the clash warning shows before Enter); a quiet row selection; no Open Recent files, open documents or saved workspaces in the lists; a full-width search with Add Shortcut at its right; a reset icon on a changed row (tooltip "Reset to ⌘N"); Remove Shortcut on the row's right-click menu and on ⌫ / Delete; search by modifier words (cmd, shift, ctrl, alt and ⌘ ⇧ ⌃ ⌥); Menus tab with the Visible box and a colour dot (popover of 8 dots, none = empty circle with a red slash) pinned to the right edge. Removed: Delete Shortcut, Use Default, Add Chord, Import Shortcuts… (the JSON icons stay; `.kys` import stays in code), the "modified" text and the confirmation lines.

## Branch

`feat/shortcuts-window` (from `custom`)

## Own files

ui-egui `prefs/prefs_ui/shortcuts_window.rs` (size, footer, dynamic items), `shortcuts_record.rs`, `shortcuts_rows.rs`, `shortcuts_search.rs`, `shortcuts_menus.rs`, `shortcuts_window_tests.rs`, `shortcuts_window_snapshots.rs` (opt-in pictures: `PHOTOCRAFT_SHORTCUT_SNAPSHOTS` folder + `PHOTOCRAFT_SHORTCUT_PHASE`)

## Hooks in official files

ui-egui `dialogs.rs` (resizable and minimum size for the window, size kept like Curves), `menus.rs` + `menus/items.rs` (`menu_tint` visible to the crate), `prefs/prefs_ui.rs` (mod lines), `prefs_ui/dialogs.rs` (width, Toolbar tab fills the window), `prefs_ui/shortcuts.rs` (body rewritten: search row, rows, footer), `shortcuts_chord.rs` (capture step removed), `shortcut_target.rs`, `shortcuts_backup.rs`, `kys_import.rs` (comment); tests adapted in `shortcut_capture_tests.rs`, `shortcuts_upgrade_tests.rs`, `kys_import/tests.rs`, `prefs_ui/tests.rs`

## Tests

```bash
cargo test -p photocraft-ui-egui --lib -- shortcuts_window shortcuts_search shortcuts_rows shortcuts_menus shortcut_capture shortcuts_upgrade kys_import prefs_ui
```
