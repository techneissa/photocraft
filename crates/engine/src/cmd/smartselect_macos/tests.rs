use super::*;
use crate::Session;
use photocraft_algo::segment::SurfaceSampler;
use serde_json::json;
use std::cell::Cell;

type Pixels = Box<dyn Fn(u32, u32) -> [f32; 3]>;

/// A document whose background shows `px(x, y)` (RGB, 0–1).
fn session_with(w: u32, h: u32, px: Pixels) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "depth": 8})).unwrap();
    s.edit("paint", |doc, _| {
        let bg = doc.layers[0].surface_mut().unwrap();
        let n = bg.channels();
        let mut data = Vec::with_capacity((w * h) as usize * n);
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&px(x, y)[..n.min(3)]);
                if n == 4 {
                    data.push(1.0);
                }
            }
        }
        bg.write_region(Rect::new(0, 0, w as i32, h as i32), &data);
        Ok(())
    })
    .unwrap();
    s
}

/// A busy two-colour picture whose Classic subject is a disc at (100, 80), radius 45.
fn disc_session() -> Session {
    session_with(
        200,
        160,
        Box::new(|x, y| {
            let (dx, dy) = (x as f32 + 0.5 - 100.0, y as f32 + 0.5 - 80.0);
            if dx * dx + dy * dy <= 45.0 * 45.0 { [0.9, 0.3, 0.2] } else { [0.25, 0.45, 0.7] }
        }),
    )
}

fn cov(s: &Session, x: i32, y: i32) -> f32 {
    let mut v = Vec::new();
    let Some(sel) = s.active().unwrap().doc.selection.as_ref() else { return 0.0 };
    sel.read_region_into(Rect::new(x, y, x + 1, y + 1), &mut v);
    v.first().copied().unwrap_or(0.0)
}

/// A cutout that selects the left quarter of whatever it is given, with a soft one-pixel edge.
fn left_quarter(_rgba: &[u8], w: u32, h: u32) -> Result<Mask, Error> {
    let edge = w / 4;
    let data = (0..w * h)
        .map(|i| {
            if i % w < edge {
                255
            } else if i % w == edge {
                128
            } else {
                0
            }
        })
        .collect();
    Ok(Mask { width: w, height: h, data })
}
fn no_subject(_: &[u8], _: u32, _: u32) -> Result<Mask, Error> {
    Err(Error::NoSubject)
}
fn broken(_: &[u8], _: u32, _: u32) -> Result<Mask, Error> {
    Err(Error::Os("boom".into()))
}
fn empty_mask(_: &[u8], w: u32, h: u32) -> Result<Mask, Error> {
    Ok(Mask { width: w, height: h, data: vec![0; (w * h) as usize] })
}

/// Run `f` with `cut` as the machine's cutout (none: a system without one).
fn with_cutout<R>(cut: Option<CutoutFn>, f: impl FnOnce() -> R) -> R {
    TEST_CUTOUT.with(|c| c.set(cut));
    let r = f();
    TEST_CUTOUT.with(|c| c.set(None));
    r
}

fn set_engine(s: &mut Session, v: &str) {
    s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": v})).unwrap();
}

// ---- the preference ----

#[test]
fn the_engine_preference_defaults_to_auto_and_takes_its_four_values() {
    let mut s = Session::new();
    assert_eq!(s.prefs().tools.select_subject_engine, SubjectEngine::Auto);
    for (v, e) in [("macos", SubjectEngine::MacOs), ("model", SubjectEngine::Model), ("classic", SubjectEngine::Classic), ("auto", SubjectEngine::Auto)] {
        set_engine(&mut s, v);
        assert_eq!(s.prefs().tools.select_subject_engine, e);
    }
    assert_eq!(crate::prefs::choices("tools.selectSubjectEngine"), Some(&["auto", "macos", "model", "classic"][..]));
}

