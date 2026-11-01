//! SAM 3 clicks against a fake backend (no model): a click is a disc, so the add, subtract,
//! cache and fallback logic can be checked by looking at the selection.

use std::sync::{Arc, Condvar, Mutex};

use super::mask::{spread, step_for, to_region};
use super::*;
use crate::prefs::ObjectEngine;

/// Radius of the disc a click selects, in encoded pixels.
const R: f32 = 30.0;

#[derive(Default)]
struct State {
    next: u64,
    current: Option<u64>,
    size: (usize, usize),
    encodes: usize,
    released: usize,
    /// Every call of `clicks`.
    calls: Vec<Vec<PointClick>>,
    fail_encode: bool,
    fail_clicks: bool,
}

/// A backend whose mask is the union of the discs around positive clicks minus those around negative
/// ones, soft at the rim. `gate`: while closed, `encode` waits (then honours `cancel`).
struct Fake {
    present: bool,
    memory: bool,
    state: Mutex<State>,
    gate: (Mutex<bool>, Condvar),
}

impl Fake {
    fn new() -> Arc<Self> {
        Arc::new(Self { present: true, memory: true, state: Mutex::default(), gate: (Mutex::new(true), Condvar::new()) })
    }

    fn with(present: bool, memory: bool) -> Arc<Self> {
        Arc::new(Self { present, memory, state: Mutex::default(), gate: (Mutex::new(true), Condvar::new()) })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn set_gate(&self, open: bool) {
        *self.gate.0.lock().unwrap() = open;
        self.gate.1.notify_all();
    }

    /// The service's idle unload.
    fn unload(&self) {
        self.state().current = None;
    }
}

impl Sam3Backend for Fake {
    fn present(&self) -> bool {
        self.present
    }
    fn has_memory(&self) -> bool {
        self.memory
    }
    fn encode(&self, rgb: &Rgb, cancel: photocraft_ml::Cancel<'_>) -> std::result::Result<EncodedImage, MlError> {
        let mut open = self.gate.0.lock().unwrap();
        while !*open {
            let (guard, _) = self.gate.1.wait_timeout(open, Duration::from_millis(10)).unwrap();
            open = guard;
            if cancel() {
                return Err(MlError::Cancelled);
            }
        }
        drop(open);
        if cancel() {
            return Err(MlError::Cancelled);
        }
        let mut st = self.state();
        if st.fail_encode {
            return Err(MlError::Backend("boom".into()));
        }
        st.next += 1;
        st.current = Some(st.next);
        st.size = (rgb.w, rgb.h);
        st.encodes += 1;
        Ok(EncodedImage { token: st.next, w: rgb.w, h: rgb.h })
    }
    fn is_current(&self, image: EncodedImage) -> bool {
        self.state().current == Some(image.token)
    }
    fn clicks(&self, image: EncodedImage, clicks: &[PointClick]) -> std::result::Result<Mask, MlError> {
        let mut st = self.state();
        st.calls.push(clicks.to_vec());
        if st.fail_clicks {
            return Err(MlError::Backend("boom".into()));
        }
        if st.current != Some(image.token) {
            return Err(MlError::BadInput("the image is no longer encoded".into()));
        }
        let (w, h) = (image.w, image.h);
        let mut alpha = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                // the rim is six pixels of partial coverage
                let cover = |c: &PointClick| {
                    let (dx, dy) = (x as f32 + 0.5 - c.x * w as f32, y as f32 + 0.5 - c.y * h as f32);
                    ((R - (dx * dx + dy * dy).sqrt()) / 6.0 + 0.5).clamp(0.0, 1.0)
                };
                let on = clicks.iter().filter(|c| c.positive).map(&cover).fold(0.0f32, f32::max);
                let off = clicks.iter().filter(|c| !c.positive).map(&cover).fold(0.0f32, f32::max);
                alpha[y * w + x] = (on - off).max(0.0);
            }
        }
        Ok(Mask { w, h, alpha })
    }
    fn release(&self, image: EncodedImage) {
        let mut st = self.state();
        if st.current == Some(image.token) {
            st.current = None;
        }
        st.released += 1;
    }
}

