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

    // a rectangle around the disc, with a margin
    let t = Instant::now();
    let r = s.execute("select.object", json!({"rect": [160, 80, 320, 320]})).unwrap();
    println!("rectangle {:?}: {r}", t.elapsed());
    assert_eq!(r["engine"], "sam3");
    assert!(coverage(&s, 320, 240) > 0.95 && coverage(&s, 380, 260) > 0.95, "the disc is selected");
    assert!(coverage(&s, 170, 90) == 0.0 && coverage(&s, 20, 20) == 0.0, "the margin and the field are not");
    let area: f32 = (0..480).flat_map(|y| (0..640).map(move |x| (x, y))).map(|(x, y)| coverage(&s, x, y)).sum();
    let disc = std::f32::consts::PI * 120.0 * 120.0;
    assert!((area / disc - 1.0).abs() < 0.1, "selected {area} px against the disc's {disc}");
    // ⌥-drag over the disc's right half takes it off again
    s.execute("select.object", json!({"rect": [320, 80, 200, 320], "mode": "subtract"})).unwrap();
    assert!(coverage(&s, 260, 240) > 0.95 && coverage(&s, 380, 240) < 0.05, "only the left half stays");

    // Select Subject with SAM 3 on the shared encoding: whatever it picks lies on the disc
    s.execute("select.deselect", json!({})).unwrap();
    s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": "sam3"})).unwrap();
    let t = Instant::now();
    let r = s.execute("select.subject", json!({})).unwrap();
    println!("Select Subject {:?}: {r}", t.elapsed());
    assert!(coverage(&s, 20, 20) == 0.0 && coverage(&s, 620, 460) == 0.0, "the field is not the subject");
}
