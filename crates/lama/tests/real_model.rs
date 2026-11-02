//! The real `big-lama.pt`, read only. By hand (it needs the 206 MB file, which tests never download):
//! `cargo test -p photocraft-lama --release --test real_model -- --ignored --nocapture`
//! The file is `PHOTOCRAFT_LAMA_FILE` when set, else the one the Craft apps share
//! (`~/Library/Application Support/ArtCraft/models/lama/big-lama.pt`). Nothing is written.
#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::time::Instant;

use photocraft_lama::{Lama, best_device};

fn model_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PHOTOCRAFT_LAMA_FILE").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/ArtCraft/models/lama/big-lama.pt"))
}

/// A smooth diagonal ramp with a fine texture, 512 × 512, and a hole in the middle.
fn picture() -> (Vec<f32>, Vec<f32>) {
    let n = 512usize;
    let mut rgb = Vec::with_capacity(n * n * 3);
    let mut hole = vec![0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            let t = (x + y) as f32 / (2 * n) as f32;
            let grain = (((x * 31 + y * 17) % 13) as f32 - 6.0) * 0.004;
            rgb.extend([0.2 + 0.5 * t + grain, 0.6 - 0.3 * t + grain, 0.3 + 0.2 * t + grain]);
            if (180..340).contains(&x) && (160..360).contains(&y) {
                hole[y * n + x] = 1.0;
            }
        }
    }
    (rgb, hole)
}

#[test]
#[ignore = "needs big-lama.pt (206 MB)"]
fn the_real_model_fills_a_hole_in_a_smooth_picture() {
    let path = model_file().expect("no home folder");
    assert!(path.is_file(), "{} is not there", path.display());
    let started = Instant::now();
    let model = Lama::load(&path, &best_device()).expect("load big-lama.pt");
    println!("load: {:.2} s", started.elapsed().as_secs_f64());
    let (rgb, hole) = picture();
    let started = Instant::now();
    let out = model.inpaint(&rgb, &hole, 512, 512, &|| false).expect("fill");
    println!("fill 512×512: {:.2} s", started.elapsed().as_secs_f64());
    assert!(out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    // The ramp continues through the hole: the middle of it is close to the ramp's own value.
    let n = 512usize;
    let mid = (260 * n + 260) * 3;
    let t = (260 + 260) as f32 / (2 * n) as f32;
    let want = [0.2 + 0.5 * t, 0.6 - 0.3 * t, 0.3 + 0.2 * t];
    for (c, w) in want.iter().enumerate() {
        assert!((out[mid + c] - w).abs() < 0.12, "channel {c}: {} vs {w}", out[mid + c]);
    }
    // And outside the hole nothing moved.
    assert_eq!(&out[..3], &rgb[..3]);
}
