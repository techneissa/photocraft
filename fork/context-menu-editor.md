# Context menu editor

## What it does

Arrange the right-click menus (Preferences ▸ Context Menus): canvas with/without a selection, Layers row, document tab; hide, reorder, add commands and separators; 3 presets in one compact row, `.json` backup (`kind` `photocraft-context-menus`); edits wait for OK

## Branch

`feat/context-menu-editor`

## Own files

`crates/ui-egui/src/menu_custom.rs` and `menu_custom/` (`apply`, `menus`, `catalog`, `presets`, `backup`, `editor`, `presets_ui`, tests), `docs/context-menu-editor.md`

## Hooks in official files

engine-core `prefs.rs` (`context_menus`); ui-egui `lib.rs`, `canvas/canvas_tool_menu.rs` (3 small changes), `canvas/tabs.rs` (`tab_context_menu` takes the saved value), `canvas.rs` (test re-export), `panels/layer_menu_ui.rs`, `prefs/prefs_ui/fields.rs` + `dialogs.rs` (page route, draft, OK commit; merged with the AI Models section list by hand), `canvas/tests.rs`

## Tests

`menu_custom`, `canvas_tool_menu`, `layer_menu`, `tab_context` (the guard tests fail when upstream adds a row to a covered menu: add it to the editor)

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```

**Official merge of 2026-11-27:** the document tab menu has the official 8 rows with separators after Move to New Window and Open Document… (#11403). `menus.rs` `TAB_ROWS` lists them; `canvas/tabs.rs` keeps the official `tab_context_menu` (its tests call it) and the canvas calls `tab_context_menu_arranged`.

## On the craft UI (2026-11-27)

Rebuilt. `menu_custom` is compiled again (`lib.rs`).
- **Page:** Settings › Context Menus (after Interface). The page is a custom page of craft's Preferences window (`prefs/prefs_ui/custom_rows.rs` `page`, `fields.rs` `ContextMenusPage`, `dialogs.rs`). The draft is one string, `contextMenus.arrangement` (the arrangement as JSON text, empty for none), in the window's document, so OK, Apply and Cancel work as for every setting; `write_edits` stores it with `menu_custom::store`. Export and Import ask through `menu_custom::Asks`, which `dialogs::take` runs after the pass. The editor draws with `Pal` (craft theme tokens) and its own glyphs; the menu picker and search box are craft's `Select` and `TextField`.
- **Menus:** Layers row (`panels/layers_tree.rs` `row_menu`: `menu_custom::layer_rows`), canvas selection menus (`canvas_tool_menu.rs`: `canvas_rows`, `canvas_has`, `closed`) and the document tab menu (`canvas/tabs.rs` `tab_menu`: `tab_rows`) follow the saved arrangement. The Layers guard list gained the group rows New Artboard and Artboard from Group (`menus.rs`).
- Not arranged (as before): Pen and path, view, transform, type, pasteboard, guide and crop menus, the layer mask thumbnail and effects menus, and craft's Paths panel menu.
- The old page-harness tests are gone with the old dialog (UI wiring); the pure tests, the guard tests and one test that a hidden and a reordered row show in the real Layers menu (`a_hidden_and_a_reordered_row_show_up_that_way_in_the_layers_menu`) stay.
