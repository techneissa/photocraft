//! Dragged rectangles and Select Subject with SAM 3, against the fake backend of `tests.rs` (no
//! model): a box is a rectangle of coverage, a concept is a rectangle found by its text.

use super::tests::{Fake, cov, pixel_edit, ready, rect_mask, session, status, undo};
use super::*;

fn drag(s: &mut Session, rect: [f32; 4], extra: Value) -> Value {
    let mut p = json!({"rect": rect});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s.execute("select.object", p).unwrap()
}

fn area(s: &Session) -> f32 {
    (0..200).flat_map(|y| (0..400).map(move |x| (x, y))).map(|(x, y)| cov(s, x, y)).sum()
}

#[test]
fn a_dragged_rectangle_selects_the_main_object_inside_it() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    let r = drag(&mut s, [100.0, 50.0, 200.0, 100.0], json!({}));
    assert_eq!((r["engine"].as_str(), r["changed"].as_bool()), (Some("sam3"), Some(true)), "{r}");
    // the box reaches the backend as corners normalised to the encoded image, with no clicks
    let (b, extra) = fake.state().boxes[0].clone();
    assert_eq!((b.x0, b.y0, b.x1, b.y1), (0.25, 0.25, 0.75, 0.75));
    assert!(extra.is_empty() && fake.state().calls.is_empty());
    // the object fills the rectangle but for its margin, and the rim keeps partial coverage
    assert_eq!(cov(&s, 200, 100), 1.0);
    assert_eq!((cov(&s, 90, 100), cov(&s, 310, 100), cov(&s, 200, 40)), (0.0, 0.0, 0.0));
    let rim: Vec<f32> = (100..112).map(|x| cov(&s, x, 100)).collect();
    assert!(rim.iter().any(|v| *v > 0.1 && *v < 0.9), "{rim:?}");
    // one History step
    undo(&mut s);
    assert_eq!(area(&s), 0.0);
}

#[test]
fn the_rectangle_maps_through_a_shrunk_encoding_and_is_cut_to_the_canvas() {
    // 4200 × 42 is shrunk by 3: the encoded image is 1400 × 14
    let fake = Fake::new();
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 4200, "height": 42})).unwrap();
    s.set_sam3_backend(fake.clone());
    assert_eq!(s.execute("select.objectPrepare", json!({"wait": true})).unwrap()["status"], "ready");
    // a rectangle that sticks out of the canvas on the left and bottom
    drag(&mut s, [-300.0, 6.0, 2400.0, 100.0], json!({}));
    let (b, _) = fake.state().boxes[0].clone();
    assert!((b.x0 - 0.0).abs() < 1e-6 && (b.x1 - 2100.0 / 4200.0).abs() < 1e-3, "{b:?}");
    assert!((b.y0 - 2.0 / 14.0).abs() < 1e-3 && (b.y1 - 1.0).abs() < 1e-6, "{b:?}");
}

#[test]
fn shift_adds_alt_subtracts_and_both_intersect() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}));
    let first = area(&s);
    assert!(first > 5000.0, "{first}");
    // ⇧: another object joins the selection
    drag(&mut s, [240.0, 20.0, 100.0, 100.0], json!({"mode": "add"}));
    assert!(cov(&s, 60, 60) == 1.0 && cov(&s, 280, 60) == 1.0);
    let both = area(&s);
    assert!((both / first - 2.0).abs() < 0.05, "{both} / {first}");
    // ⌥: an object is taken off
    drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({"mode": "subtract"}));
    assert!(cov(&s, 60, 60) == 0.0 && cov(&s, 280, 60) == 1.0);
    // ⇧⌥: only what both hold
    drag(&mut s, [200.0, 0.0, 100.0, 200.0], json!({"mode": "intersect"}));
    assert!(cov(&s, 280, 60) > 0.9 && cov(&s, 330, 60) == 0.0, "{}", cov(&s, 330, 60));
    // each drag is one History step: four edits go back to nothing
    for _ in 0..4 {
        undo(&mut s);
    }
    assert_eq!(area(&s), 0.0);
}

