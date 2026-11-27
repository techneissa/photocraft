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

## On the craft UI (2026-11-27)

Rebuilt with a craft-libs patch ([craft-libs.md](craft-libs.md) patch 6): `Entry::shortcut` shows text at the right end of a menu row. `menu_custom::shortcut::entry` puts the command's current shortcut (the user's own included) on each command row of the Layers row menu, the canvas selection menus and the document tab menu; `PathsPanel::shortcut` does it for the four Paths panel row commands that map to one engine command (Make Selection, Fill Path, Stroke Path, Delete Path; `tools/vector_ui/paths_panel.rs`). Those four have no default key, so the Paths menu shows only a key the user assigned. The Layers panel's mask and effects menus keep their rows without keys.
