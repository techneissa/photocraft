# ⌘, or Esc closes Preferences

## What it does

⌘, or Esc closes Preferences; with unsaved changes (Context Menus draft included) a small prompt asks Save / Don't Save / Cancel; Esc in a text field or open dropdown keeps its own job; the Cancel button and close box still discard

## Branch

`feat/settings-close-keys` (from `feat/settings-cmd-comma`), merged via `feat/settings-close-keys-custom`

## Own files

ui-egui `prefs/prefs_ui/close_keys.rs`, `close_keys_tests.rs`

## Hooks in official files

ui-egui `input/shortcuts.rs` (hook), `prefs/prefs_ui.rs`, `prefs_ui/dialogs.rs`

## Tests

`close_keys`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
