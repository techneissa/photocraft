# Assign Shortcut and command palette

## What it does

Assign Shortcut… on right-click of in-window menu items, palette rows, toolbar tools and buttons, the Search icon, Layers footer buttons, the options-bar tool icon; command palette on ⌘P as well as ⌘F (Print has no shortcut), scrolls past 12, PageUp/Down, ↑/↓ wrap, hover highlight, your own keys, better ranking

## Branch

`feat/assign-shortcut-palette`

## Own files

`crates/ui-egui/src/assign_shortcut.rs` + tests, `dialogs/palette/nav.rs`, `dialogs/palette/list.rs` + tests

## Hooks in official files

engine `prefs/shortcuts.rs` (⌘P alternate for `edit.search`), `cmd/print_cmds.rs` + ui-egui `menu_catalog.rs` (Print's Cmd+P removed); ui-egui `lib.rs`, `app/frame.rs` (1 line), `menus/render.rs` (1 line), `icons.rs` (`attach`), `assign_shortcut::next` one line before official `icons::button` calls in `panels/layers.rs`, `title_bar.rs`, `toolbar.rs`, `options_bar.rs`; `dialogs/palette.rs` (+14/−52); `input/shortcuts.rs` (early return while the menu is open)

## Tests

`assign_shortcut`, `dialogs::palette`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
