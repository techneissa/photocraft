//! What a Curves adjustment clips, for the clipping display of `levels_clipping.rs`: the keys
//! (samples as 16-bit numbers) each channel's curve flattens to black at the low end and to white
//! at the high end.
//!
//! A sample clips to black when the curve returns black for it, the channel's own curve and then
//! the composite curve exactly as the compositor applies them (`tone_luts_depth`), rounded to the
//! document's levels. The clipped keys are the run that starts at 0 (black) or at the top
//! (white): that is what the black point and white point sliders set. A curve that comes back to
//! black or white elsewhere (an inverted curve) is not marked there. Beyond 0..1 a 32-bit
//! sample counts as the end it passes.
use photocraft_color::SampleType;
use photocraft_doc::Adjustment;
use photocraft_doc::adjust::ToneSpace;

use crate::levels_clipping::Thresholds;

/// A table value at `v` in 0..1 (linear between entries, as the compositor reads its LUTs).
fn at(table: &[f32], v: f32) -> f32 {
    let last = table.len().saturating_sub(1);
    let x = v.clamp(0.0, 1.0) * last as f32;
    let i = (x as usize).min(last);
    let j = (i + 1).min(last);
    let f = x - i as f32;
    match (table.get(i), table.get(j)) {
        (Some(a), Some(b)) => a * (1.0 - f) + b * f,
        _ => v,
    }
}

/// The [`Thresholds`] of the Curves `adj` in a document of `depth`. None for anything but Curves
/// in the RGB channel space (which Grayscale documents use too).
pub fn thresholds(adj: &Adjustment, depth: SampleType) -> Option<Thresholds> {
    let Adjustment::Curves { space: ToneSpace::Rgb, .. } = adj else { return None };
    let luts = photocraft_compose::adjust::tone_luts_depth(adj, Some(depth));
    let quantum = photocraft_compose::adjustment_quantum(depth);
    // The result as the document stores it: whole levels (None: 32-bit float, as is).
    let out = |table: &[f32], k: u16| {
        let v = at(table, f32::from(k) / 65535.0);
        match quantum {
            Some(q) => (v * q + 0.5).floor() / q,
            None => v,
        }
    };
    Some(std::array::from_fn(|c| {
        let Some(table) = luts.get(c).filter(|t| t.len() > 1) else { return (None, None) };
        let black = (0..=u16::MAX).take_while(|k| out(table, *k) <= 0.0).last();
        let white = (0..=u16::MAX).rev().take_while(|k| out(table, *k) >= 1.0).last();
        (black, white)
    }))
}

#[cfg(test)]
#[path = "curves_clipping_tests.rs"]
mod tests;
