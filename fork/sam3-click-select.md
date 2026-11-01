# SAM 3 click select

## What it does

Object Selection and Quick Selection use SAM 3 when its model is installed and the Mac has 32 GB: a click selects the object under it (soft mask → selection alpha), ⇧-click adds a positive point and ⌥-click a negative point to the same mask, Quick Selection strokes are positive clicks (⌥ negative), the hover preview is SAM 3's. Preferences ▸ Tools ▸ Object Selection Engine: Auto (default) / SAM 3 / Classic; a fallback shows a short status note. The picture is encoded once in a background thread when the tool is picked (Esc stops it; Classic answers until it is ready) and freed on a pixel edit, document close, another document or layer, or 10 idle minutes. One encoded image at a time (the service holds one). Coverage below 0.2 → 0, above 0.8 → full. The 3.4 GB file's SHA-256 is checked once and remembered in `PhotoCraft/model-checks.tsv` (6.3 s → 3 ms). Command `select.objectPrepare` (not journaled) lets agents encode, wait or cancel. Measured on this Mac: pick → ready 6 s warm (13–22 s cold), click ~55 ms, hover ~60 ms

## Branch

`feat/sam3-click-select` (on `feat/sam3`)

## Own files

engine `cmd/sam3_select.rs` + `cmd/sam3_select/{backend,mask,tests}.rs`, `examples/sam3_click_demo.rs`, `tests/sam3_real_model.rs`; ml `marker.rs`; ui-egui `tools/sam3_ui.rs` + `tools/sam3_ui/tests.rs`

## Hooks in official files

`Cargo.toml`, `Cargo.lock`, engine `Cargo.toml`, `cmd.rs` (module + specs), `lib.rs` (field, `pub use`), `prefs.rs` (`ObjectEngine`, `tools.objectSelectionEngine`), `prefs/validate.rs`, `session/documents.rs`, `session/execute.rs` (drop on edit/close), `cmd/smartselect_cmds.rs` (`object_at_point`, `quick_selection`, `stretch_taps`); ml `store.rs` (`model_present`, marker in `hash_ok`), `lib.rs`; sam3 `service.rs` (`is_current`); ui-egui `lib.rs`, `app.rs`, `app/frame.rs` (tick), `tools.rs`, `tools/object_finder.rs` (SAM 3 hover), `tools/retouch_ui.rs` (`refine`, notes), `prefs/prefs_ui/fields.rs` + `tests.rs`; `docs/ml.md`

## Tests

```bash
cargo test -p photocraft-engine --lib -- sam3_select smartselect
cargo test -p photocraft-ml --lib
cargo test -p photocraft-sam3 --lib
cargo test -p photocraft-ui-egui --lib -- sam3_ui object_finder retouch_ui
# real model, by hand (about 20 GB of memory):
# cargo test -p photocraft-engine --test sam3_real_model -- --ignored --nocapture
# PHOTOCRAFT_SAM3_IMAGE=photo.jpg PHOTOCRAFT_SAM3_POINT=x,y cargo test -p photocraft-ui-egui --lib -- --ignored real_model --nocapture
```
