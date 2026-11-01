use super::*;
use photocraft_doc::Document;
use photocraft_geom::Rect;

const W: i32 = 120;
const H: i32 = 80;

/// Colours (dark head, light background) in the channel layout of `mode`, `n` samples a pixel.
fn colours(mode: &str, n: usize) -> ([f32; 5], [f32; 5]) {
    let (dark, light) = match mode {
        "gray" => ([0.1, 0.0, 0.0, 0.0, 0.0], [0.9, 0.0, 0.0, 0.0, 0.0]),
        "cmyk" => ([0.5, 0.6, 0.8, 0.6, 0.0], [0.1, 0.05, 0.0, 0.0, 0.0]),
        _ => ([0.15, 0.1, 0.05, 0.0, 0.0], [0.8, 0.85, 0.9, 0.0, 0.0]),
    };
    let alpha = |mut c: [f32; 5]| {
        let colour_channels = if mode == "gray" {
            1
        } else if mode == "cmyk" {
            4
        } else {
            3
        };
        if n > colour_channels {
            c[colour_channels] = 1.0;
        }
        c
    };
    (alpha(dark), alpha(light))
}

/// A dark head (x < 50) with a one-pixel strand leaving it at y = 40 (x 50–79) on a light
/// background, in `mode` at `depth`, with the head selected.
fn session(mode: &str, depth: u32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": W, "height": H, "mode": mode, "depth": depth})).unwrap();
    s.edit("paint", |doc, _| {
        let bg = doc.layers[0].surface_mut().unwrap();
        let n = bg.channels();
        let (dark, light) = colours(mode, n);
        bg.fill_rect(Rect::new(0, 0, W, H), &light[..n]);
        bg.fill_rect(Rect::new(0, 0, 50, H), &dark[..n]);
        bg.fill_rect(Rect::new(50, 40, 80, 41), &dark[..n]);
        Ok(())
    })
    .unwrap();
    s.execute("select.rect", json!({"x": 0, "y": 0, "width": 50, "height": H})).unwrap();
    s
}

fn cov(s: &Session, x: i32, y: i32) -> f32 {
    s.active().unwrap().doc.selection.as_ref().map_or(0.0, |m| m.sample_channel(x, y, 0))
}

fn mask_cov(s: &Session, x: i32, y: i32) -> f32 {
    let d = s.active().unwrap();
    d.doc.layer(d.active_layer.unwrap()).unwrap().mask.as_ref().map_or(0.0, |m| m.surface.sample_channel(x, y, 0))
}

fn assert_strand_found(c: impl Fn(i32, i32) -> f32, what: &str) {
    for x in 52..70 {
        assert!(c(x, 40) >= 0.6, "{what}: strand pixel ({x}, 40) has coverage {}", c(x, 40));
        assert!(c(x, 46) <= 0.15, "{what}: background ({x}, 46) has coverage {}", c(x, 46));
        assert!(c(x, 34) <= 0.15, "{what}: background ({x}, 34) has coverage {}", c(x, 34));
    }
    assert!(c(10, 40) >= 0.99 && c(10, 5) >= 0.99, "{what}: sure foreground");
    assert!(c(100, 40) <= 0.01 && c(100, 70) <= 0.01, "{what}: sure background");
}

#[test]
fn follows_a_thin_strand_the_selection_missed_at_every_depth_and_undoes() {
    for mode in ["rgb", "gray"] {
        for depth in [8, 16, 32] {
            let mut s = session(mode, depth);
            assert!(s.is_enabled(CMD));
            let before = s.active().unwrap().doc.selection.clone();
            // The rectangle holds none of it.
            assert!(cov(&s, 60, 40) == 0.0);
            let steps = s.active().unwrap().history.past_len();
            let r = s.execute(CMD, json!({"radius": 12})).unwrap_or_else(|e| panic!("{mode} {depth}: {e}"));
            assert_eq!(r["output"], "selection");
            assert_eq!(r["selected"], true);
            assert_eq!(r["radius"], 12.0);
            assert_strand_found(|x, y| cov(&s, x, y), &format!("{mode} {depth}-bit"));
            // One history step, undone in one.
            assert_eq!(s.active().unwrap().history.past_len(), steps + 1);
            assert_eq!(s.active().unwrap().history.undo_label(), Some("Refine Hair"));
            s.execute("edit.undo", json!({})).unwrap();
            assert_eq!(s.active().unwrap().doc.selection, before, "{mode} {depth}");
        }
    }
}

#[test]
fn runs_in_every_colour_mode() {
    for mode in ["rgb", "gray", "cmyk", "lab"] {
        let mut s = session(mode, 8);
        s.execute(CMD, json!({"radius": 12})).unwrap_or_else(|e| panic!("{mode}: {e}"));
        assert!(cov(&s, 10, 40) >= 0.99, "{mode}");
        assert!(cov(&s, 100, 40) <= 0.01, "{mode}");
    }
}

