# Open as Layers

## What it does

Open as Layers: a `.pcstack` file (from LightCraft's Edit In ▸ Open as Layers in PhotoCraft) opens its photos as the layers of one new document, top layer first, smaller ones centred, all converted to the top file's mode, depth and profile; also the command `file.openAsLayers`. Runs as a background job (Esc cancels). At most 256 files and 2^29 pixels in all (a file past that is skipped and named). Interface: [open-as-layers-interface.md](open-as-layers-interface.md)

## Branch

`feat/open-as-layers`

## Own files

`crates/engine-core/src/cmd/file_cmds/open_as_layers.rs` and `open_as_layers/tests.rs`, `crates/ui-egui/src/open_as_layers.rs` (the manifest reader, the File ▸ Open as Layers… picker and the result note) and `open_as_layers/tests.rs`

## Hooks in official files

engine-core `cmd/file_cmds.rs` (module), `cmd.rs` (`specs()` in the command list); ui-egui `lib.rs` (module), `file_open.rs` (3 lines in `open_path_placed`: a `.pcstack` opens its photos as layers), `file_dialog.rs` (`ask_open_paths`: the several-file picker, paths in the order returned), `jobs_ui.rs` (2 lines in `on_event`: the finished job's note), `menu_catalog.rs` (File ▸ Open as Layers…), `menus/view_cmds/dispatch.rs` (`wraps`) and `dialogs.rs` (null params open the picker); `packaging/macos/Info.plist.in` (a `pcstack` document type); `fork/i18n/*.tsv` (the menu label)

## Tests

`open_as_layers` in engine and ui-egui

```bash
cargo test -p photocraft-engine-core --lib -- open_as_layers double_exposure
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
```

## After the craft UI merge (2026-11-27)

Works again (`feat/step2-layers-transform`): File ▸ Open as Layers… opens the system picker with several files (the first one the picker returns is the top layer) and opens them as one document; a `.pcstack` file opens the same way. The manifest and result tests are back. A `.pcstack` dropped on the canvas was not checked on the craft UI. On macOS the menu bar is the system's, so the item shows there, not inside the window.
