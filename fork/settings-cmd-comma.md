# ⌘, opens Settings

## What it does

⌘, opens Settings, as in every Mac app (⌘K still works); Hide Layers has no default key (Photoshop's ⌘,): give it one with Assign Shortcut…

## Branch

`feat/settings-cmd-comma`

## Own files

none

## Hooks in official files

engine-core `cmd/extra_cmds.rs` (Hide Layers' default key → None), `prefs/shortcuts.rs` (`edit.preferences.general` alternate `Cmd+,`); ui-egui `layer_shortcut_tests.rs` (the ⌘, test now checks Settings), `menus/native_menu/tests.rs` (2 asserts)

## Tests

engine-core `shortcuts`, ui-egui `layer_shortcut native_menu`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```

## After the craft UI merge (2026-11-27)

Works: ⌘, opens Settings; Hide Layers has no default key (engine hooks). Rebuild: nothing.
