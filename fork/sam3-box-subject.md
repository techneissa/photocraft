# SAM 3 box and Select Subject

## What it does

Object Selection's dragged rectangle selects with SAM 3 when its encoding is ready: the rectangle is SAM's box prompt (two corner points labelled 2 and 3 in the tracker's prompt encoder, single-mask output) and the main object inside it becomes the selection, combined by the drag's mode (⇧ add, ⌥ subtract, ⇧⌥ intersect, else replace); Classic's rectangle GrabCut answers when SAM 3 cannot, with the same rules and notes as the click (`fork/sam3-click-select.md`). `Sam3Service::boxed(handle, rect, extra clicks)` is the API. `tools.selectSubjectEngine` gains `sam3`: Select ▸ Subject asks SAM 3 for the text "person", then "main subject", on the Object Selection encoding when it is ready, else the job encodes the picture and frees it; "no subject" is its answer, a SAM 3 that cannot run falls back to Classic with a note. Remove Background, Remove Subject and Select and Mask's Subject button follow the same preference. Automatic is unchanged (on the NASA astronaut photo SAM 3's "person" left out the helmet, the macOS cutout kept it, and SAM 3 needs 6 to 20 s and 19 GB against 0.1 s). Swedish text for the AI Models page and SAM 3 notes (Danish, Finnish and Norwegian Bokmål only the three labels the AI Models test checks). Measured on this Mac: box 45 to 230 ms, Select Subject with SAM 3 3.3 s with the encoding ready (the text heads load once), 8.2 s from nothing in a loaded process

**On the craft UI** (official merge of 2026-11-27): the dragged rectangle already reaches the engine through `select.object`'s `rect`, so SAM 3's box prompt works with nothing more in the interface; `retouch_ui.rs` now says the fallback note in the status bar (see [sam3-click-select.md](sam3-click-select.md)). The Settings › AI Models part (the `sam3` Select Subject choice text and its page) comes with a later rebuild lane.

## Branch

`feat/sam3-box-subject` (on `custom`, which holds `feat/sam3-click-select`)

## Own files

engine-core `cmd/sam3_select/{boxed,subject,tests_box}.rs`, `examples/sam3_box_demo.rs`

## Hooks in official files

sam3 `tracker.rs` (`BoxPrompt`, `embed_prompts`), `lib.rs` (`segment_boxed`), `service.rs` (`boxed`), `service/tests.rs`, `tests/it/real_model.rs`; engine-core `cmd/sam3_select.rs` (`can_run`, `ready_encoding`, modules), `cmd/sam3_select/{backend,mask,tests}.rs`, `cmd/smartselect_cmds.rs` (`object_selection` rect path, `find_subject_region` `sam3` argument), `cmd/cutout_cmds.rs`, `cmd/remove_subject_cmds.rs`, `prefs.rs` (`SubjectEngine::Sam3`), `crates/engine/tests/it/sam3_real_model.rs`; ui-egui `tools/retouch_ui.rs` (comment), 5 `i18n/*.tsv` (`sv` 41 lines, `da`, `fi`, `nb` 3 lines each); `docs/ml.md`

## Tests

```bash
cargo test -p photocraft-sam3 --lib
cargo test -p photocraft-engine-core --lib -- sam3_select smartselect
cargo test -p photocraft-ui-egui --lib -- ai_models
# real model, by hand (about 20 GB of memory):
# cargo test -p photocraft-sam3 --release --test it real_model -- --ignored --nocapture
# cargo test -p photocraft-engine --release --test it sam3_real_model -- --ignored --nocapture
```

## After the craft UI merge (2026-11-27)

Partly works: `select.subject` with engine `sam3` and the box prompt work in the engine; tool and Settings parts are gone. Rebuild: comes back with `sam3_ui.rs` and the AI Models page.
