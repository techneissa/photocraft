# LaMa on candle (big-lama.pt)

## What it does

Remove Subject fills with LaMa from the same `big-lama.pt` LightCraft uses, run in pure Rust on candle (Metal on the Mac) by `photocraft-lama`, LightCraft's `crates/lama` (commit `05908fa4f`) copied as `crates/lama`. The model is `ArtCraft/models/lama/big-lama.pt` (205,803,670 bytes, pinned SHA-256, downloaded from `huggingface.co/fashn-ai/LaMa`); the ONNX LaMa (`lama_fp32.onnx`) is no longer downloaded or read, and the old file is removed once `big-lama.pt` is verified (see [shared-models.md](shared-models.md)). PhotoCraft's own window, detail transfer and feathering stay (`photocraft-ml`'s `inpaint.rs`); only the network call changed, to `Lama::predict` (added to the copy: the network's raw picture, which `inpaint` now composites). The model is loaded per run and freed after it; a GPU failure is retried on the CPU (`PHOTOCRAFT_LAMA_CPU=1` forces the CPU). Measured 2026-11-02 on this Mac, release build, one 15 MP photo (3912 × 3936, a 13 % hole, the whole process: decode, window, fill, detail, PNG save): ONNX path 8.45 s and 5.0 GB resident (4.7 GB footprint), candle 1.37 s and 0.78 GB resident (3.15 GB footprint, Metal buffers included); with the Mac under load 70 the ONNX path took 22 to 32 s and candle 1.4 to 5 s. The two fills differ by 0.013 of 255 on average inside the hole (largest difference 20): same weights, same result

## Branch

`feat/shared-models-lama` (on `custom`)

## Own files

`crates/lama/` (the copy: `checkpoint.rs`, `cl.rs`, `error.rs`, `fft.rs`, `layers.rs`, `net.rs`, `region.rs`, `lib.rs`, `tests.rs`; ours: `run.rs`, `synthetic.rs`, `tests/real_model.rs`, README, `Lama::predict`), `crates/ml/src/migrate.rs`

## Hooks in official files

`Cargo.toml`, `Cargo.lock`, `xtask/src/layers.rs` (`lama` before `ml` in L4), `crates/ml` (`Cargo.toml`, `inpaint.rs`, `lib.rs`, `store.rs`, `install.rs`), `crates/engine/Cargo.toml` (dev-dependency) and `cmd/remove_subject_cmds.rs` (one test), `apps/photocraft/Cargo.toml` and `main.rs` (the start-up move), `apps/photocraft-cli/src/models.rs`, ui-egui `prefs/prefs_ui/ai_models.rs` and `i18n/sv.tsv`; `docs/ml.md`, `ATTRIBUTION.md`, `NOTICE`

## Tests

```bash
cargo test -p photocraft-lama -p photocraft-ml
cargo test -p photocraft-engine --lib -- remove_subject cutout
# real model, read only, by hand: cargo test -p photocraft-lama --release --test real_model -- --ignored --nocapture
```

## When upstream merges

`crates/lama` is ours and does not clash. If upstream rewrites `photocraft-ml`'s `inpaint.rs` (it still has the ONNX call `backend::run_with(Model::Lama, …)`), keep this version: the window code is theirs, the network call is `predict`.
