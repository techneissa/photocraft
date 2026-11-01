//! No model needed: input checks, errors for missing and damaged files, the idle rule.

use super::*;

fn gray(w: usize, h: usize) -> Rgb {
    Rgb { w, h, px: vec![0.5; w * h * 3] }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("photocraft-sam3-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn bad_images_are_bad_input() {
    let s = Sam3Service::with_dir("/nonexistent/sam3");
    assert!(matches!(s.encode(&Rgb { w: 0, h: 0, px: vec![] }), Err(MlError::BadInput(_))));
    assert!(matches!(s.encode(&Rgb { w: 4, h: 4, px: vec![0.0; 5] }), Err(MlError::BadInput(_))));
    assert!(matches!(s.encode(&Rgb { w: MAX_SIDE + 1, h: 1, px: vec![] }), Err(MlError::BadInput(_))));
    // a size whose sample count overflows is refused, not allocated
    assert!(matches!(s.encode(&Rgb { w: usize::MAX, h: 2, px: vec![] }), Err(MlError::BadInput(_))));
    assert!(!s.is_loaded());
}

#[test]
fn a_missing_model_is_reported_with_where_it_looked() {
    let s = Sam3Service::with_dir("/nonexistent/sam3");
    assert!(!s.is_installed());
    match s.encode(&gray(4, 4)) {
        Err(MlError::ModelMissing { model, path }) => {
            assert_eq!(model, Model::Sam3);
            assert!(path.contains("/nonexistent/sam3"), "{path}");
        }
        other => panic!("{other:?}"),
    }
    assert!(!s.is_busy() && !s.is_loaded());
    assert!(matches!(s.preload(), Err(MlError::ModelMissing { .. })));
}

#[test]
fn damaged_model_files_are_an_error_not_a_crash() {
    let dir = temp("damaged");
    for f in ["model.safetensors", "vocab.json", "merges.txt"] {
        std::fs::write(dir.join(f), b"not a model").expect("write");
    }
    let s = Sam3Service::with_dir(&dir);
    assert!(s.is_installed());
    assert!(matches!(s.encode(&gray(4, 4)), Err(MlError::Backend(_))));
    assert!(!s.is_loaded(), "a model that did not load is not loaded");
    // a safetensors header that lies about its size
    let mut bytes = 1000u64.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"{}");
    std::fs::write(dir.join("model.safetensors"), bytes).expect("write");
    assert!(matches!(s.preload(), Err(MlError::Backend(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_handle_the_service_does_not_hold_is_refused_before_any_load() {
    let s = Sam3Service::with_dir("/nonexistent/sam3");
    let stale = ImageHandle { id: 99, w: 4, h: 4 };
    let click = Click { x: 0.5, y: 0.5, positive: true };
    assert!(matches!(s.clicks(stale, &[click]), Err(MlError::BadInput(m)) if m.contains("encode it again")));
    assert!(matches!(s.text(stale, "sky", 0.5), Err(MlError::BadInput(_))));
    s.release(stale);
}

#[test]
fn clicks_and_prompts_are_checked() {
    let s = Sam3Service::with_dir("/nonexistent/sam3");
    let h = ImageHandle { id: 1, w: 2, h: 2 };
    assert!(matches!(s.clicks(h, &[]), Err(MlError::BadInput(m)) if m == "no clicks"));
    assert!(matches!(s.clicks(h, &[Click { x: f32::NAN, y: 0.0, positive: true }]), Err(MlError::BadInput(_))));
    assert!(matches!(s.text(h, "   ", 0.5), Err(MlError::BadInput(m)) if m == "no text"));
    assert!(matches!(s.text(h, "sky", f32::NAN), Err(MlError::BadInput(_))));
}

#[test]
fn unload_and_idle_with_nothing_loaded_do_nothing() {
    let s = Sam3Service::with_dir("/nonexistent/sam3");
    assert!(!s.unload_if_idle(Duration::ZERO));
    s.unload();
    assert!(!s.is_loaded() && !s.is_busy());
}

#[test]
fn eight_bit_conversion_clamps_and_rounds() {
    let img = Rgb { w: 2, h: 1, px: vec![0.0, 1.0, 0.5, -3.0, 7.0, f32::NAN] };
    assert_eq!(to_u8(&img), vec![0, 255, 128, 0, 255, 0]);
}

#[test]
fn a_matte_is_the_logits_resampled_to_the_image() {
    let p = Prediction { logits: vec![10.0; MASK_SIDE * MASK_SIDE], score: 0.9 };
    let m = Matte::from_prediction(&p, 5, 3);
    assert_eq!((m.w, m.h, m.alpha.len(), m.score), (5, 3, 15, 0.9));
    assert!(m.alpha.iter().all(|a| *a > 0.999));
    let none = Matte::from_prediction(&Prediction { logits: vec![-10.0; MASK_SIDE * MASK_SIDE], score: 0.1 }, 4, 4);
    assert!(none.alpha.iter().all(|a| *a < 0.001));
}
