# Copy Layers to Document

## What it does

Copy Layers to Document refuses a destination with a running background job (a filter's result was thrown away when a layer was copied in meanwhile); tests for repeated copies from one document into another with edits between, and random two- and three-document sessions. Upstream PRs #10008 (smart-object files kept on copy, #9715) and #9844 (offset copies carry unlinked masks, #9684) taken early as real merges.

A test (`png_documents_take_two_row_menu_duplicates_from_another_png_with_an_edit_between`) opens PNG files with and without alpha, runs the Layers-row menu's Duplicate twice from image 2 into image 1 with a fill in image 1 between them, and checks that every layer survives, image 2 is untouched and each Undo takes one step. The row menu runs the same `layer.duplicate` dialog as the Layer menu; the context menu editor changes no command or params.

## Branch

`fix/duplicate-to-other-doc`

## Own files

none

## Hooks in official files

engine `cmd/layer_copy_cmds.rs` (9 lines in `copy_to_document`; from the PRs: +5, +17), `cmd/edit_cmds.rs` (PR #10008: `linked_sources_to_land`; paste-side mask move removed); tests in `layer_copy_cmds/tests.rs`, ui-egui `dialogs/duplicate_layer_ui.rs` (2 tests)

## Tests

engine `layer_copy_cmds smart_clipboard_sources edit_cmds clipboard`, ui-egui `duplicate_layer_ui`

```bash
cargo test -p photocraft-engine --lib -- distort_cmds lens_cmds channel_cmds targeting vp_cmds edit_cmds clipboard
cargo test -p photocraft-engine --lib -- layer_copy_cmds
cargo test -p photocraft-engine --test it -- smart_clipboard_sources
cargo test -p photocraft-ui-egui --lib -- duplicate_layer_ui
```

## When upstream merges

**When upstream merges #9844:** main's paste moved unlinked masks itself (`translate_unlinked_paste_masks` in `edit_cmds.rs`), #9844 moves them in `translate_copies`, which paste also calls; the fork dropped main's paste-side function so masks move once: keep exactly one of the two (`clipboard_tests` `…_once` catches a double move)
