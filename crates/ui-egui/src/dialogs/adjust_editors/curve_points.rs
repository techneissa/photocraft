//! The Curves dialog's black point and white point sliders under the graph, and its Show Clipping
//! checkbox. The sliders move the curve's first and last point along the input axis, as
//! Photoshop's do; Alt-dragging one (or dragging one with Show Clipping on) shows what the curve
//! clips on the canvas (`levels_clipping.rs`, `curves_clipping.rs`). The framework's Curves panel has
//! neither, so the host draws them in a row of its own under the panel (fork/levels-curves-clipping.md).
use craft::behavior::{EditEvents, Mixed};
use craft::imaging::ui::adjust::{LevelsClip, ToneCurves};
use craft::imaging::ui::curve::{Curve, CurveRules};
use craft::ui::choice::Checkbox;
use craft::ui::contract::{Bind, Props, Show};
use craft::ui::value_drag::{Order, Thumb, ThumbRow};

use super::AdjustValue;

/// Whole levels, as the panel's own rules (`curves_panel.rs`).
const RULES: CurveRules = CurveRules::new(255.0, 16);

/// The input of the first and last point of `curve` (black point, white point); `None` for a drawn curve.
pub(super) fn ends(curve: &Curve) -> Option<[f64; 2]> {
    let points = curve.points()?;
    Some([f64::from(points.first()?[0]), f64::from(points.last()?[0])])
}

/// `curve` with its first point's input at `black` and its last at `white` (whole levels, each strictly between its
/// neighbours' inputs); the outputs and the corner flags stay.
pub(super) fn with_ends(curve: &Curve, [black, white]: [f64; 2]) -> Curve {
    let Some(points) = curve.points() else { return curve.clone() };
    let mut points = points.to_vec();
    let corners: Vec<bool> = (0..points.len()).map(|i| curve.is_corner(i)).collect();
    if let Some(last) = points.len().checked_sub(1) {
        let (first_out, last_out) = (points[0][1], points[last][1]);
        RULES.move_to(&mut points, 0, [black as f32, first_out]);
        RULES.move_to(&mut points, last, [white as f32, last_out]);
    }
    Curve::from_parts(points, corners)
}

/// The clipping display of a drag on thumb `thumb` (0 black point, 1 white point): while Alt is held, or Show Clipping
/// is on.
pub(super) fn clip_of(thumb: usize, alt: bool, show: bool) -> Option<LevelsClip> {
    (alt || show).then_some(if thumb == 0 { LevelsClip::Shadows } else { LevelsClip::Highlights })
}

/// What [`point_row`] reports.
pub(super) struct PointRow {
    /// The edits of the sliders, as dialog values.
    pub edit: EditEvents<AdjustValue>,
    /// The clipping display the sliders ask for now.
    pub clipping: Option<LevelsClip>,
}

/// The black point and white point sliders and the Show Clipping checkbox for the `channel` curve of `curves`. A drag
/// is written into `curves` as it goes. Shows nothing for a drawn curve (pencil mode has no points).
pub(super) fn point_row(ui: &mut egui::Ui, curves: &mut ToneCurves, channel: usize) -> PointRow {
    let mut row = PointRow { edit: EditEvents::new(), clipping: None };
    let Some(current) = curves.curves.get(channel).and_then(ends) else { return row };
    let id = ui.make_persistent_id("curves-show-clipping");
    let mut show = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if Checkbox::new(Bind::bound(&mut show)).label(tl!("Show Clipping")).show(ui).value().is_some() {
        ui.data_mut(|d| d.insert_temp(id, show));
    }
    let thumbs = [
        Thumb::new(tl!("Black Point")).order(Order::between(None, Some(1), 1.0)).alt_reports(),
        Thumb::new(tl!("White Point")).order(Order::between(Some(0), None, 1.0)).alt_reports(),
    ];
    let out = ThumbRow::new(Bind::given(current.map(Mixed::Same)), 0.0..=255.0).step(1.0).thumbs(thumbs).show(ui);
    let base = curves.clone();
    let lift = |pair: [Mixed<f64>; 2]| {
        let [Mixed::Same(black), Mixed::Same(white)] = pair else { return AdjustValue::Curves { curves: base.clone() } };
        let mut next = base.clone();
        if let Some(slot) = next.curves.get_mut(channel) {
            *slot = with_ends(slot, [black, white]);
        }
        AdjustValue::Curves { curves: next }
    };
    for event in out.edit.events.iter() {
        if let Some(AdjustValue::Curves { curves: landed }) = event.clone().map(lift).landed() {
            *curves = landed.clone();
        }
    }
    row.edit = out.edit.events.map(lift);
    row.clipping = out.held.and_then(|held| clip_of(held.thumb, held.mods.alt, show));
    row
}

#[cfg(test)]
mod tests {
    /// The sliders move the first and last point along the input axis and keep their outputs; a drag in the
    /// framework's point rules stays strictly between neighbours.
    #[test]
    fn with_ends_moves_only_the_end_inputs() {
        use super::*;
        let curve = Curve::smooth(vec![[0.0, 10.0], [128.0, 140.0], [255.0, 250.0]]);
        let moved = with_ends(&curve, [30.0, 220.0]);
        assert_eq!(moved.points().unwrap(), &[[30.0, 10.0], [128.0, 140.0], [220.0, 250.0]]);
        assert_eq!(ends(&moved), Some([30.0, 220.0]));
        let squeezed = with_ends(&curve, [200.0, 100.0]);
        let p = squeezed.points().unwrap();
        assert!(p[0][0] < p[1][0] && p[1][0] < p[2][0], "{p:?}");
        assert_eq!(clip_of(0, true, false), Some(LevelsClip::Shadows));
        assert_eq!(clip_of(1, false, true), Some(LevelsClip::Highlights));
        assert_eq!(clip_of(1, false, false), None);
    }
}
