# Shared models folder

## What it does

One models folder shared by the Craft apps (`~/Library/Application Support/ArtCraft/models/<model-id>/`; SAM3 moved there from LightCraft 2026-11-01, LightCraft's old path is a symlink). Lookup order: `PHOTOCRAFT_MODELS_DIR`, shared folder, old `PhotoCraft/models` (read only). A model can have several files, each with its own pinned SHA-256; new downloads go to the shared folder. Preferences ▸ AI Models and `photocraft-cli models list` show which folder a model was found in

## Branch

`feat/shared-models` (on `feat/select-subject-ai`)

## Own files

none new; changes inside `crates/ml/src/` (`store.rs`, `install.rs`, `backend.rs`, `lib.rs`)

## Hooks in official files

ui-egui `prefs/prefs_ui/ai_models.rs`; `apps/photocraft-cli/src/models.rs`; `docs/ml.md`, `ATTRIBUTION.md` (paths)

## Tests

`cargo test -p photocraft-ml --lib`; ui-egui `ai_models`; `cargo test -p photocraft-cli -- models`

```bash
cargo test -p photocraft-ml
cargo test -p photocraft-ui-egui --lib -- ai_models
cargo test -p photocraft-cli -- models
```
