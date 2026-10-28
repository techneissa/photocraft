//! Select ▸ Subject's engine choice: macOS's own subject cutout (Apple's Vision framework, through
//! `photocraft_macvision`) or PhotoCraft's own Classic one (`photocraft_algo::segment::subject`).
//!
//! The preference `tools.selectSubjectEngine` ([`SubjectEngine`]) picks:
//!
//! - `auto`: the Mac's cutout where there is one; Classic where there is none, or where the cutout
//!   finds no subject or fails;
//! - `macos`: the Mac's cutout. No subject found means nothing is selected; no cutout on this system
//!   (or Vision failing) means Classic, with a note for the status bar;
//! - `classic`: Classic, as before.
//!
//! The cutout sees the pixels the command samples (the active layer or the composite), shrunk to
//! Vision's size by [`Sampler::rgb_scaled`] (strips, so a 24 MP document is never held in memory
//! whole), and its soft mask is scaled back up to the canvas (bilinear) and becomes the region the
//! caller stores exactly like Classic's: soft edges stay partial selection.
//!
//! It runs inline on the command like Classic does: Vision takes about a second at most, and Select
//! Subject has no job path (its result is the selection, which scripts read right after).

use photocraft_algo::segment::{Sampler, scale_for, subject};
use photocraft_algo::selection::Region;
use photocraft_geom::Rect;
use photocraft_macvision::{self as macvision, Error, Mask};

use crate::prefs::SubjectEngine;

/// The cutout: RGBA8 pixels and their size in, a mask of that size out.
pub(crate) type CutoutFn = fn(&[u8], u32, u32) -> Result<Mask, Error>;

/// The note for a `macos` choice on a system with no cutout.
pub(crate) const UNAVAILABLE: &str = "macOS cutout unavailable, used Classic";
/// The note for a cutout that failed (Vision reported an error).
pub(crate) const FAILED: &str = "macOS cutout failed, used Classic";

/// Where the subject is, and what to tell the user about how it was found.
pub(crate) struct Found {
    pub region: Option<Region>,
    pub note: Option<&'static str>,
}

#[cfg(not(test))]
fn cutout() -> Option<CutoutFn> {
    macvision::available().then_some(macvision::subject_mask as CutoutFn)
}

// Tests never reach Vision unless they ask: the cutout is whatever the test installs (none by
// default), so every machine runs the same Classic tests.
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_CUTOUT: std::cell::Cell<Option<CutoutFn>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn cutout() -> Option<CutoutFn> {
    TEST_CUTOUT.with(std::cell::Cell::get)
}

/// The subject of `canvas` as seen by `smp`, found with the engine the preference chose.
pub(crate) fn subject_region(engine: SubjectEngine, smp: &dyn Sampler, canvas: Rect) -> Found {
    pick(engine, cutout(), smp, canvas)
}

fn pick(engine: SubjectEngine, cutout: Option<CutoutFn>, smp: &dyn Sampler, canvas: Rect) -> Found {
    let classic = |note| Found { region: subject::select_subject(smp, canvas), note };
    let Some(cut) = cutout.filter(|_| engine != SubjectEngine::Classic) else {
        return classic((engine == SubjectEngine::MacOs).then_some(UNAVAILABLE));
    };
    match macos_region(cut, smp, canvas) {
        Ok(Some(region)) => Found { region: Some(region), note: None },
        // Nothing found: Auto lets Classic try; an explicit macOS choice says so.
        Ok(None) | Err(Error::NoSubject) if engine == SubjectEngine::MacOs => Found { region: None, note: None },
        Ok(None) | Err(Error::NoSubject) => classic(None),
        Err(_) => classic(Some(FAILED)),
    }
}

/// Run `cut` on the canvas as Vision wants it and turn its mask into a region. `Ok(None)` when the
/// mask is empty.
fn macos_region(cut: CutoutFn, smp: &dyn Sampler, canvas: Rect) -> Result<Option<Region>, Error> {
    let step = scale_for(canvas, macvision::MAX_VISION_PIXELS as usize);
    let img = smp.rgb_scaled(canvas, step);
    let (w, h) = (u32::try_from(img.w).map_err(|_| Error::BadInput)?, u32::try_from(img.h).map_err(|_| Error::BadInput)?);
    let rgba: Vec<u8> = img.px.iter().flat_map(|p| [p[0], p[1], p[2], 1.0].map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)).collect();
    let mask = cut(&rgba, w, h)?;
    Ok(region_from_mask(&mask, canvas, step))
}

/// The region of `mask`, a `ceil(w / step) × ceil(h / step)` picture of `canvas` (each of its pixels
/// stands for a `step × step` block): scaled up by exactly `step`, cropped to the canvas, and cut
/// down to the box of the pixels it covers. `None` when it covers none, or does not fit `canvas`.
pub(crate) fn region_from_mask(mask: &Mask, canvas: Rect, step: usize) -> Option<Region> {
    let (cw, ch) = (usize::try_from(canvas.width()).ok()?, usize::try_from(canvas.height()).ok()?);
    let step = step.max(1);
    let (fw, fh) = (usize::try_from(mask.width).ok()?.checked_mul(step)?, usize::try_from(mask.height).ok()?.checked_mul(step)?);
    // The mask's blocks cover the canvas, and the last block is partial by less than a step.
    if cw == 0 || ch == 0 || fw < cw || fh < ch || fw >= cw.saturating_add(step) || fh >= ch.saturating_add(step) {
        return None;
    }
    let full = if step == 1 { mask.clone() } else { mask.resized(u32::try_from(fw).ok()?, u32::try_from(fh).ok()?)? };
    let stride = usize::try_from(full.width).ok()?;
    let (mut x0, mut y0, mut x1, mut y1) = (cw, ch, 0, 0);
    for y in 0..ch {
        let start = y.checked_mul(stride)?;
        let row = full.data.get(start..start.checked_add(cw)?)?;
        if let (Some(first), Some(last)) = (row.iter().position(|v| *v > 0), row.iter().rposition(|v| *v > 0)) {
            (x0, x1) = (x0.min(first), x1.max(last + 1));
            (y0, y1) = (y0.min(y), y1.max(y + 1));
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let mut data = Vec::with_capacity((x1 - x0).checked_mul(y1 - y0)?);
    for y in y0..y1 {
        let start = y.checked_mul(stride)?;
        data.extend_from_slice(full.data.get(start.checked_add(x0)?..start.checked_add(x1)?)?);
    }
    let at = |origin: i32, offset: usize| origin.checked_add(i32::try_from(offset).ok()?);
    let bbox = Rect::new(at(canvas.x0, x0)?, at(canvas.y0, y0)?, at(canvas.x0, x1)?, at(canvas.y0, y1)?);
    Some(Region { bbox, mask: data })
}

#[cfg(test)]
mod tests;