/// A 400 × 200 grey document with a session that uses `fake`.
fn session(fake: &Arc<Fake>) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 400, "height": 200})).unwrap();
    s.set_sam3_backend(fake.clone());
    s
}

fn ready(fake: &Arc<Fake>) -> Session {
    let mut s = session(fake);
    let r = s.execute("select.objectPrepare", json!({"wait": true})).unwrap();
    assert_eq!(r["status"], "ready", "{r}");
    s
}

fn cov(s: &Session, x: i32, y: i32) -> f32 {
    s.active().unwrap().doc.selection.as_ref().map_or(0.0, |m| m.sample_channel(x, y, 0))
}

fn click(s: &mut Session, x: f32, y: f32, extra: Value) -> Value {
    let mut p = json!({"point": [x, y]});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s.execute("select.object", p).unwrap()
}

fn pixel_edit(s: &mut Session) {
    s.execute("paint.pencil", json!({"points": [[5, 5], [30, 5]], "size": 4, "color": "#000000"})).unwrap();
}

fn undo(s: &mut Session) {
    s.execute("edit.undo", json!({})).unwrap();
}

fn status(s: &mut Session) -> Sam3Status {
    s.sam3_status(false)
}

#[test]
fn without_a_backend_the_classic_finder_answers_with_no_note() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 100, "height": 80})).unwrap();
    assert_eq!(status(&mut s), Sam3Status::Unavailable(None));
    let r = click(&mut s, 50.0, 40.0, json!({}));
    assert!(r.get("note").is_none() && r.get("engine").is_none(), "{r}");
}

#[test]
fn a_ready_encoding_makes_a_click_select_the_sam3_object_with_soft_edges() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    assert_eq!(fake.state().size, (400, 200), "the whole picture is encoded");
    let r = click(&mut s, 200.0, 100.0, json!({}));
    assert_eq!((r["engine"].as_str(), r["changed"].as_bool()), (Some("sam3"), Some(true)), "{r}");
    assert_eq!(cov(&s, 200, 100), 1.0);
    assert_eq!(cov(&s, 5, 5), 0.0);
    // the rim keeps partial coverage: the matte's alpha is the selection's alpha
    let rim: Vec<f32> = (225..235).map(|x| cov(&s, x, 100)).collect();
    assert!(rim.iter().any(|v| *v > 0.1 && *v < 0.9), "{rim:?}");
    // a click is one History step
    undo(&mut s);
    assert_eq!(cov(&s, 200, 100), 0.0);
}

#[test]
fn the_click_position_maps_through_a_shrunk_encoding() {
    // 4200 × 42 is shrunk by 3: the encoded image is 1400 × 14
    let fake = Fake::new();
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 4200, "height": 42})).unwrap();
    s.set_sam3_backend(fake.clone());
    assert_eq!(s.execute("select.objectPrepare", json!({"wait": true})).unwrap()["status"], "ready");
    assert_eq!(fake.state().size, (1400, 14));
    click(&mut s, 2100.0, 21.0, json!({}));
    let c = fake.state().calls[0][0];
    assert!((c.x - 0.5).abs() < 1e-3 && (c.y - 0.5).abs() < 1e-3, "{c:?}");
    // R = 30 encoded pixels = 90 document pixels either side
    assert!(cov(&s, 2100, 21) > 0.99 && cov(&s, 2100 + 80, 21) > 0.99 && cov(&s, 2100 + 100, 21) < 0.01);
}

