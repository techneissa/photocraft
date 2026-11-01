//! Object Selection by click, Classic against SAM 3, on a picture you name: the time of each step
//! and the selection as a picture (the photo with the unselected part tinted red, 50 %) written as
//! `<prefix>-classic.ppm`, `<prefix>-sam3.ppm` and, after a ⇧ and a ⌥ click, `<prefix>-sam3-refined.ppm`.
//! Needs the SAM 3 model installed (`ArtCraft/models/sam3/`) and about 20 GB of memory while the
//! picture is encoded.
//!
//! `cargo run --release -p photocraft-engine --example sam3_click_demo -- photo.jpg out/prefix X Y [X2 Y2 [X3 Y3]]`
//! where `X Y` is the click, `X2 Y2` the ⇧ click (adds a positive point), `X3 Y3` the ⌥ click.
use std::time::{Duration, Instant};

use photocraft_engine::Session;
use photocraft_engine::sam3_select::Sam3Status;
use serde_json::json;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(file), Some(prefix)) = (args.first(), args.get(1)) else {
        return Err("usage: sam3_click_demo <image> <out-prefix> X Y [X2 Y2 [X3 Y3]]".into());
    };
    let nums: Vec<f64> = args.iter().skip(2).map(|a| a.parse::<f64>().map_err(|e| format!("{a}: {e}"))).collect::<Result<_, _>>()?;
    let pt = |i: usize| nums.get(2 * i).zip(nums.get(2 * i + 1)).map(|(x, y)| [*x, *y]);
    let click = pt(0).ok_or("give the click as X Y")?;
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let imported = photocraft_io::import("photo.jpg", &bytes).map_err(|e| format!("{file}: {e}"))?;
    let session = |engine: &str| -> Result<Session, String> {
        let mut s = Session::new();
        s.add_document(imported.document.clone(), None);
        s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": engine})).map_err(|e| e.to_string())?;
        Ok(s)
    };
    let ms = |t: Duration| format!("{:.0} ms", t.as_secs_f64() * 1000.0);
    let b = imported.document.bounds();
    println!("{file}: {} × {} px, click {click:?}", b.width(), b.height());

    let mut classic = session("classic")?;
    let t = Instant::now();
    let r = classic.execute("select.object", json!({"point": click})).map_err(|e| e.to_string())?;
    println!("Classic click: {} ({r})", ms(t.elapsed()));
    write_picture(&classic, &format!("{prefix}-classic.ppm"))?;

    let mut s = session("sam3")?;
    let t = Instant::now();
    let status = s.sam3_prepare(false, photocraft_engine::sam3_select::Prepare::Force).map_err(|e| e.to_string())?;
    println!("pick tool → returns: {} (status {})", ms(t.elapsed()), status.word());
    let t = Instant::now();
    let status = s.sam3_wait(false, Duration::from_secs(300));
    println!("pick tool → ready (first time in this process: file check, load, encode): {} (status {})", ms(t.elapsed()), status.word());
    if status != Sam3Status::Ready {
        return Err(format!("SAM 3 is not ready: {status:?}"));
    }
    let hover = s.sam3_hover_source(false).ok_or("no hover source")?;
    for _ in 0..2 {
        let t = Instant::now();
        let region = hover.find(click[0] as f32, click[1] as f32).map_err(|e| e.to_string())?;
        println!("hover lookup: {} ({})", ms(t.elapsed()), region.map_or("nothing".into(), |r| format!("{} × {} px object", r.bbox.width(), r.bbox.height())));
    }
    let t = Instant::now();
    let r = s.execute("select.object", json!({"point": click})).map_err(|e| e.to_string())?;
    println!("SAM 3 click: {} ({r})", ms(t.elapsed()));
    write_picture(&s, &format!("{prefix}-sam3.ppm"))?;
    if let Some(p) = pt(1) {
        let t = Instant::now();
        s.execute("select.object", json!({"point": p, "mode": "add", "refine": "add"})).map_err(|e| e.to_string())?;
        println!("⇧ click (positive point): {}", ms(t.elapsed()));
    }
    if let Some(p) = pt(2) {
        let t = Instant::now();
        s.execute("select.object", json!({"point": p, "mode": "subtract", "refine": "subtract"})).map_err(|e| e.to_string())?;
        println!("⌥ click (negative point): {}", ms(t.elapsed()));
    }
    if pt(1).is_some() || pt(2).is_some() {
        write_picture(&s, &format!("{prefix}-sam3-refined.ppm"))?;
    }
    Ok(())
}

/// The photo with the unselected part tinted red (50 %), as a PPM.
fn write_picture(s: &Session, path: &str) -> Result<(), String> {
    let d = s.active().ok_or("no document")?;
    let bounds = d.doc.bounds();
    let (w, h) = (bounds.width() as usize, bounds.height() as usize);
    let mut sel = Vec::new();
    if let Some(m) = d.doc.selection.as_ref() {
        m.read_region_into(bounds, &mut sel);
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
    std::fs::write(path, ppm).map_err(|e| format!("{path}: {e}"))?;
    println!("wrote {path}");
    Ok(())
}
