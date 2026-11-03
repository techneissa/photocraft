//! Control+Option drags driven the way a person makes them, through the real canvas: the
//! diameter and hardness must follow the pointer on every step, in both directions, with no
//! stretch of pointer travel that changes nothing.

use egui::{Modifiers, PointerButton, Pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

use super::*;

const CTRL_ALT: Modifiers = Modifiers { alt: true, ctrl: true, shift: false, mac_cmd: false, command: false };

struct Drive {
    h: Harness<'static, PhotocraftApp>,
    /// Where the pointer is, in screen points.
    at: Pos2,
    /// What the brush should be: the size and hardness the pointer's travel has set.
    size: f32,
    hardness: f32,
    zoom: f32,
}

impl Drive {
    fn new(ppp: f32, size: f32, hardness: f32) -> Self {
        let mut a = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        a.run("file.new", json!({"width": 300, "height": 200, "background": "transparent"})).unwrap();
        a.run("tools.setBrush", json!({"brush": {"size": size, "hardness": hardness}})).unwrap();
        a.ui.tool = Tool::Brush;
        a.sync_views();
        let mut h = Harness::builder().with_size(vec2(1000.0, 700.0)).with_pixels_per_point(ppp).build_ui_state(
            |ui, app: &mut PhotocraftApp| {
                let ctx = ui.ctx().clone();
                if !ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                    return;
                }
                egui::CentralPanel::default().show(ui, |ui| crate::canvas::document_area(app, ui));
            },
            a,
        );
        PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::default());
        h.run_steps(4);
        let at = h.state().last_canvas_rect.center();
        let zoom = ViewXform::active(h.state()).expect("a view").zoom;
        Drive { h, at, size, hardness, zoom }
    }

    fn press(&mut self) {
        self.h.event(egui::Event::ModifiersChanged(CTRL_ALT));
        self.h.event(egui::Event::PointerMoved(self.at));
        self.h.run_steps(1);
        self.h.event(egui::Event::PointerButton { pos: self.at, button: PointerButton::Primary, pressed: true, modifiers: CTRL_ALT });
        self.h.run_steps(2);
        assert!(self.h.state().brush_resize.is_some(), "the press began a resize");
    }

    fn release(&mut self) {
        self.h.event(egui::Event::PointerButton { pos: self.at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        self.h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
        self.h.run_steps(2);
        assert!(self.h.state().brush_resize.is_none(), "the release ended the resize");
    }

    /// Move the pointer by `d` screen points in one frame, and check the brush against the model:
    /// the pointer's travel moves the diameter by `2 × travel / zoom` and the hardness by
    /// `travel / 200`, held at the limits, and leaving a limit starts changing at once.
    fn step(&mut self, d: egui::Vec2) {
        self.at += d;
        self.h.event(egui::Event::PointerMoved(self.at));
        self.h.run_steps(1);
        self.size = (self.size + 2.0 * d.x / self.zoom).clamp(1.0, MAX_BRUSH_SIZE);
        self.hardness = (self.hardness + d.y / HARDNESS_TRAVEL).clamp(0.0, 1.0);
        self.check();
    }

    fn check(&self) {
        let b = &self.h.state().session.tools.brush;
        assert!((b.size - self.size).abs() <= 1.0, "diameter {} should follow the pointer to {} (pointer at {:?})", b.size, self.size, self.at);
        assert!(
            (b.hardness - self.hardness).abs() <= 0.011,
            "hardness {} should follow the pointer to {} (pointer at {:?})",
            b.hardness,
            self.hardness,
            self.at
        );
    }
}

/// Small moves to one limit and past it, then back across the whole range to the other limit and
/// past that, then back again, checking the brush at every step.
fn sweep(ppp: f32, step: f32) {
    let mut d = Drive::new(ppp, 40.0, 0.5);
    d.press();
    let reach = (MAX_BRUSH_SIZE * d.zoom / 2.0) / step;
    for dir in [1.0, -1.0, 1.0] {
        let mut n = 0.0;
        // Right until the maximum (or left until 1 px), then 12 more steps past it.
        while n < 2.0 * reach + 14.0 {
            let at_limit = if dir > 0.0 { d.size >= MAX_BRUSH_SIZE } else { d.size <= 1.0 };
            d.step(vec2(dir * step, 0.0));
            n += 1.0;
            if at_limit {
                break;
            }
        }
        for _ in 0..12 {
            d.step(vec2(dir * step, 0.0));
        }
        let b = d.h.state().session.tools.brush.size;
        assert_eq!(b, if dir > 0.0 { MAX_BRUSH_SIZE } else { 1.0 }, "the sweep reached the limit");
    }
    d.release();
}

#[test]
fn the_diameter_follows_small_moves_from_1_px_to_the_maximum_and_back() {
    sweep(1.0, 40.0);
}

#[test]
fn the_diameter_follows_on_a_2x_screen() {
    sweep(2.0, 40.0);
}

#[test]
fn moving_back_over_the_start_and_out_the_other_side_keeps_following() {
    let mut d = Drive::new(1.0, 40.0, 0.5);
    d.press();
    for _ in 0..30 {
        d.step(vec2(2.0, 0.0));
    }
    // Left past where the drag began (the diameter hits 1 px well before), far out, and back.
    for _ in 0..90 {
        d.step(vec2(-2.0, 0.0));
    }
    for _ in 0..60 {
        d.step(vec2(2.0, 0.0));
    }
    for _ in 0..60 {
        d.step(vec2(-2.0, 0.0));
    }
    d.release();
}

#[test]
fn the_hardness_follows_up_and_down_past_its_limits() {
    let mut d = Drive::new(1.0, 40.0, 0.5);
    d.press();
    for dir in [1.0, -1.0, 1.0, -1.0] {
        for _ in 0..70 {
            d.step(vec2(0.0, dir * 5.0));
        }
    }
    d.release();
}

#[test]
fn a_pause_and_a_zig_zag_change_nothing_but_the_pointer_travel() {
    let mut d = Drive::new(1.0, 40.0, 0.5);
    d.press();
    for _ in 0..10 {
        d.step(vec2(3.0, 1.0));
    }
    for _ in 0..30 {
        d.h.run_steps(1);
        d.check();
    }
    for _ in 0..20 {
        d.step(vec2(-4.0, -2.0));
        d.step(vec2(1.0, 2.0));
    }
    d.release();
}

#[test]
fn letting_go_of_the_keys_before_the_mouse_keeps_following() {
    for (first, then) in [
        (Modifiers { alt: true, ..Modifiers::NONE }, Modifiers::NONE),
        (Modifiers { ctrl: true, ..Modifiers::NONE }, Modifiers::NONE),
        (Modifiers::NONE, Modifiers::NONE),
    ] {
        let mut d = Drive::new(1.0, 40.0, 0.5);
        d.press();
        for _ in 0..8 {
            d.step(vec2(5.0, 0.0));
        }
        d.h.event(egui::Event::ModifiersChanged(first));
        d.h.run_steps(1);
        for _ in 0..8 {
            d.step(vec2(5.0, 3.0));
        }
        d.h.event(egui::Event::ModifiersChanged(then));
        d.h.run_steps(1);
        for _ in 0..40 {
            d.step(vec2(-5.0, -1.0));
        }
        for _ in 0..40 {
            d.step(vec2(5.0, 1.0));
        }
        d.release();
    }
}

#[test]
fn a_fast_drag_in_big_jumps_lands_where_the_pointer_did() {
    let mut d = Drive::new(1.0, 40.0, 0.5);
    d.press();
    for dx in [300.0, -500.0, 800.0, -50.0, -900.0, 450.0] {
        d.step(vec2(dx, 0.0));
    }
    d.release();
}