#[test]
fn shift_adds_a_positive_point_and_alt_a_negative_one_to_the_same_mask() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    click(&mut s, 100.0, 100.0, json!({}));
    assert!(cov(&s, 100, 100) > 0.99 && cov(&s, 160, 100) < 0.01);
    // ⇧: a second positive point; the mask now holds both discs, rebuilt from both points
    let r = click(&mut s, 160.0, 100.0, json!({"refine": "add", "mode": "add"}));
    assert_eq!(r["engine"], "sam3");
    assert!(cov(&s, 100, 100) > 0.99 && cov(&s, 160, 100) > 0.99);
    assert_eq!(fake.state().calls.last().unwrap().len(), 2);
    // ⌥: a negative point cuts a hole
    click(&mut s, 160.0, 100.0, json!({"refine": "subtract", "mode": "subtract"}));
    assert!(cov(&s, 100, 100) > 0.99 && cov(&s, 160, 100) < 0.01);
    let last = fake.state().calls.last().unwrap().clone();
    assert_eq!(last.iter().map(|c| c.positive).collect::<Vec<_>>(), [true, true, false]);
    // one History step per click
    undo(&mut s);
    assert!(cov(&s, 160, 100) > 0.99);
    undo(&mut s);
    assert!(cov(&s, 100, 100) > 0.99 && cov(&s, 160, 100) < 0.01);
    undo(&mut s);
    assert_eq!(cov(&s, 100, 100), 0.0);
}

#[test]
fn a_refinement_keeps_the_selection_the_sequence_started_from() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    s.execute("select.rect", json!({"x": 300, "y": 20, "width": 40, "height": 40})).unwrap();
    // ⇧ with no sequence: the object is added to the selection
    click(&mut s, 100.0, 100.0, json!({"refine": "add", "mode": "add"}));
    assert!(cov(&s, 100, 100) > 0.99 && cov(&s, 310, 30) > 0.99);
    // ⇧ again refines that object; the rectangle stays
    click(&mut s, 160.0, 100.0, json!({"refine": "add", "mode": "add"}));
    assert!(cov(&s, 160, 100) > 0.99 && cov(&s, 310, 30) > 0.99);
    assert_eq!(fake.state().calls.last().unwrap().len(), 2, "one sequence");
}

#[test]
fn alt_without_a_sequence_takes_the_object_off_the_selection() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    s.execute("select.all", json!({})).unwrap();
    click(&mut s, 200.0, 100.0, json!({"refine": "subtract", "mode": "subtract"}));
    assert!(cov(&s, 200, 100) < 0.01 && cov(&s, 10, 10) > 0.99);
    assert!(fake.state().calls.last().unwrap().iter().all(|c| c.positive), "the click that finds the object is positive");
}

#[test]
fn another_edit_an_undo_or_a_plain_click_ends_the_sequence() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    let refine = json!({"refine": "add", "mode": "add"});
    click(&mut s, 100.0, 100.0, json!({}));
    s.execute("select.rect", json!({"x": 300, "y": 20, "width": 40, "height": 40})).unwrap();
    click(&mut s, 160.0, 100.0, refine.clone());
    assert_eq!(fake.state().calls.last().unwrap().len(), 1, "a selection edit in between: a new sequence");
    click(&mut s, 160.0, 100.0, refine.clone());
    undo(&mut s);
    click(&mut s, 250.0, 100.0, refine.clone());
    assert_eq!(fake.state().calls.last().unwrap().len(), 1, "after an undo: a new sequence");
    click(&mut s, 100.0, 100.0, json!({}));
    click(&mut s, 160.0, 100.0, refine.clone());
    click(&mut s, 330.0, 100.0, json!({"mode": "replace"}));
    assert_eq!(fake.state().calls.last().unwrap().len(), 1, "a plain click starts afresh");
}

#[test]
fn a_click_that_finds_nothing_leaves_the_selection_and_history_alone() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    click(&mut s, 100.0, 100.0, json!({}));
    // the fake's disc cannot be negated away entirely: a negative click on the only point
    let r = click(&mut s, 100.0, 100.0, json!({"refine": "subtract", "mode": "subtract"}));
    assert_eq!((r["engine"].as_str(), r["changed"].as_bool()), (Some("sam3"), Some(false)), "{r}");
    assert!(cov(&s, 100, 100) > 0.99);
    undo(&mut s);
    assert_eq!(cov(&s, 100, 100), 0.0, "only the first click was a step");
}

