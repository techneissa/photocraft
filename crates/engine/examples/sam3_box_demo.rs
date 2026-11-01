//! Object Selection by dragged rectangle, Classic against SAM 3, and Select Subject with SAM 3, on
//! a picture you name: the time of each step and the selections as pictures (the photo with the
//! unselected part tinted red, 50 %) written as `<prefix>-classic.ppm`, `<prefix>-sam3.ppm` and
//! `<prefix>-subject.ppm`. Needs the SAM 3 model installed (`ArtCraft/models/sam3/`) and about
//! 20 GB of memory while the picture is encoded.
//!
//! `cargo run --release -p photocraft-engine --example sam3_box_demo -- photo.jpg out/prefix X Y W H`
//! where `X Y W H` is the dragged rectangle in document pixels.
use std::time::{Duration, Instant};

use photocraft_engine::Session;
use photocraft_engine::sam3_select::Sam3Status;
use serde_json::json;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(file), Some(prefix)) = (args.first(), args.get(1)) else {
        return Err("usage: sam3_box_demo <image> <out-prefix> X Y W H".into());
    };
    let nums: Vec<f64> = args.iter().skip(2).map(|a| a.parse::<f64>().map_err(|e| format!("{a}: {e}"))).collect::<Result<_, _>>()?;
    let [x, y, w, h] = nums[..] else { return Err("give the rectangle as X Y W H".into()) };
    let rect = [x, y, w, h];
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
    println!("{file}: {} × {} px, rectangle {rect:?}", b.width(), b.height());

    let mut classic = session("classic")?;
    let t = Instant::now();
    let r = classic.execute("select.object", json!({"rect": rect})).map_err(|e| e.to_string())?;
    println!("Classic rectangle: {} ({r})", ms(t.elapsed()));
    write_picture(&classic, &format!("{prefix}-classic.ppm"))?;

    let mut s = session("sam3")?;
    let t = Instant::now();
    s.sam3_prepare(false, photocraft_engine::sam3_select::Prepare::Force).map_err(|e| e.to_string())?;
    let status = s.sam3_wait(false, Duration::from_secs(300));
    println!("pick tool → ready (file check, load, encode): {} (status {})", ms(t.elapsed()), status.word());
    if status != Sam3Status::Ready {
        return Err(format!("SAM 3 is not ready: {status:?}"));
    }
    for run in 0..2 {
        let _ = s.execute("select.deselect", json!({}));
        let t = Instant::now();
        let r = s.execute("select.object", json!({"rect": rect})).map_err(|e| e.to_string())?;
        println!("SAM 3 rectangle (run {run}): {} ({r})", ms(t.elapsed()));
    }
    write_picture(&s, &format!("{prefix}-sam3.ppm"))?;

    // Select Subject with SAM 3 on the shared encoding, then in a fresh session (it encodes itself).
    let _ = s.execute("select.deselect", json!({}));
    let t = Instant::now();
    let r = s.execute("select.subject", json!({"engine": "sam3"})).map_err(|e| e.to_string())?;
    println!("Select Subject with SAM 3, encoding ready: {} ({r})", ms(t.elapsed()));
    drop(s);
    let mut fresh = session("classic")?;
    let t = Instant::now();
    let r = fresh.execute("select.subject", json!({"engine": "sam3"})).map_err(|e| e.to_string())?;
    println!("Select Subject with SAM 3, from nothing (load, encode, text): {} ({r})", ms(t.elapsed()));
    write_picture(&fresh, &format!("{prefix}-subject.ppm"))?;
    let _ = fresh.execute("select.deselect", json!({}));
    let t = Instant::now();
    fresh.execute("select.subject", json!({"engine": "sam3"})).map_err(|e| e.to_string())?;
    println!("Select Subject with SAM 3, second time (model loaded, picture encoded again): {}", ms(t.elapsed()));
    for engine in ["auto", "macos", "classic"] {
        let mut c = session("classic")?;
        let t = Instant::now();
        let r = c.execute("select.subject", json!({"engine": engine})).map_err(|e| e.to_string())?;
        println!("Select Subject with {engine}: {} ({r})", ms(t.elapsed()));
        write_picture(&c, &format!("{prefix}-subject-{engine}.ppm"))?;
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
