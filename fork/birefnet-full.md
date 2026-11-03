# Full BiRefNet

## What it does

Select Subject, Remove Background and Remove Subject's subject step use the full BiRefNet (Swin-L, 1024 px) when it is installed, in front of BiRefNet-lite. It is LightCraft's `crates/birefnet` (commit `8807a2bca`, pure Rust on candle, Metal on the Mac) copied as `crates/birefnet` (`photocraft-birefnet`, MIT), without its download code. The model is LightCraft's own file, `ArtCraft/models/birefnet/model.safetensors` (444,473,596 bytes, SHA-256 `9ab37426bf4de0567af6b5d21b16151357149139362e6e8992021b8ce356a154`, from `huggingface.co/ZhengPeng7/BiRefNet` at the revision LightCraft pins): never a second copy, and Preferences ▸ AI Models › Download fetches that same file into that same folder. `Model::Birefnet` in `photocraft-ml`; BiRefNet-lite keeps its own folder, `birefnet-lite/`.

Engine order: `auto` is full BiRefNet when installed → the macOS cutout → BiRefNet-lite when installed → Classic. `model` is full BiRefNet when installed → BiRefNet-lite when installed → Classic. `macos`, `sam3` and `classic` are unchanged. The full model's matte goes through the same bilinear upscale and guided-filter edge refinement as BiRefNet-lite's. A full model that fails to load or run (an error, or a panic in the GPU code, caught), or a computer with under 24 GB of memory, never stops the command: the next finder takes over and the status bar says so ("Full AI model failed, used another engine" when a later engine found the subject, the existing "AI model failed, used Classic" when Classic did). Esc and a mask over the output budget still end the command. The model is loaded for the run and freed after it, one run at a time. `used` is `model-full` (`remove_subject`'s `subjectEngine` shows it). The Select Subject cache treats installing the full model as a change.

Preferences ▸ AI Models gets a row "Subject Segmentation (Full BiRefNet)" (Installed with its folder, Download, Remove for Every ArtCraft App? since the file is LightCraft's too); the BiRefNet-lite row stays. `photocraft-cli models fetch --model birefnet-full` fetches it (`all` now includes it). The migration of the old `PhotoCraft/models` folder skips it (it was never there).

Measured 2026-11-03 on this Mac (48 GB, Metal, release build, `/usr/bin/time -l` on `try_models segment` and `segment-full`, one 12 MP photo of a fox, 4200 × 2800, the whole process: decode, matte, edge refinement, PNG save; the Mac under load 60 to 85 from other work, so both times are inflated): full BiRefNet 15.5 s and 17.1 s, 1.4 GB resident, 15.3 GB peak footprint (GPU buffers included); BiRefNet-lite 62.6 s and 38.9 s, 7.6 to 8.8 GB resident, 8.4 to 8.8 GB footprint. Through the app's own chain (`select_subject_model_order`, `select.subject` with `model`, same photo, load about 200): 12.9 s and 12.7 GB footprint with the full model installed, 56.1 s and 8.7 GB with only BiRefNet-lite in the folder; the selections equal the two mattes. The full model is around 13 to 17 s for a 12 MP photo on this loaded Mac, over 10 s. Not tuned. The memory floor is 24 GB (Metal gives a process about three quarters of the memory); BiRefNet-lite keeps its 12 GB floor.

## Branch

`feat/birefnet-full-model` (on `custom`)

## Own files

`crates/birefnet/` (the copy: `blocks.rs`, `combine.rs`, `deform.rs`, `image.rs`, `model.rs`, `params.rs`, `swin.rs`, `weights.rs`; ours: `lib.rs` and `run.rs` edited, `INPUT_SIDE`, `matte_with`), `crates/ml/src/segment_full.rs`, engine `cmd/subject_full.rs` and `cmd/subject_full/tests.rs`, `crates/engine/examples/select_subject_model_order.rs`

## Hooks in official files

`Cargo.toml`, `Cargo.lock`, `xtask/src/layers.rs` (`birefnet` before `ml` in L4); `crates/ml` (`Cargo.toml`, `lib.rs`, `store.rs` (`Model::Birefnet`), `ram.rs`, `segment.rs` (`refine_edges` is `pub(crate)`), `migrate.rs`, `examples/try_models.rs`); engine `cmd.rs` (mod line), `cmd/smartselect_cmds.rs` (`find_subject_region`, `model_subject_with`, the cache's `model_available` is a pair) and its `tests.rs`; ui-egui `prefs/prefs_ui/ai_models.rs` (`title`, `powers`); `apps/photocraft-cli` (`args.rs`, `models.rs`); `docs/ml.md`, `ATTRIBUTION.md`, `NOTICE`

## Tests

```bash
cargo test -p photocraft-birefnet -p photocraft-ml --lib
cargo test -p photocraft-engine --lib -- subject_full full_model full_birefnet subject_cache_invalidates
cargo test -p photocraft-ui-egui --lib -- ai_models
cargo test -p photocraft-cli -- models
# real model, by hand (15 GB): cargo run --release -p photocraft-ml --example try_models -- segment-full photo.jpg matte.png
# the app's chain: cargo run --release -p photocraft-engine --example select_subject_model_order -- photo.jpg out.pgm
```

## When upstream merges

`crates/birefnet` is ours and does not clash. If upstream adds its own full BiRefNet, keep the shared file `birefnet/model.safetensors`; the `Model::ALL` order is `Birefnet`, `BirefnetLite`, `Lama`, `Sam3`.
