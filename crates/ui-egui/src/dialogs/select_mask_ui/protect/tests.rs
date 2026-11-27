//! Refine Hair and Protect Subject in the workspace against a fake SAM 3 (no model): the box, its remembered
//! state, the wait for the encoding, Stop, and a failed encoding.

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
fn workspace(fake: Option<&Arc<Fake>>) -> Box<PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.session.execute("file.new", json!({"width": 160, "height": 100, "depth": 8})).unwrap();
    app.session.execute("select.rect", json!({"x": 0, "y": 0, "width": 20, "height": 20})).unwrap();
    app.session.prefs.edit(|p| {
        p.dialogs.remove(super::REMEMBERED);
    });
    if let Some(fake) = fake {
        app.session.set_sam3_backend(fake.clone());
    }
    open(&mut app).unwrap();
    app
}

fn id(app: &PhotocraftApp) -> DialogId {
    app.select_mask_target.as_ref().expect("Select and Mask is open").0
}

/// The Refine Hair button pressed (Edge Detection radius 0, the active layer only).
fn press(app: &mut PhotocraftApp) {
    let (ctx, id) = (app.ctx().clone(), id(app));
    on_event(app, &ctx, id, RefineHairEvent::Run { radius: 0.0, sample_all_layers: false });
}

fn send(app: &mut PhotocraftApp, event: RefineHairEvent) {
    let (ctx, id) = (app.ctx().clone(), id(app));
    on_event(app, &ctx, id, event);
}

fn edits(app: &PhotocraftApp) -> Vec<SelectMaskEdit> {
    app.framework.dialogs.get_by_id::<Hosted>(id(app)).expect("the workspace is on the dialog host").workspace().session().edits.clone()
}

/// Cancel, as Esc or the button does, and the app's pass that follows.
fn cancel(app: &mut PhotocraftApp) {
    let (ctx, id) = (app.ctx().clone(), id(app));
    app.framework.dialogs.cancel_by_id(&ctx, id);
    take(app, &ctx);
}

fn hair(protect: bool) -> SelectMaskEdit {
    SelectMaskEdit::refine_hair(0.0, false, protect)
}

/// Ticks until the waiting run was recorded (or dropped).
fn settle(app: &mut PhotocraftApp) {
    let (ctx, id) = (egui::Context::default(), id(app));
    for _ in 0..600 {
        tick(app, &ctx, id);
        if !app.select_mask_protect.waiting() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("still waiting");
}

#[test]
fn without_sam_3_the_box_is_off_and_refine_hair_is_as_before() {
    let mut app = workspace(None);
    assert!(!app.select_mask_protect.possible && app.select_mask_protect.on, "ticked by default, but greyed out");
    assert_eq!(describe(&app).unwrap()["protectSubject"], false);
    press(&mut app);
    assert_eq!(edits(&app), [hair(false)], "no protection, no wait");
    let list = edits(&app);
    assert!(finders(&mut app, &list).is_empty());
}

#[test]
fn the_box_is_ticked_by_default_with_sam_3_and_its_state_is_remembered() {
    let fake = Fake::new();
    let mut app = workspace(Some(&fake));
    assert!(app.select_mask_protect.possible);
    assert_eq!(describe(&app).unwrap()["protectSubject"], true);
    // Unticked: Refine Hair goes out as it is, and the choice is kept for the next time.
    send(&mut app, RefineHairEvent::Protect(false));
    press(&mut app);
    assert_eq!(edits(&app), [hair(false)]);
    cancel(&mut app);
    open(&mut app).unwrap();
    assert!(!app.select_mask_protect.on, "remembered");
    // Ticked again.
    send(&mut app, RefineHairEvent::Protect(true));
    cancel(&mut app);
    open(&mut app).unwrap();
    assert!(app.select_mask_protect.on);
}

#[test]
fn refine_hair_waits_for_the_encoding_and_is_recorded_protected() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    press(&mut app);
    assert!(app.select_mask_protect.waiting(), "held back while SAM 3 prepares");
    assert_eq!(app.ui.status, "Preparing SAM 3 for this image… (Esc closes Select and Mask)");
    assert!(edits(&app).is_empty());
    // OK refuses meanwhile.
    let answer = ui_commit(&mut app);
    assert!(answer.is_err(), "{answer:?}");
    fake.gate_open.store(true, Ordering::SeqCst);
    settle(&mut app);
    assert_eq!(edits(&app), [hair(true)]);
    // The preview now has SAM 3 to ask.
    let list = edits(&app);
    assert_eq!(finders(&mut app, &list).len(), 1);
    // Ready already: the edit goes straight in.
    press(&mut app);
    assert_eq!(edits(&app), [hair(true), hair(true)]);
}

/// OK as the control channel sends it.
fn ui_commit(app: &mut PhotocraftApp) -> Result<Value, String> {
    let ctx = app.ctx().clone();
    crate::control::dialogs::workspace_ui(app, &ctx, "select.selectAndMask", &json!({"commit": true}))
}

#[test]
fn stop_ends_the_wait_and_adds_nothing() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    press(&mut app);
    send(&mut app, RefineHairEvent::Stop);
    assert!(edits(&app).is_empty() && !app.select_mask_protect.waiting());
    assert_eq!(app.ui.status, "Refine Hair stopped");
    assert_eq!(app.session.sam3_status(false), photocraft_engine::sam3_select::Sam3Status::Cancelled);
}

#[test]
fn closing_select_and_mask_stops_a_waiting_run() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    press(&mut app);
    cancel(&mut app);
    assert!(!app.select_mask_protect.waiting());
    assert_eq!(app.session.sam3_status(false), photocraft_engine::sam3_select::Sam3Status::Cancelled);
}

#[test]
fn an_encoding_that_fails_records_refine_hair_without_protection_and_says_so() {
    let fake = Fake::new();
    fake.fail.store(true, Ordering::SeqCst);
    let mut app = workspace(Some(&fake));
    press(&mut app);
    settle(&mut app);
    assert_eq!(edits(&app), [hair(false)]);
    assert!(app.ui.status.starts_with("Refine Hair ran without Protect Subject"), "{}", app.ui.status);
}
