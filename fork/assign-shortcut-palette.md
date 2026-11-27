# Assign Shortcut and command palette

## What it does

Assign Shortcut… on right-click of in-window menu items, palette rows, toolbar tools and buttons, the Search icon, Layers footer buttons, the options-bar tool icon; command palette on ⌘P as well as ⌘F (Print has no shortcut), scrolls past 12, PageUp/Down, ↑/↓ wrap, hover highlight, your own keys, better ranking

## Branch

`feat/assign-shortcut-palette`; `feat/step2-last` (the craft UI)

## Own files

`crates/ui-egui/src/assign_shortcut.rs` + tests, `dialogs/palette/nav.rs`, `dialogs/palette/list.rs` + tests

## Hooks in official files

engine-core `prefs/shortcuts.rs` (⌘P alternate for `edit.search`), `cmd/print_cmds.rs` + ui-egui `menu_catalog.rs` (Print's Cmd+P removed); ui-egui `lib.rs`, `app/frame.rs` (1 line), `menus/render.rs` (1 line), `icons.rs` (`attach`), `assign_shortcut::next` one line before official `icons::button` calls in `panels/layers.rs`, `title_bar.rs`, `toolbar.rs`, `options_bar.rs`; `dialogs/palette.rs` (+14/−52); `input/shortcuts.rs` (early return while the menu is open)

## After the craft UI merge (2026-11-27)

Works: ⌘P opens the command palette (Print has no key). `prefs_ui::open_shortcuts_for(app, command)` opens Keyboard Shortcuts and Menus with the command selected and scrolled into view (the entry point of Assign Shortcut…), and it is tested. Assign Shortcut… on a right click is back in these places:

- command rows of craft's menus and of the palette, and the Layers row, canvas selection and document tab menus, with craft-libs patch 6 ([craft-libs.md](craft-libs.md)): `Entry::row_action` opens a one-row menu on a right click of the row and `menu::take_row_action` reports it; `CommandPalette::row_action` and `PaletteOutput::row_action` do the same on a palette row. `menu_custom::shortcut::entry` sets it on the command rows, `dialogs.rs` `take_results` reads it (`shortcut::assign`), and `dialogs/palette.rs` closes the palette and opens Keyboard Shortcuts on the command.
- the mask and effects menus of the Layers panel (their command rows go through the same `entry`; Select and Mask… as well, which is a menu-catalog command with no engine command).
- toolbar tools and their flyout rows, the Layers footer buttons, the Paths panel row menu, with craft-libs patch 12: `menu::set_row_action_label` (set each pass by `menu_custom::shortcut::sync`) and `menu::context_row_action` show a one-row menu on a right click and report the tag through `take_row_action`. A toolbar button alone in its slot (Zoom) offers it on its own right click; a button with a flyout keeps craft's right click that opens the flyout, and a right click of a row in it offers the item (a tool's command is `toolSelect.<tool>`, which `shortcut::dialog_id` turns into the `tools.select.<tool>` the dialog lists). The footer's buttons stand for `layer.delete` (or the targeted mask's), `layer.new.layer`, `layer.new.group`, the mask button's command, `layer.linkLayers`, Double Exposure and Place Photo; its two menu buttons (adjustment, fx) have no single command and offer nothing. `PathsPanel::assign` does the four Paths commands.
- the options bar's tool icon: Assign Shortcut… after Reset Tool and Reset All Tools, for the active tool.

Not done on purpose: the menu bar (techneissa does not need it), the Search icon (gone from the craft title bar). `assign_shortcut.rs` and `dialogs/palette/{list,nav}.rs` stay in the tree, compiled out (they draw with the old egui widgets).

## Tests

`toolbar_assign_shortcut_tests`: a right click on the Zoom tool (alone in its slot) and on a flyout row offers Assign Shortcut… and opens Keyboard Shortcuts; the tool's id is one the dialog lists

```bash
cargo test -p photocraft-ui-egui --lib -- shortcuts_chords toolbar_assign_shortcut
```
