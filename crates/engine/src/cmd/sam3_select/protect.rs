//! Refine Hair's Protect Subject: before the matting is solved, SAM 3 is asked for the subject
//! inside the selection's bounds (the box prompt of Object Selection's rectangle, on the encoded
//! image Object Selection shares), and that mask, pulled in a little, is sure foreground in
//! Refine Hair's trimap, so a deep shadow at the edge (under the nose) is never cut away.
//!
//! [`SubjectFinder`] is the part that crosses threads: the backend, the encoding and the canvas it
//! covers, cheap to clone. The Select and Mask preview works on a reduced copy of the document
//! (`k` times smaller), so a finder is told `k`: it takes the selection's bounds in the copy's
//! pixels and answers in them. Taking the finder never waits for an encoding; when none is ready
//! it says why not ([`ProtectSource::Waiting`] while one is running).

use std::sync::Arc;

use photocraft_algo::selection::Region;
use photocraft_geom::Rect;
use photocraft_ml::MlError;

use super::backend::{EncodedImage, Sam3Backend};
use super::{Entry, Sam3Status, mask};
use crate::Session;

/// A ready SAM 3 encoding to ask for the subject inside a box.
#[derive(Clone)]
pub struct SubjectFinder {
    backend: Arc<dyn Sam3Backend>,
    image: EncodedImage,
    canvas: Rect,
    step: usize,
    /// How many times smaller than the encoded canvas the document asked about is.
    k: u32,
}

impl PartialEq for SubjectFinder {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.backend, &other.backend) && self.image == other.image && self.canvas == other.canvas && self.step == other.step && self.k == other.k
    }
}

impl std::fmt::Debug for SubjectFinder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubjectFinder").field("k", &self.k).finish_non_exhaustive()
    }
}

impl SubjectFinder {
    /// The finder for a document `k` times smaller than the encoded one (1 = the same size).
    #[must_use]
    pub fn reduced(&self, k: u32) -> Self {
        Self { k: k.max(1), ..self.clone() }
    }

    /// The subject SAM 3 finds inside `rect` (document pixels of the copy this finder is for):
    /// `None` when it finds nothing there or the encoding is gone. Answers in the same pixels.
    pub fn find(&self, rect: Rect) -> Result<Option<Region>, MlError> {
        let k = self.k.max(1) as i32;
        let full = Rect::new(rect.x0.saturating_mul(k), rect.y0.saturating_mul(k), rect.x1.saturating_mul(k), rect.y1.saturating_mul(k));
        let Some(rect) = mask::normalise_box(self.canvas, self.step, (self.image.w, self.image.h), full) else { return Ok(None) };
        if !self.backend.is_current(self.image) {
            return Ok(None);
        }
        let m = self.backend.boxed(self.image, rect, &[])?;
        let region = mask::to_region(&m, self.canvas, self.step).map_err(|e| MlError::Backend(e.to_string()))?;
        Ok(region.map(|r| if k > 1 { shrink(&r, k as u32) } else { r }))
    }
}

/// Whether a Refine Hair edit protects its subject, and with what.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ProtectSubject {
    /// Today's Refine Hair.
    #[default]
    Off,
    /// Asked for (`"protectSubject": true`), not yet given a finder: [`attach`](crate::select_mask_edits::attach)
    /// does. An edit still in this state runs as `Off`.
    Asked,
    /// Protects with SAM 3's subject.
    With(SubjectFinder),
}

/// `region` for a canvas `k` times smaller: each pixel the average of the `k × k` block it covers.
pub(crate) fn shrink(region: &Region, k: u32) -> Region {
    let k = k.max(1) as i32;
    let (b, kk) = (region.bbox, i64::from(k) * i64::from(k));
    let out = Rect::new(b.x0.div_euclid(k), b.y0.div_euclid(k), (b.x1 + k - 1).div_euclid(k), (b.y1 + k - 1).div_euclid(k));
    let (w, bw) = (out.width() as usize, b.width() as usize);
    let mut mask = vec![0u8; w * out.height() as usize];
    for (i, o) in mask.iter_mut().enumerate() {
        let (x, y) = (out.x0 + (i % w) as i32, out.y0 + (i / w) as i32);
        let mut sum = 0i64;
        for yy in y * k..(y + 1) * k {
            for xx in x * k..(x + 1) * k {
                if b.contains(xx, yy) {
                    sum += i64::from(region.mask.get((yy - b.y0) as usize * bw + (xx - b.x0) as usize).copied().unwrap_or(0));
                }
            }
        }
        *o = ((sum + kk / 2) / kk) as u8;
    }
    Region { bbox: out, mask }
}

/// What Protect Subject can do for the active document right now.
#[derive(Debug)]
pub enum ProtectSource {
    /// A ready encoding to ask.
    Ready(SubjectFinder),
    /// The encoding is running (the shell shows "Preparing SAM 3…" and waits).
    Waiting,
    /// SAM 3 cannot be used: not installed, no memory, the preference says Classic, or the
    /// encoding failed or was stopped; the note says why when there is one.
    Unavailable(Option<&'static str>),
}

impl Session {
    /// Protect Subject's source for the active document sampling the selected layer (`all_layers`:
    /// the composite). Never blocks; it starts no encoding (see [`Session::sam3_prepare`]).
    pub fn sam3_protect(&mut self, all_layers: bool) -> ProtectSource {
        match self.sam3_status(all_layers) {
            Sam3Status::Ready => {}
            Sam3Status::Preparing => return ProtectSource::Waiting,
            Sam3Status::Unavailable(note) => return ProtectSource::Unavailable(note),
            Sam3Status::Failed(note) => return ProtectSource::Unavailable(Some(note)),
            Sam3Status::Idle | Sam3Status::Cancelled | Sam3Status::Unloaded => return ProtectSource::Unavailable(None),
        }
        let Entry::Ready(r) = &self.sam3.entry else { return ProtectSource::Unavailable(None) };
        let (image, canvas, step) = (r.done.image, r.done.canvas, r.done.step);
        ProtectSource::Ready(SubjectFinder { backend: self.sam3.backend(), image, canvas, step, k: 1 })
    }

    /// Whether Protect Subject can ever work here: the model is installed, there is memory for
    /// it and the Object Selection engine preference is not Classic. Quick; for the checkbox.
    pub fn sam3_protect_possible(&mut self) -> bool {
        super::capable(self).is_ok()
    }
}
