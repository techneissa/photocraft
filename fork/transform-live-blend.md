# Transform live blend

## What it does

Free Transform, Warp, Puppet Warp and Perspective Warp show the moving layer in its place with its blend mode, opacity, fill, mask, clipping and style (a Screen layer stays Screen while you scale it); the Interpolation preference (Nearest) applies to the Free Transform preview too. This is upstream PR #8216 taken early (a real merge of its commit `79370e6fc`, ported onto main's split files) plus fork fixes. Hooks are larger than the fork usually allows because this is upstream's own code

## Branch

`feat/transform-live-blend`

## Own files

`crates/compose/src/live.rs`, `crates/gpu/src/live.rs`, `crates/gpu/src/live.wgsl`, `crates/ui-egui/src/tools/transform_live.rs`, `crates/gpu/tests/it/live_transform.rs`

## Hooks in official files

- ui-egui `dialogs/color_picker_preview/tests.rs` (`free_transform_keeps_its_preview_while_the_text_colour_picker_is_open` also accepts the live preview key `1 << 46` + n; upstream test from #9757)

gpu `lib.rs`, `kit.rs`, `setup.rs`, `encode.rs`, `cells.rs` (also a `get` for a pool slot), `plan.rs`, `plan/types.rs`, `plan/planner.rs`; ui-egui `canvas/display.rs`, `navigator.rs`, `textures.rs`, `paint.rs`, `gpu_canvas.rs`, `gpu_canvas/resources.rs`, `tools/transform_tool.rs` + `begin.rs` + `draw.rs` + `tests.rs`, `puppet_ui.rs`, `perspective_ui.rs`, `distort_ui.rs`, `lib.rs`, `canvas/tests.rs`; deleted `transform_fx.rs` and `transform_fx_tests.rs` (as the PR does); `docs/ui-parity.md`

## Tests

`live_transform` (gpu), `live` (compose), ui-egui `transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview`

```bash
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
cargo test -p photocraft-compose --lib live
cargo test -p photocraft-gpu --test it live_transform
cargo test -p photocraft-ui-egui --lib -- liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog
cargo test -p photocraft-ui-egui --lib -- color_picker_preview
```

## When upstream merges

**When upstream merges #8216:** if by merge commit, git sees it as already merged; if squashed, the merge conflicts in the files under Own files and Hooks: keep upstream's version where it matches and do not cherry-pick #8216 again.
