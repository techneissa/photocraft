# Top bar items

## What it does

Show or hide top-bar items (Preferences ▸ Interface ▸ Top Bar, or right-click the empty part of the bar): Logo, Document Title, Workspace, Search, Appearance Mode; menus, the drag gap and window buttons always stay

## Branch

`feat/topbar-items`

## Own files

`crates/engine/src/prefs/top_bar.rs` + tests, `crates/ui-egui/src/panels/title_bar_items.rs` + tests

## Hooks in official files

engine `prefs.rs` (`interface.top_bar`); ui-egui `panels.rs`, `panels/title_bar.rs` (+16/−17: items gated, the fit logic is a call to `title_bar_items::fit`, the right-click menu on the drag gap), `prefs/prefs_ui/fields.rs` (the group), `titlebar/tests.rs` (2 tests appended)

## Tests

`title_bar_items`, `titlebar`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
