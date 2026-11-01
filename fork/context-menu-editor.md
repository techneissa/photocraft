# Context menu editor

## What it does

Arrange the right-click menus (Preferences ▸ Context Menus): canvas with/without a selection, Layers row, document tab; hide, reorder, add commands and separators; 3 presets in one compact row, `.json` backup (`kind` `photocraft-context-menus`); edits wait for OK

## Branch

`feat/context-menu-editor`

## Own files

`crates/ui-egui/src/menu_custom.rs` and `menu_custom/` (`apply`, `menus`, `catalog`, `presets`, `backup`, `editor`, `presets_ui`, tests), `docs/context-menu-editor.md`

## Hooks in official files

engine `prefs.rs` (`context_menus`); ui-egui `lib.rs`, `canvas/canvas_tool_menu.rs` (3 small changes), `canvas/tabs.rs` (`tab_context_menu` takes the saved value), `canvas.rs` (test re-export), `panels/layer_menu_ui.rs`, `prefs/prefs_ui/fields.rs` + `dialogs.rs` (page route, draft, OK commit; merged with the AI Models section list by hand), `canvas/tests.rs`

## Tests

`menu_custom`, `canvas_tool_menu`, `layer_menu`, `tab_context` (the guard tests fail when upstream adds a row to a covered menu: add it to the editor)

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
