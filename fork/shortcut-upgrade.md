# Keyboard shortcut upgrade

## What it does

Keyboard Shortcuts upgrade in the existing window: chords (⌘K then C) with a keycap hint, When conditions per command (autocomplete, `!`, + builder; keys only: menus and the palette still run the command), a clash icon on the row, export/import JSON icons; shortcut sets carry chords and When (`.kys` stays keys-only)

## Branch

`feat/shortcut-upgrade`

## Own files

engine-core `when/`, `prefs/chords.rs`, `prefs/chord_cmds.rs`, `shortcut_backup.rs` (+ tests); ui-egui `keybinds.rs` + `keybinds/`, `prefs/prefs_ui/shortcuts_chords.rs` (+ tests); `assets/icons/download.svg`, `upload.svg` (Lucide)

## Hooks in official files

engine-core `lib.rs`, `prefs.rs` (`chords`, `shortcut_when`), `prefs/validate.rs`, `prefs/customize.rs`, `prefs/store.rs` (a When change is a catalog change), `shortcut_sets.rs`; ui-egui `lib.rs`, `app.rs` (`raw_input_hook` calls the layer), `app/frame.rs` (the chord hint), `input/shortcuts.rs` (`handle` runs what the layer fired), `input/shortcut_dispatch.rs` (`unbind_gated`, `label` visible, `pick_tool` asks `tool_on`), `prefs/prefs_ui/shortcuts.rs` (the tab, `save_keys_with`, `open_shortcuts_for`), `prefs/prefs_ui.rs`, `ATTRIBUTION.md` (Lucide count)

## After the craft UI merge (2026-11-27)

Works again on craft's keymap, as the fork's own key layer in front of it:

- `keybinds/runtime.rs` reads the frame's key presses in `raw_input_hook` (after the clipboard fix), before craft's key stage. It takes only what it owns: the first key of a chord (arms it; the next key finishes it, Esc, a wrong key or 4 s cancel it), and a key of a command whose When condition holds (the command runs). Everything else reaches craft unchanged. The commands it fires run in `shortcuts::handle`.
- A command with a When condition has no keys in craft's keymap (`keybinds::unbind_gated`, called at the end of `shortcut_dispatch::overrides`; `PrefsStore::edit` counts a When change as a catalog change). When its condition fails, its key goes to whatever else has it (B is the next tool of its group: `gate::tool_on` takes it out of `pick_tool`'s group).
- The Chords and Conditions tab of Keyboard Shortcuts and Menus (`prefs_ui/shortcuts_chords.rs`): search; per command a chord button (click, press two keys; Esc cancels, ⌫ removes), a clash icon with its note, a When text field (red with the parse error); Export and Import JSON buttons (the backup file of `shortcut_backup`; Import applies at once and reopens the dialog). While it records, the key layer hands it the presses and nothing runs.
- OK writes the chords and conditions with the keys. The save resets the keys, and a reset clears chords and conditions, so `save_keys` writes them back (also for a `.kys` import).

Not rebuilt: the When builder (autocomplete, `!`, + chips; the field takes the text and shows parse errors), the clash icon beside the key in craft's own table row, the Assign Shortcut entry from right-clicks (fork/assign-shortcut-palette.md). A shortcut set (the dialog's Set menu) still carries keys only.

## Tests

```bash
cargo test -p photocraft-engine-core --lib -- when chords chord_cmds shortcut_backup shortcut_sets
cargo test -p photocraft-ui-egui --lib -- keybinds shortcuts_chords shortcut_capture shortcut_set
```