#[test]
fn the_encoding_survives_selection_edits_and_dies_with_pixel_edits() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    click(&mut s, 100.0, 100.0, json!({}));
    s.execute("select.deselect", json!({})).unwrap();
    s.execute("select.rect", json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap();
    assert_eq!(status(&mut s), Sam3Status::Ready);
    assert_eq!(fake.state().encodes, 1);
    pixel_edit(&mut s);
    assert_eq!(status(&mut s), Sam3Status::Idle);
    // the release runs on its own thread
    for _ in 0..200 {
        if fake.state().released > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(fake.state().released, 1);
    // an undo of the pixel edit does not bring the old encoding back
    undo(&mut s);
    assert_eq!(status(&mut s), Sam3Status::Idle);
    // and the next preparation encodes again
    assert_eq!(s.execute("select.objectPrepare", json!({"wait": true})).unwrap()["status"], "ready");
    assert_eq!(fake.state().encodes, 2);
}

#[test]
fn closing_the_document_frees_the_encoding() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    s.execute("file.new", json!({"width": 50, "height": 50})).unwrap();
    assert_eq!(s.sam3_status(false), Sam3Status::Idle, "another document has no encoding");
    s.close(1);
    s.close(0);
    for _ in 0..200 {
        if fake.state().released > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(fake.state().released > 0);
    assert!(matches!(s.sam3.entry, Entry::None));
}

#[test]
fn the_layer_choice_is_part_of_the_key() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    // the layer is sampled by default; the composite is another picture
    assert_eq!(s.sam3_status(true), Sam3Status::Idle);
    assert_eq!(s.sam3_prepare(true, Prepare::Auto).unwrap(), Sam3Status::Preparing);
    assert_eq!(s.sam3_wait(true, Duration::from_secs(10)), Sam3Status::Ready);
    assert_eq!(fake.state().encodes, 2);
}

#[test]
fn until_the_encoding_is_ready_clicks_use_classic_and_say_so() {
    let fake = Fake::new();
    fake.set_gate(false);
    let mut s = session(&fake);
    assert_eq!(s.sam3_prepare(false, Prepare::Auto).unwrap(), Sam3Status::Preparing);
    assert_eq!(s.sam3_prepare(false, Prepare::Force).unwrap(), Sam3Status::Preparing, "no second encoding");
    assert!(s.sam3_hover_source(false).is_none());
    let r = click(&mut s, 200.0, 100.0, json!({}));
    assert_eq!(r["note"], NOT_READY, "{r}");
    assert!(r.get("engine").is_none());
    fake.set_gate(true);
    assert_eq!(s.sam3_wait(false, Duration::from_secs(10)), Sam3Status::Ready);
    assert!(s.sam3_hover_source(false).is_some());
    assert_eq!(click(&mut s, 200.0, 100.0, json!({}))["engine"], "sam3");
}

#[test]
fn cancelling_stops_the_encoding_and_only_a_forced_prepare_starts_it_again() {
    let fake = Fake::new();
    fake.set_gate(false);
    let mut s = session(&fake);
    s.sam3_prepare(false, Prepare::Auto).unwrap();
    assert!(s.sam3_cancel());
    assert!(!s.sam3_cancel(), "nothing left to cancel");
    assert_eq!(status(&mut s), Sam3Status::Cancelled);
    fake.set_gate(true);
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(s.sam3_prepare(false, Prepare::Auto).unwrap(), Sam3Status::Cancelled, "the UI's polling does not undo Esc");
    assert_eq!(fake.state().encodes, 0, "the stopped encoding made nothing");
    assert_eq!(s.sam3_prepare(false, Prepare::Force).unwrap(), Sam3Status::Preparing);
    assert_eq!(s.sam3_wait(false, Duration::from_secs(10)), Sam3Status::Ready);
    // the command does the same
    assert_eq!(s.execute("select.objectPrepare", json!({"cancel": true})).unwrap()["cancelled"], false);
}

