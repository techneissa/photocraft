# Upstream select and mask fixes

## What it does

Four upstream fixes taken early, as real merges (nothing of ours in them): filters applied with a targeted mask change only that mask (PR #10469, issue #10457: Liquify and Camera Raw follow the mask; the lens filters and Vanishing Point refuse it); Copy and Paste keep a layer's editable mask (PR #10375, issue #2928; continues PR #2184; PR #3022 is the bigger alternative, not taken); the Polygonal Lasso's last corner of a small shape no longer closes it (PR #6734, issue #6578); Select ▸ Modify ▸ Border / Smooth / Expand / Contract / Feather show a live Preview (PR #9304, issue #8414). The Polygonal Lasso's ⇧ snap to 45° was already on main (PR #2396): only the `docs/ui-parity.md` row changed.

## Branch

`feat/upstream-select-mask` (merges of upstream `pr-10469`, `pr-10375`, `pr-6734`, `pr-9304`)

## Own files

none

## Hooks in official files

engine `cmd/channel_cmds.rs` + `channel_cmds/targeting.rs`, `cmd/distort_cmds.rs` + tests, `cmd/lens_cmds.rs`, `cmd/vp_cmds.rs`, `session/execute.rs`, `cmd/edit_cmds.rs` + `edit_cmds/clipboard_tests.rs`, `cmd/extra_cmds.rs`; ui-egui `dialogs/distort_ui.rs`, `liquify_ui.rs` + tests, `rasterize_prompt.rs` + tests, `camera_raw_ui/open.rs`, `menus/state.rs`, `app/clipboard.rs`, `lib.rs`, `mask_clipboard_tests.rs`, `canvas.rs`, `canvas/gesture.rs`, `polygon_lasso_tests.rs`, `tools/lasso_ui.rs` + tests, `dialogs/filter_dialog.rs` + tests; `docs/clipboard.md`, `docs/ui-parity.md`

## Tests

engine `distort_cmds lens_cmds channel_cmds targeting vp_cmds edit_cmds clipboard`; ui-egui `liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog`

```bash
cargo test -p photocraft-engine --lib -- distort_cmds lens_cmds channel_cmds targeting vp_cmds edit_cmds clipboard
cargo test -p photocraft-ui-egui --lib -- liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog
```

## When upstream merges

**When upstream merges #10469, #10375, #6734 or #9304:** by merge commit git sees it as merged; if squashed, the merge conflicts in the files under Own files and Hooks: keep upstream's version and do not cherry-pick again. #10375 conflicted once with main (`edit_cmds.rs`: `Clip.components: None` added to its masked-layer arm and its `copy(s, false, false)` signature)
