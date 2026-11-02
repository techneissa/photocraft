# Shared models folder

## What it does

One models folder shared by the Craft apps (`~/Library/Application Support/ArtCraft/models/<model-id>/`; SAM3 moved there from LightCraft 2026-11-01, LightCraft's old path is a symlink). Lookup order: `PHOTOCRAFT_MODELS_DIR`, shared folder, old `PhotoCraft/models`. A model can have several files, each with its own pinned SHA-256; new downloads go to the shared folder. Preferences ▸ AI Models (a Folder line per model) and `photocraft-cli models list` show which folder a model was found in. **Moved once at start** (the app on a thread, `photocraft-cli models`; `crates/ml/src/migrate.rs`; never while `PHOTOCRAFT_MODELS_DIR` is set): each file of the old folder goes to `ArtCraft/models/<model-id>/` by rename (BiRefNet-lite to `birefnet-lite/`, its own folder, not LightCraft's full `birefnet/`) after its size and SHA-256 check; a verified copy already in the shared folder is never touched and only the old duplicate is removed (a corrupt shared copy is replaced by the verified old one); a failed move leaves both files, logs why, and the app starts; `lama_fp32.onnx` is removed once `big-lama.pt` is verified (see [lama.md](lama.md)); the old empty `.lock` files and the old folder go when empty. On techneissa's Mac the move runs at the first start of the new app

## Branch

`feat/shared-models` (on `feat/select-subject-ai`), `feat/shared-models-lama` (the move and LaMa's file)

## Own files

`crates/ml/src/migrate.rs`; changes inside `crates/ml/src/` (`store.rs`, `install.rs`, `backend.rs`, `lib.rs`)

## Hooks in official files

ui-egui `prefs/prefs_ui/ai_models.rs`; `apps/photocraft-cli/src/models.rs`; `apps/photocraft/src/main.rs` (the start-up move) and its `Cargo.toml`; `docs/ml.md`, `ATTRIBUTION.md` (paths)

## Tests

`cargo test -p photocraft-ml --lib`; ui-egui `ai_models`; `cargo test -p photocraft-cli -- models`

```bash
cargo test -p photocraft-ml
cargo test -p photocraft-ui-egui --lib -- ai_models
cargo test -p photocraft-cli -- models
```