#[test]
fn an_edit_during_the_encoding_discards_its_result() {
    let fake = Fake::new();
    fake.set_gate(false);
    let mut s = session(&fake);
    s.sam3_prepare(false, Prepare::Auto).unwrap();
    pixel_edit(&mut s);
    assert_eq!(status(&mut s), Sam3Status::Idle);
    fake.set_gate(true);
    // the worker sees its result is not wanted and gives the image back
    for _ in 0..400 {
        if fake.state().released > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let st = fake.state();
    assert!(st.encodes == 0 || st.released > 0, "encodes {} released {}", st.encodes, st.released);
}

#[test]
fn an_idle_unload_is_noticed_and_not_undone_by_polling() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    fake.unload();
    assert_eq!(status(&mut s), Sam3Status::Unloaded);
    assert_eq!(s.sam3_prepare(false, Prepare::Auto).unwrap(), Sam3Status::Unloaded);
    assert!(s.sam3_hover_source(false).is_none());
    let r = click(&mut s, 200.0, 100.0, json!({}));
    assert!(r.get("engine").is_none() && r.get("note").is_none(), "Classic, quietly: {r}");
    assert_eq!(s.sam3_prepare(false, Prepare::Force).unwrap(), Sam3Status::Preparing);
    assert_eq!(s.sam3_wait(false, Duration::from_secs(10)), Sam3Status::Ready);
}

#[test]
fn failures_fall_back_to_classic_with_a_note() {
    let fake = Fake::new();
    fake.state().fail_encode = true;
    let mut s = session(&fake);
    assert_eq!(s.execute("select.objectPrepare", json!({"wait": true})).unwrap()["status"], "failed");
    assert_eq!(click(&mut s, 200.0, 100.0, json!({}))["note"], FAILED);
    assert_eq!(s.sam3_prepare(false, Prepare::Auto).unwrap(), Sam3Status::Failed(FAILED), "a failure stands until an edit or a forced try");
    assert_eq!(fake.state().encodes, 0);
    fake.state().fail_encode = false;
    assert_eq!(s.execute("select.objectPrepare", json!({"wait": true})).unwrap()["status"], "ready");
    // the model failing on a click: the image is dropped, Classic answers
    fake.state().fail_clicks = true;
    assert_eq!(click(&mut s, 200.0, 100.0, json!({}))["note"], FAILED);
    assert!(matches!(s.sam3.entry, Entry::None));
}

#[test]
fn the_preference_and_the_computer_decide_whether_sam3_runs() {
    let fake = Fake::with(false, true);
    let mut s = session(&fake);
    assert_eq!(s.sam3_status(false), Sam3Status::Unavailable(None), "auto with no model: quietly Classic");
    assert!(click(&mut s, 200.0, 100.0, json!({})).get("note").is_none());
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "sam3"})).unwrap();
    assert_eq!(s.sam3_status(false), Sam3Status::Unavailable(Some(NOT_INSTALLED)));
    assert_eq!(click(&mut s, 200.0, 100.0, json!({}))["note"], NOT_INSTALLED);
    assert_eq!(s.execute("select.objectPrepare", json!({})).unwrap()["note"], NOT_INSTALLED);

    let fake = Fake::with(true, false);
    let mut s = session(&fake);
    assert_eq!(s.sam3_status(false), Sam3Status::Unavailable(None));
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "sam3"})).unwrap();
    assert_eq!(click(&mut s, 200.0, 100.0, json!({}))["note"], NO_MEMORY);

    // Classic ignores a ready encoding
    let fake = Fake::new();
    let mut s = ready(&fake);
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "classic"})).unwrap();
    assert_eq!(s.sam3_status(false), Sam3Status::Unavailable(None));
    assert!(click(&mut s, 200.0, 100.0, json!({})).get("engine").is_none());
    assert!(s.sam3_hover_source(false).is_none());
}

