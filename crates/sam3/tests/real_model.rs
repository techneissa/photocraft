//! The real model on this computer: load it from the models folder `photocraft-ml` finds, encode a
//! small fixed image once, and ask for a mask from a click and from a text prompt. Ignored by
//! default (it needs the 3.4 GB checkpoint and takes seconds of GPU time):
//!
//! ```sh
//! /usr/bin/time -l cargo test -p photocraft-sam3 --release --test real_model -- --ignored --nocapture
//! ```
//!
//! It prints the time to find, load and encode, the time per click and per text prompt, and (with
//! `/usr/bin/time -l`) the peak memory of the whole process. A computer without the model fails it
//! with the message that says where to put the files.

use std::time::Instant;

use photocraft_ml::Rgb;
use photocraft_sam3::{Click, Matte, Sam3Service};

const W: usize = 320;
const H: usize = 240;

/// A fixed scene: a sky-blue backdrop fading to green ground, and a red disc (centre (160, 130),
/// radius 60) with a lighter patch off-centre.
fn scene() -> Rgb {
    let mut px = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f32 - 160.0, y as f32 - 130.0);
            let rgb = if dx * dx + dy * dy <= 60.0 * 60.0 {
                if (dx + 20.0).powi(2) + (dy + 20.0).powi(2) < 144.0 { [0.9, 0.35, 0.3] } else { [0.8, 0.1, 0.1] }
            } else if y < 150 {
                [0.35 + 0.001 * y as f32, 0.6, 0.9]
            } else {
                [0.25, 0.5 + 0.001 * x as f32, 0.2]
            };
            px.extend_from_slice(&rgb);
        }
    }
    Rgb { w: W, h: H, px }
}

/// Fraction of the disc the matte covers, and fraction of the matte inside the disc.
fn overlap(m: &Matte) -> (f32, f32) {
    let (mut disc, mut hit, mut set) = (0u32, 0u32, 0u32);
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f32 - 160.0, y as f32 - 130.0);
            let inside = dx * dx + dy * dy <= 60.0 * 60.0;
            let on = m.alpha[y * W + x] > 0.5;
            disc += u32::from(inside);
            hit += u32::from(inside && on);
            set += u32::from(on);
        }
    }
    (hit as f32 / disc as f32, hit as f32 / set.max(1) as f32)
}

#[test]
#[ignore = "needs the SAM 3 model files in the models folder"]
fn click_and_text_masks_from_the_real_model() {
    let service = Sam3Service::new();
    let t = Instant::now();
    assert!(service.is_installed(), "SAM 3 is not installed: {:?}", service.preload());
    eprintln!("find + verify the files (SHA-256 of 3.4 GB, once per process): {:?}", t.elapsed());

    let t = Instant::now();
    service.preload().expect("load");
    eprintln!("load: {:?} (device: {})", t.elapsed(), if service.is_loaded() { "loaded" } else { "?" });

    let img = scene();
    // The first encode compiles the GPU kernels; the later ones show the steady time.
    let mut handle = None;
    for run in 0..3 {
        let t = Instant::now();
        handle = Some(service.encode(&img).expect("encode"));
        eprintln!("encode {W}×{H} (run {run}): {:?}", t.elapsed());
    }
    let handle = handle.expect("handle");

    let click = Click { x: 160.0 / W as f32, y: 130.0 / H as f32, positive: true };
    let mut matte = None;
    for run in 0..3 {
        let t = Instant::now();
        matte = Some(service.clicks(handle, &[click]).expect("click"));
        eprintln!("one click (run {run}): {:?}", t.elapsed());
    }
    let matte = matte.expect("matte");
    let (covered, precision) = overlap(&matte);
    eprintln!("click mask: score {:.2}, covers {:.0}% of the disc, {:.0}% of the mask is the disc", matte.score, covered * 100.0, precision * 100.0);
    assert_eq!((matte.w, matte.h, matte.alpha.len()), (W, H, W * H));
    assert!(matte.alpha[130 * W + 160] > 0.5, "the clicked pixel is in the mask");
    assert!(matte.alpha[5 * W + 5] < 0.5, "a far corner is not");
    assert!(covered > 0.8 && precision > 0.8, "the mask is the disc: {covered} / {precision}");

    // a negative click next to the first keeps the answer (and takes a further, two-click path)
    let outside = Click { x: 20.0 / W as f32, y: 200.0 / H as f32, positive: false };
    let t = Instant::now();
    let two = service.clicks(handle, &[click, outside]).expect("two clicks");
    eprintln!("two clicks: {:?} (score {:.2})", t.elapsed(), two.score);
    assert!(two.alpha[130 * W + 160] > 0.5);

    let t = Instant::now();
    let text = service.text(handle, "red ball", 0.5).expect("text");
    eprintln!("text prompt (first: loads the text encoder and detector): {:?}", t.elapsed());
    let t = Instant::now();
    let text2 = service.text(handle, "sky", 0.5).expect("text");
    eprintln!("text prompt (second): {:?}", t.elapsed());
    for (name, m) in [("red ball", &text), ("sky", &text2)] {
        match m {
            Some(m) => eprintln!("text {name:?}: score {:.2}, mask pixels {}", m.score, m.alpha.iter().filter(|a| **a > 0.5).count()),
            None => eprintln!("text {name:?}: nothing found"),
        }
    }

    // a stale handle is refused, and freeing the model really frees it
    service.release(handle);
    assert!(service.clicks(handle, &[click]).is_err());
    service.unload();
    assert!(!service.is_loaded());
    // and it loads again on the next request
    let again = service.encode(&img).expect("encode after unload");
    assert!(service.clicks(again, &[click]).expect("click after unload").alpha[130 * W + 160] > 0.5);
    service.unload();
}
