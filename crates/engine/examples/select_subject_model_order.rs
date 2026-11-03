//! Select Subject with the `model` engine on a photo, the selection written as a grey PGM and the time
//! printed: with the full BiRefNet installed it runs first, without it BiRefNet-lite does (point
//! `PHOTOCRAFT_MODELS_DIR` at a folder holding only `birefnet-lite/` to see that).
//! `cargo run --release -p photocraft-engine --example select_subject_model_order -- photo.jpg out.pgm`
use photocraft_engine::Session;
use serde_json::json;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [file, out] = args.as_slice() else { return Err("usage: select_subject_model_order <image> <out.pgm>".into()) };
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let imported = photocraft_io::import("photo.jpg", &bytes).map_err(|e| format!("{file}: {e}"))?;
    let mut s = Session::new();
    s.add_document(imported.document, None);
    let t = std::time::Instant::now();
    let r = s.execute("select.subject", json!({"engine": "model"})).map_err(|e| e.to_string())?;
    println!("select.subject (model) took {:.1} s, note: {}", t.elapsed().as_secs_f64(), r.get("note").and_then(|n| n.as_str()).unwrap_or("none"));
    let d = s.active().ok_or("no document")?;
    let b = d.doc.bounds();
    let (w, h) = (b.width() as usize, b.height() as usize);
    let mut sel = Vec::new();
    if let Some(m) = d.doc.selection.as_ref() {
        m.read_region_into(b, &mut sel);
    }
    let mut pgm = format!("P5\n{w} {h}\n255\n").into_bytes();
    pgm.extend((0..w * h).map(|i| (sel.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
    std::fs::write(out, pgm).map_err(|e| format!("{out}: {e}"))
}
