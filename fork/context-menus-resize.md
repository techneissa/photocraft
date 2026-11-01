# Context menus page size

## What it does

Context Menus page keeps the size the Preferences window is dragged to

## Branch

`fix/context-menus-resize` (from `feat/context-menu-editor`)

## Own files

in `menu_custom/editor.rs`

## Hooks in official files

none

## Tests

`menu_custom`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
