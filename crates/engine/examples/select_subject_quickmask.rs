//! Select Subject with each engine as a Quick Mask picture: the photo with the unselected part tinted
//! red (50 %), written as `<prefix>-<engine>.ppm` for each of `classic`, `macos` and `model`.
//! `cargo run --release -p photocraft-engine --example select_subject_quickmask -- photo.jpg out/prefix`
use photocraft_engine::Session;
use serde_json::json;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [file, prefix] = args.as_slice() else {
        return Err("usage: select_subject_quickmask <image> <out-prefix>".into());
    };
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let imported = photocraft_io::import("photo.jpg", &bytes).map_err(|e| format!("{file}: {e}"))?;
    for engine in ["classic", "macos", "model"] {
        let mut s = Session::new();
        s.add_document(imported.document.clone(), None);
        s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": engine})).map_err(|e| format!("{engine}: {e}"))?;
        let r = s.execute("select.subject", json!({})).map_err(|e| format!("{engine}: {e}"))?;
        let Some(d) = s.active() else { continue };
        let bounds = d.doc.bounds();
        let (w, h) = (bounds.width() as usize, bounds.height() as usize);
        let mut sel = Vec::new();
        if let Some(sel_mask) = d.doc.selection.as_ref() {
            sel_mask.read_region_into(bounds, &mut sel);
        }
        let layer = d.doc.layers.first().and_then(|l| l.surface()).ok_or("the photo has no pixel layer")?;
        let n = layer.channels();
        let px = layer.read_region(bounds);
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for i in 0..w * h {
            let keep = sel.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
            let tint = (1.0 - keep) * 0.5;
            for c in 0..3 {
                let v = px.get(i * n + c.min(n - 1)).copied().unwrap_or(0.0);
                let red = if c == 0 { 1.0 } else { 0.0 };
                ppm.push(((v * (1.0 - tint) + red * tint).clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
        let path = format!("{prefix}-{engine}.ppm");
        std::fs::write(&path, ppm).map_err(|e| format!("{path}: {e}"))?;
        let note = r.get("note").and_then(|n| n.as_str()).unwrap_or("");
        println!("{engine}: {path} {note}");
    }
    Ok(())
}
