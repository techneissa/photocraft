//! Object Selection by click with the real SAM 3 model (3.4 GB in `ArtCraft/models/sam3/`, about
//! 20 GB of memory while the picture is encoded). Opt-in, by hand:
//! `cargo test -p photocraft-engine --release --test sam3_real_model -- --ignored --nocapture`
use std::time::{Duration, Instant};

use photocraft_engine::Session;
use photocraft_engine::sam3_select::Sam3Status;
use serde_json::json;

/// A 640 × 480 picture: a bright red disc (centre 320, 240, radius 120) on a dark blue-grey field.
fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 640, "height": 480})).unwrap();
    s.edit("paint", |doc, a| {
        let surface = doc.layer_mut(a.unwrap()).unwrap().surface_mut().unwrap();
        surface.fill_rect(photocraft_geom::Rect::new(0, 0, 640, 480), &[0.1, 0.12, 0.2, 1.0]);
        for y in 0..480 {
            for x in 0..640 {
                let (dx, dy) = (x as f32 + 0.5 - 320.0, y as f32 + 0.5 - 240.0);
                if dx * dx + dy * dy <= 120.0 * 120.0 {
                    surface.fill_rect(photocraft_geom::Rect::new(x, y, x + 1, y + 1), &[0.9, 0.15, 0.1, 1.0]);
                }
            }
        }
        Ok(())
    })
    .unwrap();
    s
}

fn coverage(s: &Session, x: i32, y: i32) -> f32 {
    s.active().unwrap().doc.selection.as_ref().map_or(0.0, |m| m.sample_channel(x, y, 0))
}

#[test]
#[ignore = "needs the SAM 3 model and about 20 GB of memory"]
fn a_click_on_a_disc_selects_the_disc() {
    let mut s = session();
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "sam3"})).unwrap();
    let t = Instant::now();
    let status = s.sam3_prepare(false, photocraft_engine::sam3_select::Prepare::Force).unwrap();
    assert_eq!(status, Sam3Status::Preparing, "the encoding runs in the background; this returned at once");
    assert!(t.elapsed() < Duration::from_secs(1), "prepare must not block: {:?}", t.elapsed());
    let status = s.sam3_wait(false, Duration::from_secs(300));
    println!("ready after {:?} ({status:?})", t.elapsed());
    assert_eq!(status, Sam3Status::Ready);

    let t = Instant::now();
    let r = s.execute("select.object", json!({"point": [320, 240]})).unwrap();
    println!("click {:?}: {r}", t.elapsed());
    assert_eq!(r["engine"], "sam3");
    assert!(coverage(&s, 320, 240) > 0.95 && coverage(&s, 380, 260) > 0.95, "the disc is selected");
    assert!(coverage(&s, 20, 20) == 0.0 && coverage(&s, 620, 460) == 0.0, "the field is not");
    // area of the selection against the disc's: about 1
    let area: f32 = (0..480).flat_map(|y| (0..640).map(move |x| (x, y))).map(|(x, y)| coverage(&s, x, y)).sum();
    let disc = std::f32::consts::PI * 120.0 * 120.0;
    assert!((area / disc - 1.0).abs() < 0.1, "selected {area} px against the disc's {disc}");
    // one History step
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(coverage(&s, 320, 240), 0.0);
}

#[test]
#[ignore = "needs the SAM 3 model and about 20 GB of memory"]
fn a_dragged_rectangle_selects_the_disc_and_select_subject_stays_on_it() {
    let mut s = session();
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "sam3"})).unwrap();
    s.sam3_prepare(false, photocraft_engine::sam3_select::Prepare::Force).unwrap();
    assert_eq!(s.sam3_wait(false, Duration::from_secs(300)), Sam3Status::Ready);

    // a rectangle around the disc, with a margin of 10 px
    let t = Instant::now();
    let r = s.execute("select.object", json!({"rect": [190, 110, 260, 260]})).unwrap();
    println!("rectangle {:?}: {r}", t.elapsed());
    assert_eq!(r["engine"], "sam3");
    assert!(coverage(&s, 320, 240) > 0.8 && coverage(&s, 380, 260) > 0.8, "the disc is selected");
    assert!(coverage(&s, 195, 115) == 0.0 && coverage(&s, 20, 20) == 0.0, "the margin and the field are not");
    let area: f32 = (0..480).flat_map(|y| (0..640).map(move |x| (x, y))).map(|(x, y)| coverage(&s, x, y)).sum();
    let disc = std::f32::consts::PI * 120.0 * 120.0;
    assert!((area / disc - 1.0).abs() < 0.15, "selected {area} px against the disc's {disc}");
    // ⌥-drag around the same object takes it off again
    s.execute("select.object", json!({"rect": [190, 110, 260, 260], "mode": "subtract"})).unwrap();
    assert!(coverage(&s, 320, 240) < 0.05 && coverage(&s, 380, 260) < 0.05, "the object is taken off");

    // Select Subject with SAM 3 on the shared encoding: whatever it picks lies on the disc
    let _ = s.execute("select.deselect", json!({}));
    s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": "sam3"})).unwrap();
    let t = Instant::now();
    let r = s.execute("select.subject", json!({})).unwrap();
    println!("Select Subject {:?}: {r}", t.elapsed());
    assert!(coverage(&s, 20, 20) == 0.0 && coverage(&s, 620, 460) == 0.0, "the field is not the subject");
}