#[test]
fn the_engine_preference_is_auto_sam3_or_classic() {
    assert_eq!(crate::prefs::choices("tools.objectSelectionEngine"), Some(&["auto", "sam3", "classic"][..]));
    let mut s = Session::new();
    assert_eq!(s.prefs().tools.object_selection_engine, ObjectEngine::Auto);
    assert!(s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "model"})).is_err());
    assert!(s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": 3})).is_err());
}

#[test]
fn quick_selection_strokes_are_clicks_and_alt_strokes_negative() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    let stroke: Vec<[f32; 2]> = (0..40).map(|i| [80.0 + i as f32 * 2.0, 100.0]).collect();
    let r = s.execute("select.quick", json!({"points": stroke, "size": 20})).unwrap();
    assert_eq!(r["engine"], "sam3", "{r}");
    let calls = fake.state().calls.clone();
    assert_eq!(calls[0].len(), STROKE_CLICKS, "a long stroke is spread to a few clicks");
    assert!(calls[0].iter().all(|c| c.positive));
    assert!(cov(&s, 80, 100) > 0.99 && cov(&s, 158, 100) > 0.99);
    // ⌥ stroke: negative points on the same mask
    s.execute("select.quick", json!({"points": [[158, 100]], "size": 20, "mode": "subtract"})).unwrap();
    let last = fake.state().calls.last().unwrap().clone();
    assert_eq!(last.len(), STROKE_CLICKS + 1);
    assert!(!last.last().unwrap().positive);
    assert!(cov(&s, 158, 100) < 0.01 && cov(&s, 80, 100) > 0.99);
    undo(&mut s);
    assert!(cov(&s, 158, 100) > 0.99);
}

#[test]
fn quick_selection_falls_back_to_grabcut_with_a_note_while_preparing() {
    let fake = Fake::new();
    fake.set_gate(false);
    let mut s = session(&fake);
    s.sam3_prepare(false, Prepare::Auto).unwrap();
    let r = s.execute("select.quick", json!({"points": [[200, 100]], "size": 30})).unwrap();
    assert_eq!(r["note"], NOT_READY, "{r}");
    fake.set_gate(true);
    s.sam3_wait(false, Duration::from_secs(10));
}

#[test]
fn the_hover_source_answers_one_click_at_a_time() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    let hover = s.sam3_hover_source(false).unwrap();
    let region = hover.find(200.0, 100.0).unwrap().unwrap();
    assert!(region.at(200, 100) > 0.99 && region.at(5, 5) == 0.0);
    assert!(region.bbox.width() >= 55 && region.bbox.width() <= 70, "{:?}", region.bbox);
    assert_eq!(hover.find(-5.0, 10.0).unwrap(), None, "outside the canvas");
    assert_eq!(hover.find(f32::NAN, 10.0).unwrap(), None);
    // hovering selects nothing and costs no History step
    assert_eq!(cov(&s, 200, 100), 0.0);
    assert!(!s.active().unwrap().history.can_undo() || s.active().unwrap().doc.selection.is_none());
    // once the encoding is gone it says so instead of panicking
    fake.unload();
    assert!(hover.find(200.0, 100.0).is_err());
}

#[test]
fn a_bad_refine_value_and_a_missing_document_are_errors() {
    let fake = Fake::new();
    let mut s = ready(&fake);
    assert!(s.execute("select.object", json!({"point": [10, 10], "refine": "maybe"})).is_err());
    let mut empty = Session::new();
    empty.set_sam3_backend(fake.clone());
    assert!(empty.execute("select.objectPrepare", json!({})).is_err());
}

// the mask on the canvas

