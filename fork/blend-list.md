# Blend list height and wheel

## What it does

Blend mode lists as tall as the window allows (no fixed 14-row cap, upstream issue #7261; upstream PR #7622 is stale and not taken), and Preferences ▸ Interface ▸ "Scroll Wheel Changes Blend Mode" (off by default: the wheel over a blend box scrolls instead of stepping the mode; on = Photoshop's way)

## Branch

`feat/blend-list`

## Own files

`crates/ui-egui/src/blend_wheel.rs` and `blend_wheel/tests.rs`

## Hooks in official files

`widgets/dropdown.rs` (`list_height()`, one `let list` per call site), engine `prefs.rs` (`interface.scroll_wheel_changes_blend_mode`), ui-egui `lib.rs`, `prefs/prefs_ui/save.rs` (1 line), `panels/layers.rs` (2), `panels/options_bar.rs` (1), `panels/brush_controls.rs` (2): `dropdown_wheel_hovered` → `blend_wheel::dropdown`; official tests `blend_dropdown_wheel_tests.rs`, `options_bar_mode_wheel_tests.rs` (helpers `pub(crate)`, preference on)

## Tests

`blend_wheel`, `blend_dropdown`, `options_bar_mode_wheel`

```bash
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
```