/// Refine Hair's Protect Subject on a portrait. The picture is the public-domain 1921 portrait in
/// `fork/refine-hair.md` (Wikimedia Commons `CoraGoffin1921.jpg`), as raw 8-bit RGB:
/// `PHOTOCRAFT_HAIR_RGB=cora.rgb PHOTOCRAFT_HAIR_SIZE=541x778 [PHOTOCRAFT_HAIR_OUT=dir]`; with the
/// folder set, the selection and both Refine Hair alphas are written there as raw 8-bit bytes.
#[test]
#[ignore = "needs the SAM 3 model, about 20 GB of memory, and the portrait (PHOTOCRAFT_HAIR_RGB, PHOTOCRAFT_HAIR_SIZE)"]
fn protect_subject_never_cuts_the_subject_and_leaves_the_strands_to_the_matting() {
    let (Ok(path), Ok(size)) = (std::env::var("PHOTOCRAFT_HAIR_RGB"), std::env::var("PHOTOCRAFT_HAIR_SIZE")) else {
        panic!("set PHOTOCRAFT_HAIR_RGB (raw 8-bit RGB) and PHOTOCRAFT_HAIR_SIZE (WxH)");
    };
    let (w, h) = size.split_once('x').map(|(a, b)| (a.parse::<i32>().unwrap(), b.parse::<i32>().unwrap())).expect("WxH");
    let rgb = std::fs::read(path).unwrap();
    assert_eq!(rgb.len(), (w * h * 3) as usize);
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h})).unwrap();
    s.edit("paint", |doc, _| {
        let bg = doc.layers[0].surface_mut().unwrap();
        let ch = bg.channels();
        let mut data = Vec::with_capacity((w * h) as usize * ch);
        for p in rgb.chunks_exact(3) {
            data.extend(p.iter().map(|v| f32::from(*v) / 255.0));
            if ch == 4 {
                data.push(1.0);
            }
        }
        bg.write_region(photocraft_geom::Rect::new(0, 0, w, h), &data);
        Ok(())
    })
    .unwrap();
    let alpha = |s: &Session| -> Vec<f32> { (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| coverage(s, x, y)).collect() };
    let dump = |name: &str, a: &[f32]| {
        if let Ok(dir) = std::env::var("PHOTOCRAFT_HAIR_OUT") {
            std::fs::write(format!("{dir}/{name}.a8"), a.iter().map(|v| (v * 255.0).round() as u8).collect::<Vec<u8>>()).unwrap();
        }
    };
    let r = s.execute("select.subject", json!({})).unwrap();
    println!("select.subject: {r}");
    dump("selection", &alpha(&s));

    // Refine Hair as before.
    let t = Instant::now();
    let r = s.execute("select.refineHair", json!({"radius": 14})).unwrap();
    println!("refine hair, Protect Subject off {:?}: {r}", t.elapsed());
    assert_eq!(r["protected"], false);
    let off = alpha(&s);
    dump("off", &off);
    s.execute("edit.undo", json!({})).unwrap();

    // With Protect Subject: the picture is encoded first (the same encoding Object Selection shares).
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "sam3"})).unwrap();
    let t = Instant::now();
    s.sam3_prepare(false, photocraft_engine::sam3_select::Prepare::Force).unwrap();
    assert_eq!(s.sam3_wait(false, Duration::from_secs(300)), Sam3Status::Ready);
    println!("SAM 3 ready after {:?}", t.elapsed());
    let t = Instant::now();
    let r = s.execute("select.refineHair", json!({"radius": 14, "protectSubject": true})).unwrap();
    println!("refine hair, Protect Subject on {:?}: {r}", t.elapsed());
    assert_eq!(r["protected"], true, "{r}");
    let on = alpha(&s);
    dump("on", &on);

    // It only ever adds: nothing the plain run kept is lost, and it does not swallow the backdrop.
    let sum = |a: &[f32]| a.iter().sum::<f32>();
    let lost = off.iter().zip(&on).filter(|(a, b)| **a > 0.5 && **b < 0.5).count();
    let gained = on.iter().zip(&off).filter(|(a, b)| **a > 0.5 && **b < 0.5).count();
    println!("selected px: off {:.0}, on {:.0}; lost {lost}, gained {gained}", sum(&off), sum(&on));
    assert!(lost <= (sum(&off) * 0.002) as usize, "{lost} px lost");
    assert!(sum(&on) >= sum(&off) - 1.0 && sum(&on) <= sum(&off) * 1.10, "the backdrop is not swallowed");
}