fn mask_of(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Mask {
    Mask { w, h, alpha: (0..w * h).map(|i| f(i % w, i / w)).collect() }
}

#[test]
fn the_mask_keeps_partial_coverage_and_trims_to_the_object() {
    let m = mask_of(40, 30, |x, _| {
        if x < 10 {
            0.0
        } else if x == 10 {
            0.5
        } else if x < 20 {
            1.0
        } else {
            0.0
        }
    });
    let r = to_region(&m, Rect::new(0, 0, 40, 30), 1).unwrap().unwrap();
    assert_eq!(r.bbox, Rect::new(10, 0, 20, 30));
    assert_eq!(r.at(10, 5), 128.0 / 255.0, "0.5 stays 0.5");
    assert_eq!(r.at(15, 5), 1.0);
    assert_eq!(r.at(25, 5), 0.0);
}

#[test]
fn an_empty_or_faint_mask_selects_nothing_and_a_wrong_size_is_an_error() {
    assert_eq!(to_region(&mask_of(10, 10, |_, _| 0.0), Rect::new(0, 0, 10, 10), 1).unwrap(), None);
    assert_eq!(to_region(&mask_of(10, 10, |_, _| 0.001), Rect::new(0, 0, 10, 10), 1).unwrap(), None, "the sigmoid's tail is not an edge");
    assert!(to_region(&Mask { w: 4, h: 4, alpha: vec![1.0; 5] }, Rect::new(0, 0, 4, 4), 1).is_err());
    assert!(to_region(&Mask { w: 0, h: 0, alpha: vec![] }, Rect::new(0, 0, 4, 4), 1).is_err());
    assert!(to_region(&mask_of(4, 4, |_, _| 1.0), Rect::new(0, 0, 0, 0), 1).is_err());
}

#[test]
fn a_shrunk_mask_is_stretched_back_over_the_canvas() {
    // a 10 × 10 mask of a 25 × 25 canvas (step 3): its middle block covers the middle of the canvas
    let m = mask_of(9, 9, |x, y| if (3..6).contains(&x) && (3..6).contains(&y) { 1.0 } else { 0.0 });
    let r = to_region(&m, Rect::new(0, 0, 25, 25), 3).unwrap().unwrap();
    assert!(r.at(12, 12) == 1.0 && r.at(1, 1) == 0.0 && r.at(24, 24) == 0.0);
    let (b, mid) = (r.bbox, 13);
    assert!(b.x0 <= 9 && b.x1 >= 18 && b.x0 > 3 && b.x1 < 24 && (b.x0 + b.x1) / 2 - mid <= 2, "{b:?}");
}

#[test]
fn steps_and_points_of_a_stroke() {
    assert_eq!(step_for(Rect::new(0, 0, 2016, 100)), 1);
    assert_eq!(step_for(Rect::new(0, 0, 2017, 100)), 2);
    assert_eq!(step_for(Rect::new(0, 0, 6000, 4000)), 3);
    let line: Vec<(f32, f32)> = (0..100).map(|i| (i as f32, 0.0)).collect();
    let few = spread(&line, 6);
    assert_eq!((few.len(), few.first(), few.last()), (6, Some(&(0.0, 0.0)), Some(&(99.0, 0.0))));
    assert_eq!(spread(&line[..3], 6).len(), 3);
    assert_eq!(spread(&line, 1), vec![(0.0, 0.0)]);
    assert!(spread(&line, 0).is_empty() && spread(&[], 6).is_empty());
}

#[test]
fn unsure_coverage_is_stretched_so_the_inside_is_solid_and_the_haze_goes() {
    let m = mask_of(30, 10, |x, _| match x {
        0..=4 => 0.1,
        5..=9 => 0.3,
        10..=14 => 0.5,
        15..=19 => 0.7,
        _ => 0.9,
    });
    let r = to_region(&m, Rect::new(0, 0, 30, 10), 1).unwrap().unwrap();
    assert_eq!(r.at(2, 5), 0.0, "0.1: not selected");
    assert!((r.at(7, 5) - 1.0 / 6.0).abs() < 0.01, "0.3 → {}", r.at(7, 5));
    assert!((r.at(12, 5) - 0.5).abs() < 0.01);
    assert!((r.at(17, 5) - 5.0 / 6.0).abs() < 0.01);
    assert_eq!(r.at(25, 5), 1.0, "0.9: solid");
}