#[test]
fn the_engine_preference_refuses_other_values_and_keeps_the_old_one() {
    let mut s = Session::new();
    set_engine(&mut s, "classic");
    for bad in ["classical", "Auto", "", "mac"] {
        let e = s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": bad})).unwrap_err().to_string();
        assert!(e.contains("auto") && e.contains("macos") && e.contains("model") && e.contains("classic"), "{bad}: {e}");
    }
    assert!(s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": 3})).is_err());
    assert_eq!(s.prefs().tools.select_subject_engine, SubjectEngine::Classic);
}

#[test]
fn the_engine_preference_is_saved_and_an_old_file_without_it_means_auto() {
    let mut s = Session::new();
    set_engine(&mut s, "macos");
    let saved = s.prefs_to_json();
    let mut t = Session::new();
    t.load_prefs_json(&saved).unwrap();
    assert_eq!(t.prefs().tools.select_subject_engine, SubjectEngine::MacOs);
    // A file from before this setting: the tools section has no such key.
    let mut u = Session::new();
    u.load_prefs_json(r#"{"tools": {"showTooltips": false}}"#).unwrap();
    assert_eq!(u.prefs().tools.select_subject_engine, SubjectEngine::Auto);
    assert!(!u.prefs().tools.show_tooltips);
}

// ---- which engine runs ----

fn disc_sampler(s: &Session) -> (Rect, std::sync::Arc<photocraft_doc::Document>) {
    let d = s.active().unwrap();
    (d.doc.bounds(), std::sync::Arc::clone(&d.doc))
}

fn run_pick(s: &Session, engine: SubjectEngine, cut: Option<CutoutFn>) -> Found {
    let (canvas, doc) = disc_sampler(s);
    let surf = doc.layers[0].surface().unwrap();
    pick(engine, cut, &SurfaceSampler(surf), canvas)
}

fn bbox_of(f: &Found) -> Option<Rect> {
    f.region.as_ref().map(|r| r.bbox)
}

#[test]
fn classic_never_asks_the_cutout() {
    let s = disc_session();
    thread_local!(static CALLS: Cell<u32> = const { Cell::new(0) });
    fn counting(rgba: &[u8], w: u32, h: u32) -> Result<Mask, Error> {
        CALLS.with(|c| c.set(c.get() + 1));
        left_quarter(rgba, w, h)
    }
    let f = run_pick(&s, SubjectEngine::Classic, Some(counting));
    assert_eq!(CALLS.with(Cell::get), 0);
    assert!(f.note.is_none());
    let b = bbox_of(&f).expect("classic finds the disc");
    assert!(b.x0 > 40 && b.x1 < 160, "{b:?}");
}

#[test]
fn auto_uses_the_cutout_when_there_is_one_and_classic_when_there_is_not() {
    let s = disc_session();
    let with = run_pick(&s, SubjectEngine::Auto, Some(left_quarter));
    assert!(with.note.is_none());
    let b = bbox_of(&with).unwrap();
    assert_eq!((b.x0, b.y0, b.y1), (0, 0, 160), "the cutout's left quarter, not the disc: {b:?}");
    let without = run_pick(&s, SubjectEngine::Auto, None);
    assert!(without.note.is_none(), "auto without a cutout is not an event");
    let b = bbox_of(&without).expect("classic finds the disc");
    assert!(b.x0 > 40, "{b:?}");
}

#[test]
fn macos_without_a_cutout_uses_classic_and_says_so() {
    let s = disc_session();
    let f = run_pick(&s, SubjectEngine::MacOs, None);
    assert_eq!(f.note, Some("macOS cutout unavailable, used Classic"));
    assert!(bbox_of(&f).is_some_and(|b| b.x0 > 40));
}

#[test]
fn a_cutout_that_fails_falls_back_to_classic_with_a_note() {
    let s = disc_session();
    for engine in [SubjectEngine::Auto, SubjectEngine::MacOs] {
        let f = run_pick(&s, engine, Some(broken));
        assert_eq!(f.note, Some(FAILED), "{engine:?}");
        assert!(bbox_of(&f).is_some_and(|b| b.x0 > 40), "{engine:?}");
    }
}

#[test]
fn no_subject_found_lets_auto_try_classic_but_macos_selects_nothing() {
    let s = disc_session();
    for cut in [no_subject as CutoutFn, empty_mask] {
        let auto = run_pick(&s, SubjectEngine::Auto, Some(cut));
        assert!(auto.note.is_none() && bbox_of(&auto).is_some_and(|b| b.x0 > 40), "auto asks Classic");
        let mac = run_pick(&s, SubjectEngine::MacOs, Some(cut));
        assert!(mac.note.is_none() && mac.region.is_none(), "an explicit macOS choice has no fallback");
    }
}

// ---- the mask becomes a region ----

fn canvas(w: i32, h: i32) -> Rect {
    Rect::new(0, 0, w, h)
}

#[test]
fn a_mask_at_canvas_size_is_cut_down_to_its_box() {
    let (w, h) = (20u32, 10u32);
    let data: Vec<u8> = (0..w * h).map(|i| if (5..9).contains(&(i % w)) && (2..6).contains(&(i / w)) { 200 } else { 0 }).collect();
    let r = region_from_mask(&Mask { width: w, height: h, data }, canvas(20, 10), 1).unwrap();
    assert_eq!(r.bbox, Rect::new(5, 2, 9, 6));
    assert_eq!(r.mask, vec![200; 16]);
}

#[test]
fn a_shrunk_mask_is_scaled_back_by_exactly_the_step_and_cropped() {
    // A 100×60 canvas seen at step 3: 34×20 pixels (the last block is partial). Pixels x 10..20 of the
    // small mask are set: canvas x 30..60 once scaled (the soft edge reaches one block out).
    let (sw, sh) = (34u32, 20u32);
    let data: Vec<u8> = (0..sw * sh).map(|i| if (10..20).contains(&(i % sw)) { 255 } else { 0 }).collect();
    let r = region_from_mask(&Mask { width: sw, height: sh, data }, canvas(100, 60), 3).unwrap();
    assert!(r.bbox.x0 >= 27 && r.bbox.x0 <= 31, "{:?}", r.bbox);
    assert!(r.bbox.x1 >= 59 && r.bbox.x1 <= 63, "{:?}", r.bbox);
    assert_eq!((r.bbox.y0, r.bbox.y1), (0, 60), "full height");
    assert_eq!(r.mask.len(), (r.bbox.width() * r.bbox.height()) as usize);
    assert_eq!(r.at(45, 30), 1.0);
    assert!(r.at(r.bbox.x0, 30) < 1.0, "the edge is partial coverage");
}

#[test]
fn a_mask_reaching_the_last_partial_block_stays_inside_the_canvas() {
    let (sw, sh) = (4u32, 4u32); // a 10×10 canvas at step 3
    let r = region_from_mask(&Mask { width: sw, height: sh, data: vec![255; 16] }, canvas(10, 10), 3).unwrap();
    assert_eq!(r.bbox, canvas(10, 10));
    assert!(r.mask.iter().all(|v| *v == 255));
}

#[test]
fn a_region_is_placed_at_the_canvas_origin() {
    let r = region_from_mask(&Mask { width: 4, height: 4, data: [vec![0; 5], vec![90; 2], vec![0; 9]].concat() }, Rect::new(10, 20, 14, 24), 1).unwrap();
    assert_eq!(r.bbox, Rect::new(11, 21, 13, 22));
    assert_eq!(r.mask, vec![90, 90]);
}

#[test]
fn an_empty_or_mismatched_mask_is_no_region() {
    assert!(region_from_mask(&Mask { width: 8, height: 8, data: vec![0; 64] }, canvas(8, 8), 1).is_none());
    // The mask does not fit the canvas at this step.
    assert!(region_from_mask(&Mask { width: 8, height: 8, data: vec![255; 64] }, canvas(30, 30), 3).is_none());
    assert!(region_from_mask(&Mask { width: 8, height: 8, data: vec![255; 64] }, canvas(8, 8), 2).is_none());
    assert!(region_from_mask(&Mask { width: 8, height: 8, data: vec![255; 60] }, canvas(8, 8), 1).is_none(), "data short of its size");
    assert!(region_from_mask(&Mask { width: 8, height: 8, data: vec![255; 64] }, Rect::new(0, 0, 0, 0), 1).is_none());
}

// ---- the command ----

#[test]
fn select_subject_stores_the_cutout_as_the_selection_with_soft_edges() {
    let mut s = disc_session();
    let r = with_cutout(Some(left_quarter), || s.execute("select.subject", json!({})).unwrap());
    assert_eq!((r["changed"].as_bool(), r["selected"].as_bool()), (Some(true), Some(true)));
    assert!(r.get("note").is_none());
    assert_eq!(r["bounds"], json!([0, 0, 51, 160]));
    assert!(cov(&s, 10, 80) > 0.99);
    assert!(cov(&s, 100, 80) < 0.01, "not the Classic disc");
    let edge = cov(&s, 50, 80);
    assert!((0.4..0.6).contains(&edge), "soft edge kept: {edge}");
    s.execute("edit.undo", json!({})).unwrap();
    assert!(s.active().unwrap().doc.selection.is_none());
}

#[test]
fn select_subject_with_macos_but_no_cutout_reports_the_fallback_in_its_result() {
    let mut s = disc_session();
    set_engine(&mut s, "macos");
    let r = with_cutout(None, || s.execute("select.subject", json!({})).unwrap());
    assert_eq!(r["note"], "macOS cutout unavailable, used Classic");
    assert_eq!(r["changed"], true);
    assert!(cov(&s, 100, 80) > 0.99, "Classic's disc");
}

#[test]
fn select_subject_classic_ignores_the_cutout_and_has_no_note() {
    let mut s = disc_session();
    set_engine(&mut s, "classic");
    let r = with_cutout(Some(left_quarter), || s.execute("select.subject", json!({})).unwrap());
    assert!(r.get("note").is_none());
    assert!(cov(&s, 100, 80) > 0.99 && cov(&s, 10, 80) < 0.01);
}

#[test]
fn select_subject_keeps_its_modes_with_the_cutout() {
    let mut s = disc_session();
    with_cutout(Some(left_quarter), || {
        s.execute("select.rect", json!({"rect": [100, 0, 100, 160]})).ok();
        s.execute("select.all", json!({})).unwrap();
        // Subtract the cutout from Select All: its area is out, the rest stays.
        s.execute("select.subject", json!({"mode": "subtract"})).unwrap();
        assert!(cov(&s, 10, 80) < 0.01 && cov(&s, 150, 80) > 0.99);
        // Add it back to what is left.
        s.execute("select.subject", json!({"mode": "add"})).unwrap();
        assert!(cov(&s, 10, 80) > 0.99 && cov(&s, 150, 80) > 0.99);
    });
}

#[test]
fn select_subject_nothing_found_by_the_cutout_keeps_the_selection() {
    let mut s = session_with(140, 100, Box::new(|_, _| [0.5, 0.5, 0.5]));
    set_engine(&mut s, "macos");
    s.execute("select.all", json!({})).unwrap();
    let r = with_cutout(Some(no_subject), || s.execute("select.subject", json!({})).unwrap());
    assert_eq!(r["changed"], false);
    assert!(cov(&s, 10, 10) > 0.99);
}

#[test]
fn a_changed_engine_choice_is_not_answered_from_the_last_result() {
    thread_local!(static CALLS: Cell<u32> = const { Cell::new(0) });
    fn counting(rgba: &[u8], w: u32, h: u32) -> Result<Mask, Error> {
        CALLS.with(|c| c.set(c.get() + 1));
        left_quarter(rgba, w, h)
    }
    let mut s = disc_session();
    with_cutout(Some(counting), || {
        s.execute("select.subject", json!({})).unwrap();
        s.execute("select.subject", json!({})).unwrap();
        assert_eq!(CALLS.with(Cell::get), 1, "the repeat on the same pixels and engine is answered from the last result");
        assert!(cov(&s, 10, 80) > 0.99);
        // Same pixels, other engine: Classic's disc, not the cached cutout.
        set_engine(&mut s, "classic");
        s.execute("select.subject", json!({})).unwrap();
        assert!(cov(&s, 100, 80) > 0.99 && cov(&s, 10, 80) < 0.01);
        // And back: the cutout runs again.
        set_engine(&mut s, "auto");
        s.execute("select.subject", json!({})).unwrap();
        assert_eq!(CALLS.with(Cell::get), 2);
        assert!(cov(&s, 10, 80) > 0.99);
    });
}

#[test]
fn a_big_document_is_shrunk_for_the_cutout_and_the_mask_covers_the_whole_canvas() {
    // 2600 × 1700 = 4.4 MP: past Vision's budget, so the cutout is given a shrunk picture.
    thread_local!(static SEEN: Cell<(u32, u32)> = const { Cell::new((0, 0)) });
    fn spy(rgba: &[u8], w: u32, h: u32) -> Result<Mask, Error> {
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        SEEN.with(|c| c.set((w, h)));
        Ok(Mask { width: w, height: h, data: vec![255; (w * h) as usize] })
    }
    let mut s = session_with(2600, 1700, Box::new(|x, _| [x as f32 / 2600.0, 0.4, 0.4]));
    let r = with_cutout(Some(spy), || s.execute("select.subject", json!({})).unwrap());
    let (w, h) = SEEN.with(Cell::get);
    assert!(u64::from(w) * u64::from(h) <= macvision::MAX_VISION_PIXELS && w < 2600, "{w}×{h}");
    assert_eq!(r["bounds"], json!([0, 0, 2600, 1700]), "the mask is scaled back to the canvas");
}

/// The real Vision on this Mac, through the whole command: a synthetic figure on a plain
/// background (runs by default on a Mac with the request, skips elsewhere).
#[test]
#[cfg(target_os = "macos")]
fn the_real_macos_cutout_selects_a_synthetic_figure_through_the_command() {
    if !macvision::available() {
        eprintln!("skip: no Vision subject request on this Mac");
        return;
    }
    let (w, h) = (480u32, 600u32);
    let mut s = session_with(
        w,
        h,
        Box::new(move |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let (cx, cy, r) = (w as f32 * 0.5, h as f32 * 0.30, w as f32 * 0.11);
            let head = (fx - cx).powi(2) + (fy - cy).powi(2) < r * r;
            let torso = fy >= h as f32 * 0.45 && fy < h as f32 * 0.95 && (fx - cx).abs() < w as f32 * 0.3;
            let fold = 0.08 * ((fx * 0.12).sin() + (fy * 0.05).cos());
            if head {
                [0.88, 0.68, 0.55]
            } else if torso {
                [0.16 + fold, 0.27 + fold, 0.5 + fold]
            } else {
                [0.78, 0.76, 0.72]
            }
        }),
    );
    set_engine(&mut s, "macos");
    let cutout: CutoutFn = macvision::subject_mask;
    let r = with_cutout(Some(cutout), || s.execute("select.subject", json!({})).unwrap());
    assert_eq!(r["changed"], true, "{r}");
    assert!(r.get("note").is_none(), "{r}");
    assert!(cov(&s, 240, 400) > 0.9, "torso");
    assert!(cov(&s, 5, 5) < 0.1, "background");
}

/// A cutout that covers only the top 15 % of whatever it is given.
fn top_strip(_rgba: &[u8], w: u32, h: u32) -> Result<Mask, Error> {
    Ok(Mask { width: w, height: h, data: (0..w * h).map(|i| if i / w < h * 15 / 100 { 255 } else { 0 }).collect() })
}

/// The mask's rows are the document's rows in the same order: a cutout of the top is the top of the
/// selection, for a document big enough to be shrunk too.
#[test]
fn the_cutouts_top_is_the_selections_top() {
    for (w, h) in [(200u32, 160u32), (2600, 1700)] {
        let mut s = session_with(w, h, Box::new(|x, _| [x as f32 / 3000.0, 0.4, 0.4]));
        let r = with_cutout(Some(top_strip), || s.execute("select.subject", json!({})).unwrap());
        let (top, bottom) = (h as i32 * 15 / 100, h as i32);
        assert_eq!(r["bounds"][1], 0, "{w}×{h}: {r}");
        assert!((i64::from(r["bounds"][3].as_i64().unwrap() as i32 - top)).abs() <= 3, "{w}×{h}: the selection ends near the top 15 %: {r}");
        assert!(cov(&s, w as i32 / 2, 2) > 0.99, "{w}×{h}");
        assert!(cov(&s, w as i32 / 2, bottom - 3) < 0.01 && cov(&s, w as i32 / 2, bottom * 6 / 10) < 0.01, "{w}×{h}");
    }
}

#[test]
fn a_repeat_on_unchanged_pixels_keeps_the_fallback_note() {
    let mut s = disc_session();
    set_engine(&mut s, "macos");
    let first = with_cutout(None, || s.execute("select.subject", json!({})).unwrap());
    let again = with_cutout(None, || s.execute("select.subject", json!({})).unwrap());
    assert_eq!(first["note"], UNAVAILABLE);
    assert_eq!(again["note"], UNAVAILABLE, "answered from the last result, with its note");
    // A result that came without a note (Classic chosen) has none on the repeat either.
    set_engine(&mut s, "classic");
    s.execute("select.subject", json!({})).unwrap();
    assert!(s.execute("select.subject", json!({})).unwrap().get("note").is_none());
}

/// The real Vision through the command: a figure only in the top 15 % of the document is selected
/// there and nowhere near the bottom (a flipped mask would select the bottom).
#[test]
#[cfg(target_os = "macos")]
fn the_real_macos_cutout_keeps_a_top_figure_at_the_top_through_the_command() {
    if !macvision::available() {
        eprintln!("skip: no Vision subject request on this Mac");
        return;
    }
    let (w, h, bw, bh) = (640u32, 800u32, 300u32, 120u32);
    let (bx, cx) = ((w - bw) / 2, w as f32 / 2.0);
    let mut s = session_with(
        w,
        h,
        Box::new(move |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let (head_cy, head_r) = (bh as f32 * 0.30, bw as f32 * 0.11);
            let head = (fx - cx).powi(2) + (fy - head_cy).powi(2) < head_r * head_r;
            let half_w = bw as f32 * (0.14 + 0.16 * ((fy - bh as f32 * 0.45) / (bh as f32 * 0.1)).clamp(0.0, 1.0));
            let torso = fy >= bh as f32 * 0.45 && fy < bh as f32 * 0.95 && (fx - cx).abs() < half_w && x >= bx;
            let fold = 0.08 * ((fx * 0.12).sin() + (fy * 0.05).cos());
            if head {
                [0.88, 0.68, 0.55]
            } else if torso {
                [0.16 + fold, 0.27 + fold, 0.5 + fold]
            } else {
                [0.78, 0.76, 0.72]
            }
        }),
    );
    set_engine(&mut s, "macos");
    let cutout: CutoutFn = macvision::subject_mask;
    let r = with_cutout(Some(cutout), || s.execute("select.subject", json!({})).unwrap());
    assert_eq!(r["changed"], true, "{r}");
    assert!(cov(&s, 320, 90) > 0.9, "the torso at the top");
    assert!(cov(&s, 320, 760) < 0.01 && cov(&s, 320, 400) < 0.01 && cov(&s, 20, 790) < 0.01, "nothing below");
}

/// A strip too thin for Vision is "nothing found", not a failure with a note.
#[test]
#[cfg(target_os = "macos")]
fn the_real_macos_cutout_on_a_thin_strip_finds_nothing_without_a_note() {
    if !macvision::available() {
        return;
    }
    let mut s = session_with(3000, 6, Box::new(|x, _| [x as f32 / 3000.0, 0.4, 0.4]));
    set_engine(&mut s, "macos");
    let cutout: CutoutFn = macvision::subject_mask;
    let r = with_cutout(Some(cutout), || s.execute("select.subject", json!({})).unwrap());
    assert_eq!(r["changed"], false, "{r}");
    assert!(r.get("note").is_none(), "{r}");
}
