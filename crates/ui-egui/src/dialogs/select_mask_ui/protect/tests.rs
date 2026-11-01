//! Protect Subject in the workspace against a fake SAM 3 (no model): the box, its remembered
//! state, the wait for the encoding, Esc, and a failed encoding.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use photocraft_engine::sam3_select::{EncodedImage, Mask, PointBox, PointClick, Sam3Backend};

use super::super::*;
use super::*;

struct Fake {
    gate_open: AtomicBool,
    fail: AtomicBool,
    current: Mutex<Option<u64>>,
}

impl Fake {
    fn new() -> Arc<Self> {
        Arc::new(Self { gate_open: AtomicBool::new(true), fail: AtomicBool::new(false), current: Mutex::new(None) })
    }
    fn current(&self) -> std::sync::MutexGuard<'_, Option<u64>> {
        self.current.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Sam3Backend for Fake {
    fn present(&self) -> bool {
        true
    }
    fn has_memory(&self) -> bool {
        true
    }
    fn encode(&self, rgb: &photocraft_ml::Rgb, cancel: photocraft_ml::Cancel<'_>) -> Result<EncodedImage, photocraft_ml::MlError> {
        while !self.gate_open.load(Ordering::SeqCst) {
            if cancel() {
                return Err(photocraft_ml::MlError::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(photocraft_ml::MlError::Backend("boom".into()));
        }
        *self.current() = Some(1);
        Ok(EncodedImage { token: 1, w: rgb.w, h: rgb.h })
    }
    fn is_current(&self, image: EncodedImage) -> bool {
        *self.current() == Some(image.token)
    }
    fn clicks(&self, _: EncodedImage, _: &[PointClick]) -> Result<Mask, photocraft_ml::MlError> {
        Err(photocraft_ml::MlError::Unsupported)
    }
    fn boxed(&self, image: EncodedImage, _: PointBox, _: &[PointClick]) -> Result<Mask, photocraft_ml::MlError> {
        Ok(Mask { w: image.w, h: image.h, alpha: vec![1.0; image.w * image.h] })
    }
    fn release(&self, _: EncodedImage) {}
}

/// A 160 × 100 document with the top-left 20 × 20 selected, Select and Mask open, and `fake` (when
/// there is one) as SAM 3.
fn workspace(fake: Option<&Arc<Fake>>) -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.session.execute("file.new", json!({"width": 160, "height": 100, "depth": 8})).unwrap();
    app.session.execute("select.rect", json!({"x": 0, "y": 0, "width": 20, "height": 20})).unwrap();
    app.session.prefs.edit(|p| {
        p.dialogs.remove(super::REMEMBERED);
        p.dialogs.remove(super::super::REMEMBERED);
    });
    if let Some(fake) = fake {
        app.session.set_sam3_backend(fake.clone());
    }
    open(&mut app).unwrap();
    app
}

fn hair() -> Value {
    json!({"tool": "refineHair", "radius": 0.0, "sampleAllLayers": false})
}

fn edits(app: &PhotocraftApp) -> Vec<Value> {
    app.distort.select_mask.as_ref().unwrap().edits.clone()
}

/// Ticks until the waiting run was added (or dropped).
fn settle(app: &mut PhotocraftApp) {
    let ctx = egui::Context::default();
    for _ in 0..600 {
        tick(app, &ctx);
        if !app.distort.select_mask.as_ref().unwrap().protect.waiting() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("still waiting");
}

#[test]
fn without_sam_3_the_box_is_off_and_refine_hair_is_as_before() {
    let mut app = workspace(None);
    let w = app.distort.select_mask.as_ref().unwrap();
    assert!(!w.protect.possible && w.protect.on, "ticked by default, but greyed out");
    assert_eq!(w.describe()["protectSubject"], false);
    assert_eq!(route(&mut app, hair()), Some(hair()), "no protectSubject, no wait");
    let j = job(&mut app).unwrap();
    assert!(j.protect.is_none());
}

#[test]
fn the_box_is_ticked_by_default_with_sam_3_and_its_state_is_remembered() {
    let fake = Fake::new();
    let mut app = workspace(Some(&fake));
    assert!(app.distort.select_mask.as_ref().unwrap().protect.possible);
    assert_eq!(app.distort.select_mask.as_ref().unwrap().describe()["protectSubject"], true);
    // Unticked: Refine Hair goes out as it is, and the choice is kept for the next time.
    {
        let w = app.distort.select_mask.as_mut().unwrap();
        w.protect.on = false;
        w.protect.changed = true;
    }
    persist(&mut app);
    assert_eq!(route(&mut app, hair()), Some(hair()));
    cancel(&mut app);
    open(&mut app).unwrap();
    assert!(!app.distort.select_mask.as_ref().unwrap().protect.on, "remembered");
    assert_eq!(route(&mut app, hair()), Some(hair()));
    // Ticked again.
    {
        let w = app.distort.select_mask.as_mut().unwrap();
        w.protect.on = true;
        w.protect.changed = true;
    }
    persist(&mut app);
    cancel(&mut app);
    open(&mut app).unwrap();
    assert!(app.distort.select_mask.as_ref().unwrap().protect.on);
}

#[test]
fn refine_hair_waits_for_the_encoding_and_goes_out_protected() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    assert_eq!(route(&mut app, hair()), None, "held back while SAM 3 prepares");
    assert_eq!(app.ui.status, "Preparing SAM 3 for this image… (Esc stops it)");
    assert!(edits(&app).is_empty());
    // OK refuses meanwhile, and nothing else is held back.
    assert!(commit(&mut app).is_err());
    assert_eq!(route(&mut app, json!({"tool": "invert"})), Some(json!({"tool": "invert"})));
    fake.gate_open.store(true, Ordering::SeqCst);
    settle(&mut app);
    let list = edits(&app);
    assert_eq!(list.len(), 1);
    assert_eq!((list[0]["tool"].as_str(), list[0]["protectSubject"].as_bool()), (Some("refineHair"), Some(true)));
    // The preview now has SAM 3 to ask.
    assert!(job(&mut app).unwrap().protect.is_some());
    // Ready already: the edit goes straight out.
    let again = route(&mut app, hair()).expect("ready");
    assert_eq!(again["protectSubject"], true);
}

#[test]
fn esc_stops_the_wait_and_adds_nothing() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    assert_eq!(route(&mut app, hair()), None);
    escape(&mut app);
    assert!(app.distort.select_mask.is_some(), "Esc stopped the run, it did not close Select and Mask");
    assert!(edits(&app).is_empty() && !app.distort.select_mask.as_ref().unwrap().protect.waiting());
    assert_eq!(app.ui.status, "Refine Hair stopped");
    assert_eq!(app.session.sam3_status(false), photocraft_engine::sam3_select::Sam3Status::Cancelled);
    // Esc again closes, as before.
    escape(&mut app);
    assert!(app.distort.select_mask.is_none());
}

#[test]
fn an_encoding_that_fails_adds_refine_hair_without_protection_and_says_so() {
    let fake = Fake::new();
    fake.fail.store(true, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    let held = route(&mut app, hair());
    if held.is_none() {
        settle(&mut app);
    } else {
        assert_eq!(held, Some(hair()));
    }
    let list = edits(&app);
    if held.is_none() {
        assert_eq!(list.len(), 1);
        assert!(list[0].get("protectSubject").is_none(), "{list:?}");
    }
    assert!(app.ui.status.starts_with("Refine Hair ran without Protect Subject"), "{}", app.ui.status);
}
