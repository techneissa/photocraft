//! Refine Hair's Protect Subject against a fake SAM 3 that answers a box with the whole subject
//! (no model): a shadow at the selection's edge that the colours cut away stays with it.

use std::sync::{Arc, Mutex};

use photocraft_ml::Cancel;

use super::backend::{Mask, PointBox};
use super::*;

/// A SAM 3 whose box prompt answers with `subject` (document pixels, one encoded pixel each), fully
/// covered; it records the boxes it was asked for.
struct Subject {
    subject: Rect,
    boxes: Mutex<Vec<PointBox>>,
}

impl Sam3Backend for Subject {
    fn present(&self) -> bool {
        true
    }
    fn has_memory(&self) -> bool {
        true
    }
    fn encode(&self, rgb: &photocraft_ml::Rgb, _: Cancel<'_>) -> std::result::Result<EncodedImage, MlError> {
        Ok(EncodedImage { token: 1, w: rgb.w, h: rgb.h })
    }
    fn is_current(&self, image: EncodedImage) -> bool {
        image.token == 1
    }
    fn clicks(&self, _: EncodedImage, _: &[PointClick]) -> std::result::Result<Mask, MlError> {
        Err(MlError::Unsupported)
    }
    fn boxed(&self, image: EncodedImage, rect: PointBox, _: &[PointClick]) -> std::result::Result<Mask, MlError> {
        self.boxes.lock().unwrap().push(rect);
        let (w, h) = (image.w, image.h);
        let alpha = (0..w * h).map(|i| if self.subject.contains((i % w) as i32, (i / w) as i32) { 1.0 } else { 0.0 }).collect();
        Ok(Mask { w, h, alpha })
    }
    fn release(&self, _: EncodedImage) {}
}

const SUBJECT: Rect = Rect { x0: 30, y0: 20, x1: 90, y1: 70 };
const SHADOW: Rect = Rect { x0: 30, y0: 35, x1: 42, y1: 55 };

/// A 120 × 90 picture: a skin subject on a backdrop as dark as the shadow under its nose, which
/// touches the subject's edge; the subject is selected, shadow included.
fn scene(fake: &Arc<Subject>) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 120, "height": 90, "depth": 8})).unwrap();
    s.edit("paint", |doc, a| {
        let surface = doc.layer_mut(a.unwrap()).unwrap().surface_mut().unwrap();
        surface.fill_rect(Rect::new(0, 0, 120, 90), &[0.10, 0.10, 0.11, 1.0]);
        surface.fill_rect(SUBJECT, &[0.80, 0.60, 0.50, 1.0]);
        surface.fill_rect(SHADOW, &[0.09, 0.08, 0.08, 1.0]);
        Ok(())
    })
    .unwrap();
    s.execute("select.rect", json!({"x": SUBJECT.x0, "y": SUBJECT.y0, "width": SUBJECT.width(), "height": SUBJECT.height()})).unwrap();
    s.set_sam3_backend(fake.clone());
    s
}

fn fake() -> Arc<Subject> {
    Arc::new(Subject { subject: SUBJECT, boxes: Mutex::new(Vec::new()) })
}

/// The mean coverage of the selection over the shadow's three columns inside the edge pixel.
fn notch(s: &Session) -> f32 {
    let mut sum = 0.0;
    for y in SHADOW.y0..SHADOW.y1 {
        for x in SHADOW.x0 + 1..SHADOW.x0 + 4 {
            sum += super::tests::cov(s, x, y);
        }
    }
    sum / (3 * SHADOW.height()) as f32
}

