//! Times SAM 3 on this computer and, under `/usr/bin/time -l`, shows the peak memory up to a stage:
//!
//! ```sh
//! cargo build -p photocraft-sam3 --release --example sam3_probe
//! /usr/bin/time -l target/release/examples/sam3_probe load     # the model in memory
//! /usr/bin/time -l target/release/examples/sam3_probe click    # + one image encoded + one click
//! /usr/bin/time -l target/release/examples/sam3_probe text     # + a text prompt
//! ```
//!
//! The image is a fixed synthetic one (1200 × 800, a gradient); the first run includes compiling
//! the GPU kernels, so each stage runs `RUNS` times. The model comes from the models folder
//! `photocraft-ml` finds, or from `PHOTOCRAFT_MODELS_DIR`.

use std::time::Instant;

use photocraft_ml::Rgb;
use photocraft_sam3::{Click, Sam3Service};

const RUNS: usize = 3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = std::env::args().nth(1).unwrap_or_else(|| "text".into());
    if !["load", "click", "text"].contains(&stage.as_str()) {
        return Err("usage: sam3_probe load|click|text".into());
    }
    let service = Sam3Service::new();
    let t = Instant::now();
    service.preload()?;
    eprintln!("find, verify and load: {:?}", t.elapsed());
    if stage == "load" {
        return Ok(());
    }
    let (w, h) = (1200, 800);
    let px: Vec<f32> = (0..w * h * 3).map(|i| ((i / 3 % w) as f32) / w as f32).collect();
    let img = Rgb::new(w, h, px)?;
    let click = Click { x: 0.5, y: 0.5, positive: true };
    for run in 0..RUNS {
        let t = Instant::now();
        let handle = service.encode(&img)?;
        let e = t.elapsed();
        let t = Instant::now();
        let m = service.clicks(handle, &[click])?;
        eprintln!("run {run}: encode {w}×{h} {e:?}, click {:?} (score {:.2})", t.elapsed(), m.score);
        if stage == "text" {
            let t = Instant::now();
            let found = service.text(handle, "sky", 0.5)?;
            eprintln!("run {run}: text {:?} ({})", t.elapsed(), found.is_some());
        }
    }
    Ok(())
}
