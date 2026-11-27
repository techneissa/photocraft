//! The Control+Option drag holds the pointer at the limits (fork/brush-resize-drag.md). The framework's
//! `BrushResize` sets the diameter and level from the pointer's distance to the press, clamped, so dragging past a
//! limit and turning back changes nothing until the pointer has retraced the overshoot. `Held` feeds it a stand-in
//! pointer that moves by the real pointer's travel but never leaves the box where the values are inside their limits:
//! turning back changes the brush at once.

use craft::ui::canvas_controls::brush::{BrushDimensions, BrushResizeOptions};
use egui::{Pos2, Vec2, vec2};

/// The smallest diameter a resize sets, as the framework's.
const MIN_DIAMETER: f64 = 1.0;

/// The stand-in pointer of one resize.
#[derive(Clone, Copy, Debug)]
pub(super) struct Held {
    /// Where the real pointer was at the last call.
    last: Pos2,
    /// The stand-in, always inside `lo..=hi` from the press.
    at: Pos2,
    press: Pos2,
    lo: Vec2,
    hi: Vec2,
}

impl Held {
    /// The resize starts at the screen point `press` from `start`; `zoom` is the view's (screen points per document unit).
    pub(super) fn new(press: Pos2, start: BrushDimensions, options: &BrushResizeOptions, zoom: f64) -> Self {
        let zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
        let max = options.max_diameter.max(MIN_DIAMETER);
        // Diameter = start + 2 × offset / zoom; level = start + offset / travel (the framework's rule).
        let (left, right) = ((MIN_DIAMETER - start.diameter) / 2.0 * zoom, (max - start.diameter) / 2.0 * zoom);
        let travel = f64::from(options.travel);
        let (up, down) = (-start.level * travel, (1.0 - start.level) * travel);
        let lo = vec2(left.min(0.0) as f32, up.min(0.0) as f32);
        let hi = vec2(right.max(0.0) as f32, down.max(0.0) as f32);
        Held { last: press, at: press, press, lo, hi }
    }

    /// The stand-in after the real pointer moved to `to`: moved by the pointer's travel, held at the limits.
    pub(super) fn follow(&mut self, to: Pos2) -> Pos2 {
        if to.x.is_finite() && to.y.is_finite() {
            let moved = to - self.last;
            self.last = to;
            let off = (self.at - self.press + moved).clamp(self.lo, self.hi);
            self.at = self.press + off;
        }
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(zoom: f64) -> Held {
        // Diameter 40, level 0.5, 200 points of travel for the whole level range.
        Held::new(Pos2::new(100.0, 100.0), BrushDimensions::new(40.0, 0.5), &BrushResizeOptions::new(1000.0), zoom)
    }

    #[test]
    fn the_stand_in_stops_at_a_limit_and_turning_back_moves_it_at_once() {
        let mut h = held(1.0);
        // Far past the right limit (diameter 1000 is 480 points out), then 10 points back.
        h.follow(Pos2::new(2000.0, 100.0));
        let at_limit = h.follow(Pos2::new(2000.0, 100.0));
        assert_eq!(at_limit.x, 100.0 + 480.0, "held at the maximum diameter");
        let back = h.follow(Pos2::new(1990.0, 100.0));
        assert_eq!(back.x, at_limit.x - 10.0, "turning back moves it at once, not after retracing the overshoot");
        // Past the left limit (diameter 1 is 19.5 points left of the press) and the level limits.
        let left = h.follow(Pos2::new(-5000.0, 100.0));
        assert_eq!(left.x, 100.0 - 19.5);
        let top = h.follow(Pos2::new(-5000.0, -5000.0));
        assert_eq!(top.y, 100.0 - 100.0, "level 0 is 100 points up");
        let down = h.follow(Pos2::new(-5000.0, 9000.0));
        assert_eq!(down.y, 100.0 + 100.0, "level 1 is 100 points down");
    }

    #[test]
    fn zoom_scales_the_diameter_limits_only() {
        let h = held(2.0);
        assert_eq!((h.lo.x, h.hi.x), (-39.0, 960.0));
        assert_eq!((h.lo.y, h.hi.y), (-100.0, 100.0));
    }
}