#[test]
fn the_edit_in_select_and_mask_does_the_same() {
    let mut a = session("rgb", 8);
    a.execute(CMD, json!({"radius": 12})).unwrap();
    let mut b = session("rgb", 8);
    let steps = b.active().unwrap().history.past_len();
    b.execute("select.refineEdge", json!({"edits": [{"tool": "refineHair", "radius": 12}]})).unwrap();
    assert_eq!(b.active().unwrap().history.past_len(), steps + 1);
    assert_strand_found(|x, y| cov(&b, x, y), "edit");
    for (x, y) in [(55, 40), (60, 40), (65, 40), (60, 44), (45, 40), (52, 52)] {
        assert!((cov(&a, x, y) - cov(&b, x, y)).abs() <= 1.0 / 255.0, "({x}, {y})");
    }
    // After another edit the band is solved again from the selection as it then is.
    b.execute("edit.undo", json!({})).unwrap();
    b.execute("select.refineEdge", json!({"edits": [{"tool": "refineHair"}, {"tool": "invert"}]})).unwrap();
    assert!(cov(&b, 100, 40) >= 0.99 && cov(&b, 10, 40) <= 0.01);
    // Refine Hair with nothing selected leaves nothing selected.
    let mut empty = session("rgb", 8);
    empty.execute("select.deselect", json!({})).unwrap();
    let r = empty.execute("select.refineEdge", json!({"edits": [{"tool": "refineHair"}], "radius": 4}));
    assert!(r.is_err() || empty.active().unwrap().doc.selection.is_none());
}

#[test]
fn refines_the_layer_mask_when_there_is_no_selection() {
    let mut s = session("rgb", 8);
    s.execute("layer.layerMask.revealSelection", json!({})).unwrap();
    assert!(s.active().unwrap().doc.selection.is_none());
    assert!(s.is_enabled(CMD));
    let r = s.execute(CMD, json!({"radius": 12})).unwrap();
    assert_eq!(r["output"], "layerMask");
    assert_strand_found(|x, y| mask_cov(&s, x, y), "layer mask");
    s.execute("edit.undo", json!({})).unwrap();
    assert!(mask_cov(&s, 60, 40) == 0.0);
}

#[test]
fn needs_a_selection_or_mask_and_rejects_bad_params_without_changes() {
    let mut s = Session::new();
    assert!(!s.is_enabled(CMD));
    s.execute("file.new", json!({"width": 32, "height": 32})).unwrap();
    assert!(!s.is_enabled(CMD));
    assert!(s.execute(CMD, json!({})).is_err());
    let mut s = session("rgb", 8);
    let before = s.active().unwrap().doc.selection.clone();
    let steps = s.active().unwrap().history.past_len();
    for p in [
        json!({"radius": -1}),
        json!({"radius": 1001}),
        json!({"radius": 1e300}),
        json!({"radius": "wide"}),
        json!({"radius": [1]}),
        json!({"sampleAllLayers": "yes"}),
        json!({"target": "channel"}),
        json!({"target": 3}),
        // Not an object: the dispatcher refuses it.
        json!(7),
        json!("x"),
        json!([]),
    ] {
        assert!(s.execute(CMD, p.clone()).is_err(), "{p}");
        assert_eq!(s.active().unwrap().doc.selection, before, "{p}");
        assert_eq!(s.active().unwrap().history.past_len(), steps, "{p}");
    }
    for p in [json!(null), json!({}), json!({"radius": null}), json!({"radius": 0}), json!({"radius": 500}), json!({"target": "mask"})] {
        s.execute(CMD, p.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
        s.execute("edit.undo", json!({})).unwrap();
    }
}

/// A selection of a huge canvas is refused up front, and a small one on it still works.
#[test]
fn refuses_what_is_too_large_and_keeps_the_selection() {
    let mut s = Session::new();
    s.add_document(Document::new("huge", photocraft_doc::Size::new(100_000, 100_000), photocraft_doc::ColorMode::Rgb, photocraft_color::SampleType::U8), None);
    s.execute("select.rect", json!({"x": 10, "y": 60, "width": 10, "height": 10})).unwrap();
    s.execute(CMD, json!({"radius": 4})).unwrap();
    s.execute("edit.undo", json!({})).unwrap();
    s.execute("select.all", json!({})).unwrap();
    let all = s.active().and_then(|d| d.doc.selection.clone()).expect("selected");
    let t = std::time::Instant::now();
    let e = s.execute(CMD, json!({"radius": 4})).expect_err("too large");
    assert!(e.to_string().contains("too large for Refine Hair"), "{e}");
    assert!(s.active().and_then(|d| d.doc.selection.as_ref()) == Some(&all));
    // The same through Select and Mask's edit, before anything is computed.
    let e = s.execute("select.refineEdge", json!({"edits": [{"tool": "refineHair"}]})).expect_err("too large");
    assert!(e.to_string().contains("too large for Refine Hair"), "{e}");
    assert!(s.active().and_then(|d| d.doc.selection.as_ref()) == Some(&all));
    assert!(t.elapsed() < std::time::Duration::from_secs(20), "{:?}", t.elapsed());
}
