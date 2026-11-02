# Double Exposure

## What it does

Layer ▸ Double Exposure (also a Layers-panel button next to Add Mask, hidden with Preferences ▸ Interface ▸ "Double Exposure button"): the top visible, unlocked pixel or smart object layer (or the selected one; Place Photo makes smart objects, and Screen and the mask work on them as on pixel layers; text, shape, fill, adjustment and group layers don't count) gets Screen and a Reveal All mask in one undo step, the mask is targeted (also after Redo), and a click by the user switches to a soft black brush that is temporary (the saved Brush comes back). Scripts and the control channel get the layer setup only. Command `layer.doubleExposure` (`mode`: `screen` or `lighten`)

## Branch

`feat/double-exposure`

## Own files

`crates/engine/src/cmd/double_exposure.rs` and `double_exposure/tests.rs` (tests include placed smart objects, a mixed pair, hidden and locked ones), `crates/ui-egui/src/double_exposure.rs` and `double_exposure/tests.rs`

## Hooks in official files

engine `cmd.rs` (module, `specs()`), `cmd/channel_cmds.rs` + `channel_cmds/targeting.rs` (`target` made `pub(crate)`), `session/history.rs` (one call in `redo`, matched on the step label "Double Exposure"), `prefs.rs` (`interface.showDoubleExposureButton`); ui-egui `lib.rs`, `menus/invoke.rs` (3 lines), `menus/items.rs` (`PLACE_AFTER` row), `menus/render.rs` (tooltip "Needs two visible pixel or smart object layers"), `panels/layers.rs` (the button, 8 lines), `prefs/prefs_ui.rs`, `canvas/paint_mouse.rs` (1 line in `sync_tool_brush`), `icon_data.rs` (icon); `ATTRIBUTION.md` (icon row)

## Tests

`double_exposure` in engine and ui-egui

```bash
cargo test -p photocraft-engine --lib -- open_as_layers double_exposure
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
```
