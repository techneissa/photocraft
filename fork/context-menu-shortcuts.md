# Context menu shortcuts

## What it does

Right-click menus show each command's current shortcut at the right, as the menu bar does (the user's own assignment included; no shortcut shows nothing): Layers row and mask menus, canvas menus, document tab menu, Paths panel menu

## Branch

`feat/context-menu-shortcuts`

## Own files

`crates/ui-egui/src/menu_custom/shortcut.rs` (with its tests)

## Hooks in official files

ui-egui `canvas/canvas_tool_menu.rs` (2 lines), `canvas/tabs.rs` (`tab_context_menu` takes the app), `panels/layer_menu_ui.rs` (1 line), `tools/vector_ui/paths_panel.rs` (1 line), `menu_custom.rs` (`pub mod shortcut`)

## Tests

```bash
cargo test -p photocraft-ui-egui --lib -- menu_custom::shortcut
```
