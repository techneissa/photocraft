//! Object Selection by dragged rectangle with SAM 3: the rectangle is SAM's box prompt (two corner
//! points, labels 2 and 3), and the model's mask for the main object inside it becomes the
//! selection, combined with the selection as it is by the drag's mode (⇧ add, ⌥ subtract, ⇧⌥
//! intersect, else replace). One History step. Classic's rectangle GrabCut answers whenever SAM 3
//! cannot (see [`super::click`] for the notes).

use photocraft_algo::selection::{SelectionMode, combine_region};
use photocraft_geom::Rect;
use serde_json::json;

use super::{Attempt, FAILED, ready_encoding};
use crate::{EngineError, Result, Session};

/// One dragged rectangle.
pub(crate) struct BoxRequest<'a> {
    /// The rectangle, in document pixels.
    pub rect: Rect,
    /// How the mask combines with the selection.
    pub mode: SelectionMode,
    pub label: &'a str,
    pub all_layers: bool,
}

/// Selects the main object inside the rectangle with SAM 3 when it can: ready encoding for the
/// active document, usable preference.
pub(crate) fn select_box(s: &mut Session, req: &BoxRequest) -> Result<Attempt> {
    let (image, canvas, step) = match ready_encoding(s, req.all_layers) {
        Ok(ready) => ready,
        Err(classic) => return Ok(classic),
    };
    let Some(rect) = super::mask::normalise_box(canvas, step, (image.w, image.h), req.rect) else { return Ok(Attempt::Classic(None)) };
    let backend = s.sam3.backend();
    let region = match backend.boxed(image, rect, &[]).map_err(|e| e.to_string()).and_then(|m| super::mask::to_region(&m, canvas, step).map_err(|e| e.to_string())) {
        Ok(region) => region,
        Err(_) => {
            // The image was freed, the model failed or has no box prompt: Classic answers.
            s.sam3.clear();
            return Ok(Attempt::Classic(Some(FAILED)));
        }
    };
    let Some(region) = region else {
        // Nothing found in the rectangle: the selection (and History) stay as they are.
        let selected = s.active().ok_or(EngineError::NoDocument)?.doc.selection.is_some();
        return Ok(Attempt::Done(json!({ "selected": selected, "changed": false, "engine": "sam3" })));
    };
    let bbox = region.bbox;
    let (doc_id, target) = {
        let d = s.active().ok_or(EngineError::NoDocument)?;
        (d.doc.id, super::sampled_target(&d.doc, d.active_layer, req.all_layers))
    };
    let mode = req.mode;
    let selected = s.edit(req.label, |doc, _| {
        doc.selection = combine_region(doc.selection.as_ref(), Some(&region), mode);
        Ok(doc.selection.is_some())
    })?;
    // The encoding stays valid past this selection edit; a click sequence from before does not.
    if let Some(doc) = s.active().filter(|d| d.doc.id == doc_id).map(|d| std::sync::Arc::clone(&d.doc))
        && let Some(key) = s.sam3.entry.key_mut()
    {
        key.holds(&doc, target);
    }
    s.sam3.sequence = None;
    Ok(Attempt::Done(
        json!({ "selected": selected, "changed": true, "bounds": [bbox.x0, bbox.y0, bbox.width() as i32, bbox.height() as i32], "engine": "sam3" }),
    ))
}