#[test]
fn protect_subject_keeps_the_shadow_that_refine_hair_alone_cuts_away() {
    let fake = fake();
    let mut s = scene(&fake);
    // Off: today's Refine Hair; the shadow reads as backdrop.
    let r = s.execute("select.refineHair", json!({"radius": 10})).unwrap();
    assert_eq!(r["protected"], false, "{r}");
    let off = notch(&s);
    assert!(off < 0.5, "cut away without protection: {off}");
    super::tests::undo(&mut s);
    // On, with the picture encoded: the subject SAM 3 finds inside the selection's bounds is kept.
    assert_eq!(s.execute("select.objectPrepare", json!({"wait": true})).unwrap()["status"], "ready");
    let r = s.execute("select.refineHair", json!({"radius": 10, "protectSubject": true})).unwrap();
    assert_eq!(r["protected"], true, "{r}");
    let on = notch(&s);
    assert!(on > 0.99, "kept with protection: {on}");
    let sent = fake.boxes.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].x0, sent[0].y0, sent[0].x1, sent[0].y1), (30.0 / 120.0, 20.0 / 90.0, 90.0 / 120.0, 70.0 / 90.0), "the selection's bounds");
    // One History step either way.
    super::tests::undo(&mut s);
    assert_eq!(super::tests::cov(&s, 60, 45), 1.0);
    assert_eq!(super::tests::cov(&s, 20, 45), 0.0);
}

#[test]
fn without_a_ready_encoding_protect_subject_runs_as_off() {
    let fake = fake();
    let mut s = scene(&fake);
    let r = s.execute("select.refineHair", json!({"radius": 10, "protectSubject": true})).unwrap();
    assert_eq!(r["protected"], false, "{r}");
    assert!(fake.boxes.lock().unwrap().is_empty());
    let asked = notch(&s);
    super::tests::undo(&mut s);
    s.execute("select.refineHair", json!({"radius": 10})).unwrap();
    assert_eq!(notch(&s), asked, "exactly Refine Hair without it");
    assert!(s.execute("select.refineHair", json!({"protectSubject": "yes"})).is_err());
}

#[test]
fn select_and_mask_edits_carry_protect_subject() {
    let fake = fake();
    let mut s = scene(&fake);
    s.execute("select.objectPrepare", json!({"wait": true})).unwrap();
    let edits = json!([{"tool": "refineHair", "radius": 10, "protectSubject": true}]);
    s.execute("select.refineEdge", json!({"edits": edits})).unwrap();
    assert!(notch(&s) > 0.99, "{}", notch(&s));
    super::tests::undo(&mut s);
    s.execute("select.refineEdge", json!({"edits": [{"tool": "refineHair", "radius": 10}]})).unwrap();
    assert!(notch(&s) < 0.5);
}

#[test]
fn a_finder_for_a_reduced_copy_asks_for_the_same_box_and_answers_in_its_pixels() {
    let fake = fake();
    let mut s = scene(&fake);
    s.execute("select.objectPrepare", json!({"wait": true})).unwrap();
    let ProtectSource::Ready(finder) = s.sam3_protect(false) else { panic!("the encoding is ready") };
    // The preview works on a copy half the size: the subject is at (15, 10)–(45, 35) there.
    let region = finder.reduced(2).find(Rect::new(15, 10, 45, 35)).unwrap().expect("a subject");
    assert_eq!(region.bbox, Rect::new(15, 10, 45, 35));
    assert!(region.mask.iter().all(|v| *v == 255));
    let sent = fake.boxes.lock().unwrap()[0];
    assert_eq!((sent.x0, sent.y0, sent.x1, sent.y1), (30.0 / 120.0, 20.0 / 90.0, 90.0 / 120.0, 70.0 / 90.0));
    // Nothing of the box on the canvas: no subject.
    assert!(finder.find(Rect::new(500, 500, 600, 600)).unwrap().is_none());
}

#[test]
fn shrink_averages_blocks_and_keeps_odd_edges_partial() {
    let region = Region { bbox: Rect::new(1, 1, 5, 4), mask: vec![255; 12] };
    let small = protect::shrink(&region, 2);
    assert_eq!(small.bbox, Rect::new(0, 0, 3, 2));
    // The border blocks hold one or two of their four pixels, the middle one all four.
    assert_eq!(small.mask, [64, 128, 64, 128, 255, 128]);
}

#[test]
fn protect_possible_follows_the_install_the_memory_and_the_preference() {
    let mut s = scene(&fake());
    assert!(s.sam3_protect_possible());
    s.execute("prefs.set", json!({"path": "tools.objectSelectionEngine", "value": "classic"})).unwrap();
    assert!(!s.sam3_protect_possible(), "Classic never uses SAM 3");
    let mut s = Session::new();
    assert!(!s.sam3_protect_possible(), "no backend, no model");
}
