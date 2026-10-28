//! Select Subject with each engine (`tools.selectSubjectEngine`) on pictures you name: the time of
//! the command (sampling, the cutout or Classic, storing the selection) and how much of the picture
//! it selected.
//! `cargo run --release -p photocraft-engine --example bench_select_subject_engines -- photo.jpg [more.jpg]`
use std::time::Instant;

use photocraft_engine::Session;
use photocraft_geom::Rect;
use serde_json::json;

fn main() -> Result<(), String> {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        return Err("usage: bench_select_subject_engines <image>...".into());
    }
    for file in &files {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let name = std::path::Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or("image");
        let imported = photocraft_io::import(name, &bytes).map_err(|e| format!("{file}: {e}"))?;
        for engine in std::env::var("ENGINES").unwrap_or_else(|_| "classic,macos,model".into()).split(',') {
            let mut s = Session::new();
            s.add_document(imported.document.clone(), None);
            s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": engine})).map_err(|e| format!("{engine}: {e}"))?;
            let bounds = s.active().map(|d| d.doc.bounds()).unwrap_or(Rect::EMPTY);
            let t = Instant::now();
            let r = s.execute("select.subject", json!({})).map_err(|e| format!("{engine}: {e}"))?;
            let took = t.elapsed();
            let mut covered = 0.0f64;
            if let Some(sel) = s.active().and_then(|d| d.doc.selection.as_ref()) {
                let mut v = Vec::new();
                sel.read_region_into(bounds, &mut v);
                covered = v.iter().map(|x| f64::from(*x)).sum::<f64>() / (f64::from(bounds.width()) * f64::from(bounds.height()));
            }
            let mp = f64::from(bounds.width()) * f64::from(bounds.height()) / 1e6;
            let note = r.get("note").and_then(|n| n.as_str()).unwrap_or("");
            println!(
                "{name} {}×{} ({mp:.1} MP)  {engine:>7}: {took:.2?}  selected {:.1} %  now {} MB resident  {note}",
                bounds.width(),
                bounds.height(),
                covered * 100.0,
                resident_mb()
            );
        }
    }
    Ok(())
}

/// This process's resident memory in MB now (`ps`; 0 where there is none), to see what stays after a run.
fn resident_mb() -> u64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.trim().parse::<u64>().ok()).map_or(0, |kb| kb / 1024)
}
