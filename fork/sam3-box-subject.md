# SAM 3 box and Select Subject

## What it does

Object Selection's dragged rectangle selects with SAM 3 when its encoding is ready: the rectangle is SAM's box prompt (two corner points labelled 2 and 3 in the tracker's prompt encoder, single-mask output) and the main object inside it becomes the selection, combined by the drag's mode (⇧ add, ⌥ subtract, ⇧⌥ intersect, else replace); Classic's rectangle GrabCut answers when SAM 3 cannot, with the same rules and notes as the click (`fork/sam3-click-select.md`). `Sam3Service::boxed(handle, rect, extra clicks)` is the API. `tools.selectSubjectEngine` gains `sam3`: Select ▸ Subject asks SAM 3 for the text "person", then "main subject", on the Object Selection encoding when it is ready, else the job encodes the picture and frees it; "no subject" is its answer, a SAM 3 that cannot run falls back to Classic with a note. Remove Background, Remove Subject and Select and Mask's Subject button follow the same preference. Automatic is unchanged (on the NASA astronaut photo SAM 3's "person" left out the helmet, the macOS cutout kept it, and SAM 3 needs 6 to 20 s and 19 GB against 0.1 s). Swedish text for the AI Models page and SAM 3 notes (Danish, Finnish and Norwegian Bokmål only the three labels the AI Models test checks). Measured on this Mac: box 45 to 230 ms, Select Subject with SAM 3 3.3 s with the encoding ready (the text heads load once), 8.2 s from nothing in a loaded process

## Branch

`feat/sam3-box-subject` (on `custom`, which holds `feat/sam3-click-select`)

## Own files

engine `cmd/sam3_select/{boxed,subject,tests_box}.rs`, `examples/sam3_box_demo.rs`

## Hooks in official files

sam3 `tracker.rs` (`BoxPrompt`, `embed_prompts`), `lib.rs` (`segment_boxed`), `service.rs` (`boxed`), `service/tests.rs`, `tests/real_model.rs`; engine `cmd/sam3_select.rs` (`can_run`, `ready_encoding`, modules), `cmd/sam3_select/{backend,mask,tests}.rs`, `cmd/smartselect_cmds.rs` (`object_selection` rect path, `find_subject_region` `sam3` argument), `cmd/cutout_cmds.rs`, `cmd/remove_subject_cmds.rs`, `prefs.rs` (`SubjectEngine::Sam3`), `tests/sam3_real_model.rs`; ui-egui `tools/retouch_ui.rs` (comment), 5 `i18n/*.tsv` (`sv` 41 lines, `da`, `fi`, `nb` 3 lines each); `docs/ml.md`

## Tests

```bash
cargo test -p photocraft-sam3 --lib
cargo test -p photocraft-engine --lib -- sam3_select smartselect
cargo test -p photocraft-ui-egui --lib -- ai_models
# real model, by hand (about 20 GB of memory):
# cargo test -p photocraft-sam3 --release --test real_model -- --ignored --nocapture
# cargo test -p photocraft-engine --release --test sam3_real_model -- --ignored --nocapture
```
