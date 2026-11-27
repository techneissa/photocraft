# Transform live blend

## What it does

Free Transform, Warp, Puppet Warp and Perspective Warp show the moving layer in its place with its blend mode, opacity, fill, mask, clipping and style (a Screen layer stays Screen while you scale it); the Interpolation preference (Nearest) applies to the Free Transform preview too. This is upstream PR #8216 taken early (its commit `79370e6fc`) plus fork fixes, and since the craft UI merge it is wired in through the craft preview's own maps (see "On the craft UI" below). The Navigator draws the moving layer on a scaled-down copy of a large document (`transform_live::navigator_buffer`, from 16 MP up), not on a full CPU composite.

## Branch

`feat/transform-live-blend`

## Own files

`crates/compose/src/live.rs`, `crates/gpu/src/live.rs`, `crates/gpu/src/live.wgsl`, `crates/ui-egui/src/tools/transform_live.rs` (also `navigator_buffer`), `crates/gpu/tests/it/live_transform.rs`

## Hooks in official files

- ui-egui `canvas/display.rs` (the live document for a layer's box), `dialogs/distort_ui.rs` (`display_doc` takes `&mut` and asks `transform_live` first), `canvas/textures.rs` (`cpu_doc` in `ensure_texture`, `set_live` in `ensure_gpu`, the live damage), `canvas/navigator.rs` (`navigator_buffer`, `cpu_doc`), `canvas/gpu_canvas.rs` + `gpu_canvas/resources.rs` (`set_live`, `live_for`, the CPU fallback), `tools/transform_tool.rs` (`OpenPreview::live`, `damage` maps live keys) + `begin.rs` (starts the session; not for type) + `draw.rs` (no craft preview while live), `tools/puppet_ui.rs` and `perspective_ui.rs` (`live`, `live_key`, `live_mesh`, no craft preview while live), `tools.rs` and `lib.rs` (the module)
- tests that read the preview as a drawn mesh now read the live document: `canvas/tests.rs` (guides draw over the preview, the box's damage), `transform_undo_tests.rs` (the preview stays while panning)
- gpu `lib.rs`, `kit.rs`, `setup.rs`, `encode.rs`, `cells.rs`, `plan.rs`, `plan/types.rs`, `plan/planner.rs`; docs `docs/ui-parity.md`; ui-egui `dialogs/color_picker_preview/tests.rs` (`free_transform_keeps_its_preview_while_the_text_colour_picker_is_open` also accepts the live preview key `1 << 46` + n; upstream test from #9757)

## Tests

`live_transform` (gpu), `live` (compose), ui-egui `transform_live` (Free Transform and Warp: a Multiply layer's preview equals the commit; the keys' damage; the Navigator), `transform_tool:: puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview`

```bash
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
cargo test -p photocraft-compose --lib live
cargo test -p photocraft-ui-egui --lib -- navigator_gpu_tests transform_live
cargo test -p photocraft-gpu --test it live_transform
cargo test -p photocraft-ui-egui --lib -- liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog
cargo test -p photocraft-ui-egui --lib -- color_picker_preview
```

## When upstream merges

**When upstream merges #8216:** if by merge commit, git sees it as already merged; if squashed, the merge conflicts in the files under Own files and Hooks: keep upstream's version where it matches and do not cherry-pick #8216 again.

**Official merge of 2026-11-27:** a large layer's box now prepares in the background (`begin.rs`, `Prepared`, `on_prepared`). The live preview is decided before that: with a live session nothing is prepared (an empty image, no "above" picture) and the box opens at once. `transform_fx.rs` was deleted again (upstream had changed it); `styled` previews are off everywhere.

## On the craft UI (2026-11-27)

Free Transform, Warp, Puppet Warp and Perspective Warp show the layer in its place again. No change inside craft-libs: its transform preview draws the moving pixels as an egui mesh, which can only blend Normally, so the canvas shows the layer itself instead (the old design, rebuilt on the craft UI).

- **One map for both.** A tool describes its box once, as a craft `PreviewMap` (quad, warp mesh, pin mesh, function). `transform_live::mesh_of` builds that map's mesh (`PreviewMap::build`) and turns it into the `LiveMesh` the compositor takes. Free Transform and Warp (`box_map`), Puppet (`PuppetSession::live_mesh`) and Perspective (`PerspSession::live_mesh`) each give their map in a few lines.
- **Canvas.** `canvas/display.rs` and `dialogs/distort_ui.rs` return the document prepared for the mesh under a live key (`1 << 46` + n; `transform_live::display_doc`, `distort_display_doc`). The GPU canvas gets the mesh (`GpuCanvas::set_live`, `Resources::live`; `ensure_gpu`), draws it as the layer's content inside the normal plan and falls back to `compose::live::apply` on the CPU; the CPU canvas and the Navigator call `cpu_doc` or `navigator_buffer`. `transform_tool::damage` maps a live key to the box's own key and `transform_live::damage` redraws where the layer was and is.
- **The craft preview is skipped** while a live session runs (`show_preview`, Puppet's and Perspective's `canvas`): nothing is drawn over the canvas. Speed: the mesh is rebuilt only when the box changes, the GPU draws it in the existing composite and nothing composites the document on the CPU per frame.
- **Not live** (they keep the craft preview, drawn in Normal as before): editable type (its pixels are the text with its Warp Text), a lone mask or channel, several layers, a Free Transform Path, Content-Aware Scale, the Frame tool, a smart object's stored warp.
- **Known gaps.** The preview edge is not faded a texel as the committed layer's is (`mesh_of` takes the map as it is). Documents past the GPU canvas budget (the streamed pages of `viewport_pages.rs`) get no live mesh on that path. The Free Transform drag tests of `crates/ui-egui/tests/it/free_transform_preview_canvas.rs` fail on `step2/official-merge` with and without this change (the drag moves the box 10 px, not 60).

Pictures: `$WORKSPACE/assets/photocraft/shots/step2-transform-live/` (`before.png`, `after.png`: a Multiply layer over five colour bands under Free Transform, dark mode, app window).
