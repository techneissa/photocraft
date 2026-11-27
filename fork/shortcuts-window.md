# Keyboard Shortcuts and Menus window

## What it does

One resizable size for the three tabs (opens 1000×640, remembered; the Toolbar tab fits without scrolling); recording a shortcut like VS Code (click a shortcut or Add Shortcut, each combination pressed is added, two make a chord, Enter ends the recording, Esc cancels; the first combination is applied to the command at once, as the official dialog and its tests do, so a second one takes it back for the chord, and Esc puts back the key the row had; the clash warning shows with it); a quiet row selection; no Open Recent files, open documents or saved workspaces in the lists; a full-width search with Add Shortcut at its right; a reset icon on a changed row (tooltip "Reset to ⌘N"); Remove Shortcut on the row's right-click menu and on ⌫ / Delete; search by modifier words (cmd, shift, ctrl, alt and ⌘ ⇧ ⌃ ⌥); Menus tab with the Visible box and a colour dot (popover of 8 dots, none = empty circle with a red slash) pinned to the right edge. Removed: Delete Shortcut, Use Default, Add Chord, Import Shortcuts… (the JSON icons stay; `.kys` import stays in code), the "modified" text and the confirmation lines.

## Branch

`feat/shortcuts-window` (from `custom`)

## Own files

ui-egui `prefs/prefs_ui/shortcuts_menus.rs`, `shortcuts_toolbar.rs`, `shortcuts_more_tests.rs`, `shortcuts_snapshots.rs`

## Hooks in official files

ui-egui `prefs/prefs_ui/shortcuts.rs` (the Menus tab body), `prefs/prefs_ui.rs` (mod lines), `menus.rs` (`menu_tint`), `menus/items.rs`

## After the craft UI merge (2026-11-27)

The fork's own egui window is gone; the official craft dialog (`ShortcutsDialog`) is the window now, opened from the same places (Edit › Keyboard Shortcuts and Edit › Menus). It already has what the fork's window added: a resizable size shared by its tabs, recording, a reset icon on a changed row, search by key (including ⌘ ⇧ ⌃ ⌥), Add and Remove, undo and redo. What the fork keeps on top of it:

- the Menus tab: a Visible check and a colour dot (popover of 8 dots, none = empty circle with a red slash) in aligned columns (`prefs_ui/shortcuts_menus.rs`, hook in `shortcuts.rs`, `menus.rs` exports `menu_tint`);
- the Chords and Conditions tab (fork/shortcut-upgrade.md), where a chord is recorded as two keys in a row.

Brought back after that (the `shortcuts-more` pass), on the craft dialog:

- Remove Shortcut on a row's right-click, and the clashing command in the tooltip of the clash mark in the table row: craft-libs patch 8 (`menu` crate, `shortcuts/rows.rs`, `view/table.rs`; fork/craft-libs.md). Craft already draws the mark and removes a key with ⌫ / Delete.
- The Toolbar tab (`prefs_ui/shortcuts_toolbar.rs`): a check per tool with its key, Restore Defaults; OK writes the unchecked tools through `edit.toolbar {hidden}` only when they changed. Edit › Toolbar still opens the toolbar drawer; `open(app, 2)` opens the dialog on this tab.
- Search by modifier words (cmd, shift, opt, alt, ctrl): craft's own search already finds them, so nothing was added to the table's search; the Chords and Conditions tab's search now also matches the keys as words.

Not rebuilt: the old window's own right-click menu entries other than Remove Shortcut. Pictures: `$WORKSPACE/assets/photocraft/shots/step2-shortcuts/` and `step2-shortcuts-more/` (`shortcuts_snapshots.rs`, opt-in: `PHOTOCRAFT_SHORTCUT_SNAPSHOTS` folder + `PHOTOCRAFT_SHORTCUT_PHASE`, optional `PHOTOCRAFT_SHORTCUT_TABS`).

## Tests

```bash
cargo test -p photocraft-ui-egui --lib -- shortcuts_chords shortcuts_more shortcut_capture shortcut_set kys_import prefs_ui
```
