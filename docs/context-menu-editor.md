# Context menu editor

> **Last updated:** 2026-10-29 · **Code:** `crates/ui-egui/src/menu_custom/`

Edit › Preferences › Context Menus arranges right-click menus: rows hidden, put in another order, separators placed, any live command of the menu bar added. It keeps to [context-menu-parity.md](context-menu-parity.md): rows are command ids, the menu still comes from its model, and a row that cannot run now stays greyed.

## What can be arranged

| Key | Menu | Rows can be added |
|---|---|---|
| `canvas.selection` | Canvas, the selection tools with a selection (`SELECTION_MENU`) | yes |
| `canvas.noSelection` | Canvas, the selection tools without one (`NO_SELECTION_MENU`) | yes |
| `layers.row` | Layers panel row (`layer_menu_ui::entries`); Rename Layer and Color stay at its end | yes |
| `tab.document` | Document tab | no: an added command would act on the active document, not the clicked tab |

Not yet: the Pen and path menu, the Zoom and Hand view menu, the Free Transform menu, the Type menus, the pasteboard, guide and crop menus, the layer-mask thumbnail menu, the effect rows' menu and the Brush Preset picker.

## How it works

- `Preferences::context_menus` holds `{menus: {key: {items: ["cmd:<id>", "---", …], hidden: [id, …]}}, presets: {slots: [menus|null ×3], active, undo}}`. Null (the default) arranges nothing. It is read leniently (`Custom::from_value`): an unreadable part is dropped, never an error. Agents read and set it with `prefs.get` / `prefs.set` on `contextMenus`.
- `menu_custom::apply` lays an arrangement over the rows the menu builds now. Rows that depend on state (a mask's rows, the layer kind's Rasterize row, crop and spelling rows) stay as the menu builds them; a saved row the menu does not build now is skipped until it is. A row an update adds, which the arrangement neither lists nor hides, shows after the row before it. An id no longer known is skipped. A menu left with no row shows its default rows.
- Hooks: `canvas_tool_menu::show` and `available` (`canvas_rows`, `canvas_has`), `layer_menu_ui::show` (`layer_rows`), the tab menu (`tab_rows`). A hidden row cannot be chosen through `ui.context.choose` either.
- `menus::LAYER_ROWS` lists every row the Layers panel menu can build. The test `every_layer_row_the_menu_builds_is_known_to_the_editor` fails when `entries()` builds a row the list lacks; add it there (and `TAB_ROWS` for the tab menu).
- Three presets keep whole sets of arrangements. The dropdown beside the page title switches (at once), saves to a preset, clears one and undoes the last switch. Export and Import write and read a `.json` backup (`kind` `photocraft-context-menus`, `version` 1): a file of another kind, a newer version or one that is not JSON is refused with a message and nothing changes; unknown menus and rows are skipped and named.
