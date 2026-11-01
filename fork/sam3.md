# SAM 3

## What it does

SAM 3 (Segment Anything with Concepts) in pure Rust on candle, Metal on the Mac: click and text masks, no UI yet. Taken from LightCraft's `crates/segment` (`eda19667`). Model id `sam3` in `ArtCraft/models/sam3/` (3 files, no public download: AI Models says where to put them, no Download or Remove button; `models fetch --model all` skips it). 32 GB total-RAM check (peak 20 GB while an image is encoded). API: `Sam3Service` (encode once, clicks, text, unload / idle). Measured on this Mac (load 40–60): file check 7–25 s per process, load 0.5–1.4 s, encode ~5.3 s (+7 s GPU compile the first time), click ~41 ms, text 0.7 s (2.5 s first)

## Branch

`feat/sam3` (on `feat/shared-models`)

## Own files

`crates/sam3/` (`src/service.rs` + tests, `tests/real_model.rs`, `examples/sam3_probe.rs`, README)

## Hooks in official files

- `ml` `store.rs` (`Model::Sam3`, `downloadable()`), `install.rs`, `ram.rs`
- ui-egui `prefs/prefs_ui/ai_models.rs`; cli `models.rs`; `xtask/src/layers.rs` (`ml` → `sam3` inside L4); `Cargo.lock`
- `docs/ml.md`, `ATTRIBUTION.md`, `NOTICE`

## Tests

```bash
cargo test -p photocraft-sam3 --lib
cargo test -p photocraft-ml --lib
# real model, by hand: cargo test -p photocraft-sam3 --release --test real_model -- --ignored --nocapture
```
