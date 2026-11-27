# Assign Shortcut and command palette

## What it does

Assign Shortcut… on right-click of in-window menu items, palette rows, toolbar tools and buttons, the Search icon, Layers footer buttons, the options-bar tool icon; command palette on ⌘P as well as ⌘F (Print has no shortcut), scrolls past 12, PageUp/Down, ↑/↓ wrap, hover highlight, your own keys, better ranking

## Branch

`feat/assign-shortcut-palette`

## Own files

`crates/ui-egui/src/assign_shortcut.rs` + tests, `dialogs/palette/nav.rs`, `dialogs/palette/list.rs` + tests

## Hooks in official files

engine-core `prefs/shortcuts.rs` (⌘P alternate for `edit.search`), `cmd/print_cmds.rs` + ui-egui `menu_catalog.rs` (Print's Cmd+P removed); ui-egui `lib.rs`, `app/frame.rs` (1 line), `menus/render.rs` (1 line), `icons.rs` (`attach`), `assign_shortcut::next` one line before official `icons::button` calls in `panels/layers.rs`, `title_bar.rs`, `toolbar.rs`, `options_bar.rs`; `dialogs/palette.rs` (+14/−52); `input/shortcuts.rs` (early return while the menu is open)

## After the craft UI merge (2026-11-27)

Partly works: ⌘P opens the command palette (Print has no key). `prefs_ui::open_shortcuts_for(app, command)` opens Keyboard Shortcuts and Menus with the command selected and scrolled into view (the entry point of Assign Shortcut…), and it is tested. The right-click entry is not rebuilt: craft's menus, context menus and `CommandPalette` rows give an app no place for an extra item (nothing in craft-libs at a46af76c names one), so it needs a change in craft-libs (a context action on menu and palette rows). `assign_shortcut.rs` and `dialogs/palette/{list,nav}.rs` stay in the tree, compiled out, for that rebuild (they draw with the old egui widgets).

## Tests

```bash
cargo test -p photocraft-ui-egui --lib -- shortcuts_chords
```
