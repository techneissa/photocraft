# Refine Hair

## What it does

Refine Hair in Select and Mask (upstream issue #2671): the button builds a trimap from the selection (outer band = Edge Detection radius, inner band 40% of it, radius capped at 64 px, 0 → 10) and solves alpha in the band with closed-form matting (Levin, Lischinski, Weiss 2008, clean-room from the paper; matrix-free preconditioned conjugate gradients in 256×256 tiles in parallel), plus a local colour prior and freeing of off-band strand pixels. Command `select.refineHair`, `refineHair` edit in `select.refineEdge`'s edits, one undo step "Refine Hair". 8/16/32-bit, RGB, Gray, CMYK, Lab. Result reaches 3 radii beyond the selection, capped at 2²⁷ px (error above). Synthetic hair: band error 0.017 vs Refine Edge 0.056; 24 MP 0.1–0.2 s, worst case 1.7 s. Known limits on real photos: the fringe keeps the old background's colour unless Decontaminate Colors is on; a deep shadow at the edge can read as background (a notch under the nose in the 1921 portrait), which Protect Subject (below) fixes.

**Protect Subject** (branch `feat/dup-note-protect-subject`): a checkbox next to the Refine Hair button in Select and Mask, ticked by default where SAM 3 is installed and the Mac has the memory (greyed out, with a tooltip, otherwise or when Object Selection's engine is Classic); its state is remembered in the preferences (`select.refineEdge.protectSubject`). On: Refine Hair asks SAM 3 for the subject inside the current selection's bounds (the box prompt of Object Selection's rectangle, on the encoding Object Selection shares), pulls its mask in by 15% of the band radius (1 to 4 px) and makes it sure foreground wherever the trimap would leave it to the matting; those pixels stay fully selected (the solver may free a sure pixel whose colour is far from its neighbours'), strands outside the mask are solved as before. Off: today's Refine Hair exactly. SAM 3 not ready: the button waits (the status bar says "Preparing SAM 3 for this image… (Esc stops it)", OK is refused meanwhile) and adds the edit once the encoding is ready; Esc stops the run and adds nothing; an encoding that fails or goes away adds Refine Hair without Protect Subject, with the reason in the status bar. The preview works on a reduced copy: the finder takes the box in the copy's pixels and shrinks SAM's mask to them. The edit and the command carry `protectSubject` (`select.refineHair` also returns `protected`; with no ready encoding it runs as off, so scripts call `select.objectPrepare` with `wait` first). On the portrait (SAM 3 ready after 17 s, then 0.13 s a run) the notch under the nose is cut away with it off and kept with it on; 238 px gained, 9 lost. Unit tests never load the model (`Session::sam3_backend_installed`)

## Branch

`feat/refine-hair`

## Own files

`crates/algo/src/hair.rs`, `hair/tests.rs`, `crates/engine/src/cmd/hair_cmds.rs`, `hair_cmds/tests.rs`; Protect Subject: engine `cmd/sam3_select/{protect,tests_protect}.rs`, ui-egui `dialogs/select_mask_ui/protect.rs` and `protect/tests.rs`

## Hooks in official files

`algo/lib.rs` (mod line); `algo/hair.rs` (`refine_hair_protected`, `protect_erosion`; `refine_hair` calls it with `None`); `engine/cmd.rs` (mod and specs lines, placed apart from the fork's `remove_subject_cmds` line); `engine/lib.rs` (re-export); `select_mask_edits.rs` (edit variant, parse, apply, size check, tests); ui-egui `select_mask_ui.rs` (button enabled, radius scaling; Protect Subject: `protect` module line, a field, the checkbox, `job` takes `&mut` and a finder, Esc and OK while waiting) and its `tests.rs`; engine `sam3_select.rs` (module lines, `sam3_backend_installed`), `smartselect_cmds.rs` (`attach` in `refine_edge`), `hair_cmds.rs` (`protectSubject`), `select_mask_edits.rs` (`protect` field, `attach`)

## Tests

```bash
cargo test -p photocraft-algo --lib hair::
cargo test -p photocraft-engine --lib -- hair_cmds select_mask_edits sam3_select
cargo test -p photocraft-ui-egui --lib -- select_mask_ui
# real model, by hand (about 20 GB of memory; the portrait as raw RGB, see $WORKSPACE/assets/photocraft/shots/refine-hair/SOURCE.txt):
# PHOTOCRAFT_HAIR_RGB=cora.rgb PHOTOCRAFT_HAIR_SIZE=541x778 cargo test -p photocraft-engine --test sam3_real_model -- --ignored --nocapture protect_subject
```
