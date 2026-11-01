# Refine Hair

## What it does

Refine Hair in Select and Mask (upstream issue #2671): the button builds a trimap from the selection (outer band = Edge Detection radius, inner band 40% of it, radius capped at 64 px, 0 → 10) and solves alpha in the band with closed-form matting (Levin, Lischinski, Weiss 2008, clean-room from the paper; matrix-free preconditioned conjugate gradients in 256×256 tiles in parallel), plus a local colour prior and freeing of off-band strand pixels. Command `select.refineHair`, `refineHair` edit in `select.refineEdge`'s edits, one undo step "Refine Hair". 8/16/32-bit, RGB, Gray, CMYK, Lab. Result reaches 3 radii beyond the selection, capped at 2²⁷ px (error above). Synthetic hair: band error 0.017 vs Refine Edge 0.056; 24 MP 0.1–0.2 s, worst case 1.7 s. Known limits on real photos: deep shadow at the edge can read as background (a notch), and the fringe keeps the old background's colour unless Decontaminate Colors is on

## Branch

`feat/refine-hair`

## Own files

`crates/algo/src/hair.rs`, `hair/tests.rs`, `crates/engine/src/cmd/hair_cmds.rs`, `hair_cmds/tests.rs`

## Hooks in official files

`algo/lib.rs` (mod line); `engine/cmd.rs` (mod and specs lines, placed apart from the fork's `remove_subject_cmds` line); `engine/lib.rs` (re-export); `select_mask_edits.rs` (edit variant, parse, apply, size check, tests); ui-egui `select_mask_ui.rs` (button enabled, radius scaling) and its `tests.rs`

## Tests

```bash
cargo test -p photocraft-algo --lib hair::
cargo test -p photocraft-engine --lib -- hair_cmds select_mask_edits
```
