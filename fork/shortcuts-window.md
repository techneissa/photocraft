# Keyboard Shortcuts and Menus window

## What it does

One resizable size for the three tabs (opens 1000×640, remembered; the Toolbar tab fits without scrolling); recording a shortcut like VS Code (click a shortcut or Add Shortcut, each combination pressed is added, two make a chord, Enter ends the recording, Esc cancels; the first combination is applied to the command at once, as the official dialog and its tests do, so a second one takes it back for the chord, and Esc puts back the key the row had; the clash warning shows with it); a quiet row selection; no Open Recent files, open documents or saved workspaces in the lists; a full-width search with Add Shortcut at its right; a reset icon on a changed row (tooltip "Reset to ⌘N"); Remove Shortcut on the row's right-click menu and on ⌫ / Delete; search by modifier words (cmd, shift, ctrl, alt and ⌘ ⇧ ⌃ ⌥); Menus tab with the Visible box and a colour dot (popover of 8 dots, none = empty circle with a red slash) pinned to the right edge. Removed: Delete Shortcut, Use Default, Add Chord, Import Shortcuts… (the JSON icons stay; `.kys` import stays in code), the "modified" text and the confirmation lines.

## Branch

`feat/shortcuts-window` (from `custom`)

## Own files

ui-egui `prefs/prefs_ui/shortcuts_menus.rs`, `shortcuts_snapshots.rs`

## Hooks in official files

ui-egui `prefs/prefs_ui/shortcuts.rs` (the Menus tab body), `prefs/prefs_ui.rs` (mod lines), `menus.rs` (`menu_tint`), `menus/items.rs`

## After the craft UI merge (2026-11-27)

The fork's own egui window is gone; the official craft dialog (`ShortcutsDialog`) is the window now, opened from the same places (Edit › Keyboard Shortcuts and Edit › Menus). It already has what the fork's window added: a resizable size shared by its tabs, recording, a reset icon on a changed row, search by key (including ⌘ ⇧ ⌃ ⌥), Add and Remove, undo and redo. What the fork keeps on top of it:

- the Menus tab: a Visible check and a colour dot (popover of 8 dots, none = empty circle with a red slash) in aligned columns (`prefs_ui/shortcuts_menus.rs`, hook in `shortcuts.rs`, `menus.rs` exports `menu_tint`);
- the Chords and Conditions tab (fork/shortcut-upgrade.md), where a chord is recorded as two keys in a row.

Not rebuilt (craft's dialog has no hook for them, and craft-libs is not edited here): right-click Remove Shortcut on a row, the clash warning beside the recorded key in the table row, the Toolbar tab (Edit › Toolbar opens the toolbar drawer instead). The old window's files are deleted (`git log` has them). Pictures: `$WORKSPACE/assets/photocraft/shots/step2-shortcuts/` (`shortcuts_snapshots.rs`, opt-in: `PHOTOCRAFT_SHORTCUT_SNAPSHOTS` folder + `PHOTOCRAFT_SHORTCUT_PHASE`).

## Tests

```bash
cargo test -p photocraft-ui-egui --lib -- shortcuts_chords shortcut_capture shortcut_set kys_import prefs_ui
```