#[test]
fn a_box_that_finds_nothing_leaves_the_selection_and_history_alone() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}));
    let before = area(&s);
    let r = drag(&mut s, [300.0, 150.0, 2.0, 2.0], json!({"mode": "add"}));
    assert_eq!((r["changed"].as_bool(), r["engine"].as_str()), (Some(false), Some("sam3")), "{r}");
    assert_eq!(area(&s), before);
    undo(&mut s);
    assert_eq!(area(&s), 0.0, "the empty box made no History step");
}

#[test]
fn a_box_starts_no_click_sequence_and_keeps_the_encoding() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}));
    assert_eq!(status(&mut s), Sam3Status::Ready, "a selection edit keeps the encoding");
    // a ⇧-click now starts a new sequence from the selection (one positive point), not a refinement of the box
    super::tests::click(&mut s, 300.0, 100.0, json!({"refine": "add", "mode": "add"}));
    assert_eq!(fake.state().calls.last().map(Vec::len), Some(1));
    assert_eq!(fake.state().encodes, 1);
    pixel_edit(&mut s);
    assert_eq!(status(&mut s), Sam3Status::Idle, "a pixel edit drops it, as for clicks");
}

#[test]
fn until_ready_and_on_failure_the_classic_rectangle_answers_with_a_note() {
    // not encoded yet: Classic, quietly (the tool was not picked through the UI)
    let fake = Fake::new();
    let mut s = session(&fake);
    let r = drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}));
    assert!(r.get("engine").is_none() && r.get("note").is_none() && fake.state().boxes.is_empty(), "{r}");
    // preparing
    fake.set_gate(false);
    s.sam3_prepare(false, Prepare::Force).unwrap();
    assert_eq!(drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}))["note"], NOT_READY);
    s.sam3_cancel();
    fake.set_gate(true);
    // a model failure drops the encoding and says so
    let mut s = ready(&fake);
    fake.state().fail_boxes = true;
    let r = drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}));
    assert_eq!(r["note"], FAILED, "{r}");
    assert_ne!(status(&mut s), Sam3Status::Ready);
    // the Classic preference never asks SAM 3, with no note
    let fake = Fake::new();
    let mut s = ready(&fake);
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "classic"})).unwrap();
    let r = drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}));
    assert!(r.get("engine").is_none() && r.get("note").is_none() && fake.state().boxes.is_empty(), "{r}");
    // an explicit sam3 choice without the model says why
    let mut s = session(&Fake::with(false, true));
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "sam3"})).unwrap();
    assert_eq!(drag(&mut s, [20.0, 20.0, 100.0, 100.0], json!({}))["note"], NOT_INSTALLED);
}

/// A fake that finds a person in the middle of the picture and a "main subject" on the left.
fn with_concepts(person: bool, main: bool) -> Arc<Fake> {
    let fake = Fake::new();
    if person {
        fake.state().concepts.push(("person".into(), (0.25, 0.25, 0.75, 0.75)));
    }
    if main {
        fake.state().concepts.push(("main subject".into(), (0.0, 0.0, 0.25, 1.0)));
    }
    fake
}

fn subject_session(fake: &Arc<Fake>) -> Session {
    let mut s = session(fake);
    s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": "sam3"})).unwrap();
    s
}

#[test]
fn select_subject_with_sam3_asks_for_a_person_and_frees_its_own_encoding() {
    let fake = with_concepts(true, true);
    let mut s = subject_session(&fake);
    let r = s.execute("select.subject", json!({})).unwrap();
    assert_eq!(r["changed"], true, "{r}");
    assert!(r.get("note").is_none(), "{r}");
    assert_eq!(fake.state().texts, ["person"], "the first prompt that scores is the answer");
    assert_eq!((cov(&s, 200, 100), cov(&s, 20, 100), cov(&s, 380, 190)), (1.0, 0.0, 0.0));
    let st = fake.state();
    assert_eq!((st.encodes, st.released), (1, 1), "the job encoded the picture and gave it back");
    drop(st);
    // a repeat on the same pixels is answered from the cache
    s.execute("select.deselect", json!({})).unwrap();
    s.execute("select.subject", json!({})).unwrap();
    assert_eq!(fake.state().texts.len(), 1);
    assert_eq!(cov(&s, 200, 100), 1.0);
}

