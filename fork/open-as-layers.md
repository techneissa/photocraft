# Open as Layers

## What it does

Open as Layers: a `.pcstack` file (from LightCraft's Edit In ▸ Open as Layers in PhotoCraft) opens its photos as the layers of one new document, top layer first, smaller ones centred, all converted to the top file's mode, depth and profile; also the command `file.openAsLayers`. Runs as a background job (Esc cancels). At most 256 files and 2^29 pixels in all (a file past that is skipped and named). Interface: [open-as-layers-interface.md](open-as-layers-interface.md)

## Branch

`feat/open-as-layers`

## Own files

`crates/engine/src/cmd/file_cmds/open_as_layers.rs` and `open_as_layers/tests.rs`, `crates/ui-egui/src/open_as_layers.rs` and `open_as_layers/tests.rs`

## Hooks in official files

engine `cmd/file_cmds.rs` (module), `cmd.rs` (`specs()` in the command list); ui-egui `lib.rs` (module), `file_open.rs` (3 lines in `open_path`; `opens_in_place` replaces `is_preset_file` at its 3 call sites so a dropped `.pcstack` is never placed; one test at the end of `file_open/tests.rs`), `jobs_ui.rs` (6 lines in `on_event`: the finished job's notice); `packaging/macos/Info.plist.in` (a `pcstack` document type)

## Tests

`open_as_layers` in engine and ui-egui

```bash
cargo test -p photocraft-engine --lib -- open_as_layers double_exposure
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
```
