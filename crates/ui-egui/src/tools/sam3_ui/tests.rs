//! The shell's SAM 3 steps against a fake backend (no model): a click is a disc of radius 30.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use photocraft_engine::sam3_select::{EncodedImage, Mask, PointClick, Sam3Backend};
use serde_json::json;

use super::*;
use crate::canvas::{ToolEvent, tool_event};

struct Fake {
    gate_open: AtomicBool,
    encodes: AtomicU64,
    current: Mutex<Option<u64>>,
}

impl Fake {
    fn new() -> Arc<Self> {
        Arc::new(Self { gate_open: AtomicBool::new(true), encodes: AtomicU64::new(0), current: Mutex::new(None) })
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
        let token = self.encodes.fetch_add(1, Ordering::SeqCst) + 1;
        *self.current() = Some(token);
        Ok(EncodedImage { token, w: rgb.w, h: rgb.h })
    }
    fn is_current(&self, image: EncodedImage) -> bool {
        *self.current() == Some(image.token)
    }
    fn clicks(&self, image: EncodedImage, clicks: &[PointClick]) -> Result<Mask, photocraft_ml::MlError> {
        let (w, h) = (image.w, image.h);
        let mut alpha = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                let on = |c: &PointClick| {
                    let (dx, dy) = (x as f32 + 0.5 - c.x * w as f32, y as f32 + 0.5 - c.y * h as f32);
                    dx * dx + dy * dy <= 30.0 * 30.0
                };
                let inside = clicks.iter().any(|c| c.positive && on(c)) && !clicks.iter().any(|c| !c.positive && on(c));
                alpha[y * w + x] = if inside { 1.0 } else { 0.0 };
            }
        }
        Ok(Mask { w, h, alpha })
    }
    fn release(&self, image: EncodedImage) {
        let mut cur = self.current();
        if *cur == Some(image.token) {
            *cur = None;
        }
    }
}

