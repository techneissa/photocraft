# photocraft-lama

LaMa (big-lama, "Resolution-robust Large Mask Inpainting with Fourier Convolutions") in pure Rust on [candle](https://github.com/huggingface/candle) 0.9.2: Metal on a Mac, the CPU elsewhere. Layer L4; Apache-2.0 only (see `NOTICE`). On wasm32 the crate is empty.

Taken from LightCraft's `crates/lama`. It reads the TorchScript archive `big-lama.pt` without Python (the pickle is parsed, never run) and runs the generator. The weights are never part of PhotoCraft: `photocraft-ml` finds them as `Model::Lama` (`ArtCraft/models/lama/big-lama.pt`, the file LightCraft uses too).

```rust
let model = Lama::load(path, &best_device())?;               // a second, 206 MB
let filled = model.inpaint(&rgb, &hole, w, h, &|| false)?;     // hole pixels replaced, the rest exact (up to 2048 a side)
let raw = model.predict(&rgb, &hole, w, h, &|| false)?;        // the network's whole picture, for callers that blend themselves
let out = predict_from_file(path, &rgb, &hole, w, h, &|| false)?; // load, predict, free (GPU, then CPU if the GPU fails)
```

`PHOTOCRAFT_LAMA_CPU=1` keeps it on the CPU; `PHOTOCRAFT_LAMA_PROFILE=1` prints the time of each stage.

Tests: `cargo test -p photocraft-lama` (a tiny generator with made-up weights; the `synthetic` feature gives other crates' tests the same, `synthetic::tiny_model()`). The real file, by hand and read only: `cargo test -p photocraft-lama --release --test real_model -- --ignored --nocapture`.
