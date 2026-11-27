# Upstream select and mask fixes

## What it does

Four upstream fixes taken early, as real merges (nothing of ours in them): filters applied with a targeted mask change only that mask (PR #10469, issue #10457: Liquify and Camera Raw follow the mask; the lens filters and Vanishing Point refuse it); Copy and Paste keep a layer's editable mask (PR #10375, issue #2928; continues PR #2184; PR #3022 is the bigger alternative, not taken); the Polygonal Lasso's last corner of a small shape no longer closes it (PR #6734, issue #6578); Select ▸ Modify ▸ Border / Smooth / Expand / Contract / Feather show a live Preview (PR #9304, issue #8414). The Polygonal Lasso's ⇧ snap to 45° was already on main (PR #2396): only the `docs/ui-parity.md` row changed.

## Branch

`feat/upstream-select-mask` (merges of upstream `pr-10469`, `pr-10375`, `pr-6734`, `pr-9304`)

## Own files

none

## Hooks in official files

engine-core `cmd/channel_cmds.rs` + `channel_cmds/targeting.rs`, `cmd/distort_cmds.rs` + tests, `cmd/lens_cmds.rs`, `cmd/vp_cmds.rs`, `session/execute.rs`, `cmd/edit_cmds.rs` + `edit_cmds/clipboard_tests.rs`, `cmd/extra_cmds.rs`; ui-egui `dialogs/distort_ui.rs`, `liquify_ui.rs` + tests, `rasterize_prompt.rs` + tests, `camera_raw_ui/open.rs`, `menus/state.rs`, `app/clipboard.rs`, `lib.rs`, `mask_clipboard_tests.rs`, `canvas.rs`, `canvas/gesture.rs`, `polygon_lasso_tests.rs`, `tools/lasso_ui.rs` + tests, `dialogs/filter_dialog.rs` + tests; on the craft UI also `dialogs/liquify_ui.rs` (`open`), `camera_raw_ui/open.rs`, `canvas/document_display.rs`; `docs/clipboard.md`, `docs/ui-parity.md`

## Tests

engine-core `distort_cmds lens_cmds channel_cmds targeting vp_cmds edit_cmds clipboard`; ui-egui `liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog`

```bash
cargo test -p photocraft-engine-core --lib -- distort_cmds lens_cmds channel_cmds targeting vp_cmds edit_cmds clipboard
cargo test -p photocraft-ui-egui --lib -- liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog
```

## When upstream merges

**When upstream merges #10469, #10375, #6734 or #9304:** by merge commit git sees it as merged; if squashed, the merge conflicts in the files under Own files and Hooks: keep upstream's version and do not cherry-pick again. #10375 conflicted once with main (`edit_cmds.rs`: `Clip.components: None` added to its masked-layer arm and its `copy(s, false, false)` signature)

## State on the craft UI (2026-11-27)

Checked against the official code (`origin/main`) and the branch, one fix at a time:

| Fix | Official code | On the craft UI |
|---|---|---|
| Polygonal Lasso: the last corner of a small shape no longer closes it (PR #6734) | in the official code since `6aa956346` (merged 2026-11-07) | nothing of ours; `polygon_lasso` and `lasso_ui` tests pass (measured) |
| Filters with a targeted mask change only that mask (PR #10469, issue #10457) | not merged (PR open) | engine part was in the tree; the interface part is back: Liquify and Camera Raw stay greyed on an adjustment layer's mask (`distort_ui::dialog_can_open`, `menus/state.rs`, Liquify and Camera Raw `open`), a type layer's targeted mask asks no rasterize (`rasterize_prompt::filter_needs`); tests `liquify_and_camera_raw_grey_out_where_their_dialogs_cannot_open`, `a_type_layers_targeted_mask_never_asks_to_rasterize` (both fail with the `menus/state.rs` hook off: measured) |
| Copy and Paste keep a layer's editable mask (PR #10375, issue #2928) | not merged (PR open) | engine part was in the tree; the interface part is back: `app/clipboard.rs` (bounded export to the system bitmap with its status messages, the every-byte signature, the `export_os_clipboard_with_test_budget` test hook), and `mask_clipboard_tests` (6 tests) is compiled in again |
| Select › Modify previews the modified selection (PR #9304, issue #8414) | not merged (PR open) | `filter_dialog::PREVIEWED` lists the five commands again, and the canvas's selection outline (`canvas/document_display.rs` `selection_controls`) draws the preview's selection on the proxy, `reduction(k)`; test `select_modify_previews_the_modified_selection` |

The ⇧ snap of the Polygonal Lasso was already on main (PR #2396).

Two changes so the official interface works, kept from the merge: Lens Correction and Adaptive Wide Angle no longer refuse in their own `enabled` (the target check in `channel_cmds/targeting.rs` still greys them, and Vanishing Point, on a targeted mask or channel: measured in the menu for a mask, an alpha channel and the Quick Mask), and a Liquify or Puppet Warp call that names a pixel layer (`"layer"`, as the official Liquify workspace sends) edits that layer, not the targeted mask or channel.