/// A 200 × 160 document with a red disc (centre 100, 80, radius 45) on blue, the Object Selection
/// tool picked, and a SAM 3 fake installed.
fn app_with(fake: &Arc<Fake>) -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.session.execute("file.new", json!({"width": 200, "height": 160})).unwrap();
    app.session
        .edit("paint", |doc, a| {
            let s = doc.layer_mut(a.unwrap()).unwrap().surface_mut().unwrap();
            s.fill_rect(photocraft_geom::Rect::new(0, 0, 200, 160), &[0.2, 0.4, 0.8, 1.0]);
            for y in 0..160 {
                for x in 0..200 {
                    let (dx, dy) = (x as f32 + 0.5 - 100.0, y as f32 + 0.5 - 80.0);
                    if dx * dx + dy * dy <= 45.0 * 45.0 {
                        s.fill_rect(photocraft_geom::Rect::new(x, y, x + 1, y + 1), &[0.9, 0.2, 0.1, 1.0]);
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    app.session.set_sam3_backend(fake.clone());
    app.ui.tool = Tool::ObjectSelection;
    app
}

fn settle(app: &mut PhotocraftApp) {
    for _ in 0..400 {
        if !step(app, false, false) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn cov(app: &PhotocraftApp, x: i32, y: i32) -> f32 {
    app.session.active().unwrap().doc.selection.as_ref().map_or(0.0, |s| s.sample_channel(x, y, 0))
}

fn click(app: &mut PhotocraftApp, at: (f64, f64), m: egui::Modifiers) {
    tool_event(app, ToolEvent::Down { x: at.0, y: at.1, pressure: 1.0 }, m);
    tool_event(app, ToolEvent::Up { x: at.0, y: at.1 }, m);
}

#[test]
fn picking_the_tool_encodes_in_the_background_and_says_so() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = app_with(&fake);
    assert!(step(&mut app, false, false), "keep frames coming while it runs");
    assert_eq!(app.ui.status, "Preparing SAM 3 for this image… (Esc stops it)");
    // Classic answers meanwhile, with a note
    click(&mut app, (100.0, 80.0), egui::Modifiers::NONE);
    assert_eq!(app.ui.status, "SAM 3 is still preparing, used Classic");
    assert!(cov(&app, 60, 80) >= 0.5, "the Classic finder took the whole disc");
    fake.gate_open.store(true, Ordering::SeqCst);
    settle(&mut app);
    assert_eq!(app.session.sam3_status(false), Sam3Status::Ready);
    assert_eq!(app.ui.status, "SAM 3 is ready");
    assert_eq!(fake.encodes.load(Ordering::SeqCst), 1);
    // no second encoding while nothing changes
    assert!(!step(&mut app, false, false));
    assert_eq!(fake.encodes.load(Ordering::SeqCst), 1);
}

#[test]
fn other_tools_do_not_encode() {
    let fake = Fake::new();
    let mut app = app_with(&fake);
    app.ui.tool = Tool::Lasso;
    assert!(!step(&mut app, false, false));
    assert_eq!(app.session.sam3_status(false), Sam3Status::Idle);
    app.ui.tool = Tool::QuickSelection;
    settle(&mut app);
    assert_eq!(app.session.sam3_status(false), Sam3Status::Ready, "Quick Selection encodes too");
}

#[test]
fn esc_stops_it_and_only_picking_the_tool_again_starts_it() {
    let fake = Fake::new();
    fake.gate_open.store(false, Ordering::SeqCst);
    let mut app = app_with(&fake);
    step(&mut app, false, false);
    assert!(!step(&mut app, true, false));
    assert_eq!(app.session.sam3_status(false), Sam3Status::Cancelled);
    assert_eq!(app.ui.status, "SAM 3 stopped, Classic selects for now");
    fake.gate_open.store(true, Ordering::SeqCst);
    for _ in 0..5 {
        assert!(!step(&mut app, false, false), "frames do not undo Esc");
    }
    assert_eq!(app.session.sam3_status(false), Sam3Status::Cancelled);
    app.ui.tool = Tool::Lasso;
    step(&mut app, false, false);
    app.ui.tool = Tool::ObjectSelection;
    settle(&mut app);
    assert_eq!(app.session.sam3_status(false), Sam3Status::Ready);
}

#[test]
fn a_pixel_edit_while_the_tool_stays_picked_encodes_again() {
    let fake = Fake::new();
    let mut app = app_with(&fake);
    settle(&mut app);
    app.session.execute("paint.pencil", json!({"points": [[5, 5], [30, 5]], "size": 4, "color": "#000000"})).unwrap();
    assert_eq!(app.session.sam3_status(false), Sam3Status::Idle);
    settle(&mut app);
    assert_eq!(app.session.sam3_status(false), Sam3Status::Ready);
    assert_eq!(fake.encodes.load(Ordering::SeqCst), 2);
}

#[test]
fn a_model_freed_while_idle_comes_back_with_a_press_not_by_itself() {
    let fake = Fake::new();
    let mut app = app_with(&fake);
    settle(&mut app);
    *fake.current() = None;
    for _ in 0..5 {
        step(&mut app, false, false);
    }
    assert_eq!(app.session.sam3_status(false), Sam3Status::Unloaded);
    assert_eq!(fake.encodes.load(Ordering::SeqCst), 1);
    step(&mut app, false, true);
    settle(&mut app);
    assert_eq!(app.session.sam3_status(false), Sam3Status::Ready);
    assert_eq!(fake.encodes.load(Ordering::SeqCst), 2);
}

#[test]
fn hovering_shows_the_sam3_object_once_it_is_ready() {
    let fake = Fake::new();
    let mut app = app_with(&fake);
    settle(&mut app);
    let mut t = 100.0;
    let shown = loop {
        let u = crate::object_finder::update(&mut app, Some([100.0, 80.0]), false, t);
        if !u.busy && u.shown.is_some() || t > 130.0 {
            break u.shown;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        t += 0.05;
    };
    let r = shown.expect("a highlighted object");
    // SAM 3's disc (radius 30) and not the Classic finder's (radius 45)
    assert!(r.at(100, 80) > 0.99 && r.at(100 + 25, 80) > 0.99 && r.at(100 + 40, 80) == 0.0, "{:?}", r.bbox);
    assert!(app.session.object_finder.is_empty(), "Classic's cache is kept for Classic's results");
}

#[test]
fn shift_and_alt_clicks_refine_the_same_mask_with_sam3() {
    let fake = Fake::new();
    let mut app = app_with(&fake);
    settle(&mut app);
    click(&mut app, (60.0, 80.0), egui::Modifiers::NONE);
    assert!(cov(&app, 60, 80) > 0.99 && cov(&app, 100, 80) == 0.0, "SAM 3's object, not the Classic disc");
    click(&mut app, (130.0, 80.0), egui::Modifiers::SHIFT);
    assert_eq!(app.session.journal.last().map(|(_, p)| p["refine"].clone()), Some(json!("add")));
    assert!(cov(&app, 60, 80) > 0.99 && cov(&app, 130, 80) > 0.99);
    click(&mut app, (60.0, 80.0), egui::Modifiers::ALT);
    assert_eq!(app.session.journal.last().map(|(_, p)| p["refine"].clone()), Some(json!("subtract")));
    assert!(cov(&app, 60, 80) == 0.0 && cov(&app, 130, 80) > 0.99);
    // each click is one step
    for _ in 0..3 {
        app.session.execute("edit.undo", json!({})).unwrap();
    }
    assert_eq!(cov(&app, 130, 80), 0.0);
    // a plain click sends no refine
    click(&mut app, (60.0, 80.0), egui::Modifiers::NONE);
    assert!(app.session.journal.last().is_some_and(|(_, p)| p.get("refine").is_none()));
}

#[test]
fn a_quick_selection_stroke_is_sam3_clicks() {
    let fake = Fake::new();
    let mut app = app_with(&fake);
    app.ui.tool = Tool::QuickSelection;
    settle(&mut app);
    tool_event(&mut app, ToolEvent::Down { x: 70.0, y: 80.0, pressure: 1.0 }, egui::Modifiers::NONE);
    tool_event(&mut app, ToolEvent::Move { x: 90.0, y: 80.0, pressure: 1.0 }, egui::Modifiers::NONE);
    tool_event(&mut app, ToolEvent::Up { x: 90.0, y: 80.0 }, egui::Modifiers::NONE);
    assert!(cov(&app, 70, 80) > 0.99 && cov(&app, 100, 80) > 0.99 && cov(&app, 100 + 40, 80) == 0.0, "SAM 3's mask, not GrabCut's");
}

/// The real model through the shell's own steps, timed: picking the tool until SAM 3 is ready,
/// the hover preview and a click. By hand (needs the model and about 20 GB of memory):
/// `PHOTOCRAFT_SAM3_IMAGE=photo.jpg PHOTOCRAFT_SAM3_POINT=990,900 cargo test -p photocraft-ui-egui --lib -- --ignored real_model --nocapture`
#[test]
#[ignore = "needs the SAM 3 model and about 20 GB of memory"]
fn real_model_timings_in_the_shell() {
    let file = std::env::var("PHOTOCRAFT_SAM3_IMAGE").expect("PHOTOCRAFT_SAM3_IMAGE=photo.jpg");
    let point: Vec<f64> = std::env::var("PHOTOCRAFT_SAM3_POINT").expect("PHOTOCRAFT_SAM3_POINT=x,y").split(',').map(|v| v.trim().parse().unwrap()).collect();
    let imported = photocraft_io::import("photo.jpg", &std::fs::read(file).unwrap()).unwrap();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.session.add_document(imported.document, None);
    app.ui.tool = Tool::ObjectSelection;
    let t = std::time::Instant::now();
    let mut frames = 0;
    while step(&mut app, false, false) {
        frames += 1;
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    println!("pick tool → ready: {:?} ({frames} frames, status {:?}, bar: {})", t.elapsed(), app.session.sam3_status(false), app.ui.status);
    assert_eq!(app.session.sam3_status(false), Sam3Status::Ready);
    let t = std::time::Instant::now();
    let mut now = 100.0;
    let shown = loop {
        let u = crate::object_finder::update(&mut app, Some([point[0], point[1]]), false, now);
        if u.shown.is_some() || now > 130.0 {
            break u.shown;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        now += 0.05;
    };
    println!(
        "hover: pointer at rest → highlight {:?} (includes the {:?} debounce; the lookup is one click)",
        t.elapsed(),
        std::time::Duration::from_secs_f64(crate::object_finder::DEBOUNCE)
    );
    assert!(shown.is_some());
    let t = std::time::Instant::now();
    click(&mut app, (point[0], point[1]), egui::Modifiers::NONE);
    println!("click: {:?} (journal: {:?})", t.elapsed(), app.session.journal.last().map(|(id, p)| (id.clone(), p.clone())));
    assert!(app.session.active().unwrap().doc.selection.is_some());
}