#[test]
fn nothing_for_person_asks_for_the_main_subject_then_gives_up_without_classic() {
    let fake = with_concepts(false, true);
    let mut s = subject_session(&fake);
    s.execute("select.subject", json!({})).unwrap();
    assert_eq!(fake.state().texts, ["person", "main subject"]);
    assert_eq!((cov(&s, 20, 100), cov(&s, 200, 100)), (1.0, 0.0));
    // neither scores: "no subject" is the answer, the selection stays
    let fake = with_concepts(false, false);
    let mut s = subject_session(&fake);
    s.execute("select.rect", json!({"rect": [0, 0, 40, 40]})).ok();
    let r = s.execute("select.subject", json!({})).unwrap();
    assert_eq!(r["changed"], false, "{r}");
    assert!(r.get("note").is_none(), "an explicit choice's \"nothing\" is not a fallback: {r}");
    assert_eq!(fake.state().texts, ["person", "main subject"]);
}

#[test]
fn the_shared_encoding_is_used_and_kept_when_it_is_ready() {
    let fake = with_concepts(true, false);
    let mut s = ready(&fake);
    s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": "sam3"})).unwrap();
    s.execute("select.subject", json!({})).unwrap();
    let st = fake.state();
    assert_eq!((st.encodes, st.released), (1, 0));
    drop(st);
    assert_eq!(status(&mut s), Sam3Status::Ready, "Object Selection still has its encoding");
    assert_eq!(cov(&s, 200, 100), 1.0);
}

#[test]
fn a_sam3_that_cannot_run_falls_back_to_classic_with_a_note() {
    for (present, memory, note) in [(false, true, NOT_INSTALLED), (true, false, NO_MEMORY)] {
        let fake = Fake::with(present, memory);
        let mut s = subject_session(&fake);
        let r = s.execute("select.subject", json!({})).unwrap();
        assert_eq!(r["note"], note, "{r}");
        assert!(fake.state().texts.is_empty() && fake.state().encodes == 0);
    }
    // the model failing while it runs
    let fake = with_concepts(true, false);
    fake.state().fail_text = true;
    let mut s = subject_session(&fake);
    assert_eq!(s.execute("select.subject", json!({})).unwrap()["note"], FAILED);
    // a failed encoding
    let fake = with_concepts(true, false);
    fake.state().fail_encode = true;
    let mut s = subject_session(&fake);
    assert_eq!(s.execute("select.subject", json!({})).unwrap()["note"], FAILED);
}

#[test]
fn automatic_and_the_other_engines_never_ask_sam3() {
    for engine in ["auto", "classic", "macos", "model"] {
        let fake = with_concepts(true, true);
        let mut s = session(&fake);
        s.execute("prefs.set", json!({"path": "tools.selectSubjectEngine", "value": engine})).unwrap();
        s.execute("select.subject", json!({})).unwrap();
        let st = fake.state();
        assert!(st.texts.is_empty() && st.encodes == 0, "{engine}");
    }
    // the engine param beats the preference both ways
    let fake = with_concepts(true, false);
    let mut s = session(&fake);
    s.execute("select.subject", json!({"engine": "sam3"})).unwrap();
    assert_eq!(fake.state().texts, ["person"]);
    assert!(s.execute("select.subject", json!({"engine": "sam4"})).unwrap_err().to_string().contains("sam3"));
}

#[test]
fn remove_background_follows_the_same_choice() {
    let fake = with_concepts(true, false);
    let mut s = session(&fake);
    s.execute("layer.removeBackground", json!({"engine": "sam3"})).unwrap();
    assert_eq!(fake.state().texts, ["person"]);
    let d = s.active().unwrap();
    let mask = d.doc.layers[0].mask.as_ref().expect("a mask");
    assert!(mask.surface.sample_channel(200, 100, 0) > 0.9 && mask.surface.sample_channel(20, 100, 0) < 0.1);
}

#[test]
fn the_fake_masks_are_rectangles() {
    let m = rect_mask(100, 50, (0.2, 0.2, 0.8, 0.8), 2.0);
    assert!(m.alpha[25 * 100 + 50] == 1.0 && m.alpha[2 * 100 + 2] == 0.0);
}
