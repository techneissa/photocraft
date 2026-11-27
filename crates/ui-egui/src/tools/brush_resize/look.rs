//! Photoshop's on-canvas resize look (fork/brush-resize-drag.md): the label always shows the diameter, the hardness
//! and the opacity together, not the level the drag changes only. The red preview fill is the framework's falloff
//! preview in the Brush Preview colour at `OVERLAY_ALPHA`.

use craft::ui::canvas::ToolContext;
use craft::ui::canvas_controls::CanvasControlWords;
use craft::ui::runtime::env::{Label, UnitKind};
use photocraft_paint::BrushSettings;

/// The overlay's alpha: about half, as Photoshop's.
pub(super) const OVERLAY_ALPHA: u8 = 128;

/// The label's rows for `brush`, which already holds the resize's current values: Diameter, Hardness and Opacity.
pub(super) fn rows(cx: &mut ToolContext<'_>, brush: &BrushSettings) -> Vec<(Label, String)> {
    let words = cx.words::<CanvasControlWords>();
    let units = cx.units();
    let percent = |level: f32| units.text(UnitKind::Percent, f64::from(level) * 100.0, 0..=0);
    vec![
        (words.diameter.clone(), units.pixels(f64::from(brush.size).round().max(0.0))),
        (words.hardness.clone(), percent(brush.hardness)),
        (words.opacity.clone(), percent(brush.opacity)),
    ]
}
