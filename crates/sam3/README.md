# photocraft-sam3

SAM 3 (Segment Anything with Concepts, Meta) in pure Rust on [candle](https://github.com/huggingface/candle) 0.9.2: Metal on a Mac, the CPU elsewhere. Layer L4; Apache-2.0 only (see `NOTICE`).

Taken from LightCraft's `crates/segment`; the ViT backbone, the click decoder (SAM 2 style) and the text detector are its ports of Hugging Face Transformers. The weights are never part of PhotoCraft: `photocraft-ml` finds them as `Model::Sam3` (`ArtCraft/models/sam3/`: `model.safetensors`, `vocab.json`, `merges.txt`).

```rust
let service = Sam3Service::new();                       // nothing loads yet
let image = service.encode(&rgb)?;                      // loads the model, encodes once (seconds)
let object = service.clicks(image, &[Click { x: 0.5, y: 0.4, positive: true }])?; // Matte, ~50 ms
let sky = service.text(image, "sky", 0.5)?;             // Option<Matte>
service.unload();                                       // or Sam3Service::spawn_idle_reaper(&service)
```

Errors are `photocraft_ml::MlError` (`ModelMissing`, `LowMemory`, `BadInput`, `Backend`, `Cancelled`).
Tests: `cargo test -p photocraft-sam3 --lib` (no model); the real model:
`cargo test -p photocraft-sam3 --release --test real_model -- --ignored --nocapture`.
