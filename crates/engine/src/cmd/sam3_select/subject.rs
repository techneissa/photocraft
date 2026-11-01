//! Select Subject with SAM 3 (`tools.selectSubjectEngine` = `sam3`): the text prompt "person",
//! then "main subject" when nothing scores, on an encoded image of the picture. The shared
//! encoding of Object Selection is used when it is ready for these pixels; otherwise the job
//! encodes the picture itself (about 6 s warm, 13 to 22 s cold) and gives it back afterwards.
//! Remove Background, Remove Subject and Select and Mask's Subject button follow the same
//! preference through [`find_subject_region`](crate::smartselect_cmds::find_subject_region).

use std::sync::Arc;

use photocraft_algo::segment::Sampler;
use photocraft_algo::selection::Region;
use photocraft_geom::Rect;
use photocraft_ml::{MlError, Rgb};
use photocraft_raster::Interrupt;

use super::backend::{EncodedImage, Sam3Backend};
use super::{Entry, FAILED, NO_MEMORY, NOT_INSTALLED, can_run, mask};
use crate::Session;

/// The concepts asked for, in order; the first that scores is the subject.
pub(crate) const PROMPTS: [&str; 2] = ["person", "main subject"];

/// How sure the model must be of an instance to count it (the reference default).
const THRESHOLD: f32 = 0.5;

/// What a Select Subject job needs from SAM 3, taken on the session's thread.
#[derive(Clone)]
pub(crate) struct SubjectSam3 {
    backend: Arc<dyn Sam3Backend>,
    /// The note for why SAM 3 cannot run here (not installed, not enough memory).
    refusal: Option<&'static str>,
    /// An encoding ready for the active document: image, canvas, step.
    ready: Option<(EncodedImage, Rect, usize)>,
}

impl SubjectSam3 {
    /// Why SAM 3 cannot run, if it cannot.
    pub(crate) fn refusal(&self) -> Option<&'static str> {
        self.refusal
    }

    /// The subject of `smp` over `area`: `Ok(None)` when no prompt scores. Stops with
    /// [`MlError::Cancelled`] once `ctl` says so.
    pub(crate) fn find(&self, smp: &dyn Sampler, area: Rect, ctl: &Interrupt) -> Result<Option<Region>, MlError> {
        let cancelled = || ctl.cancelled();
        let stop = || if cancelled() { Err(MlError::Cancelled) } else { Ok(()) };
        stop()?;
        let (image, canvas, step, own) = match self.ready.filter(|(image, canvas, _)| *canvas == area && self.backend.is_current(*image)) {
            Some((image, canvas, step)) => (image, canvas, step, false),
            None => {
                let step = mask::step_for(area);
                let small = smp.rgb_scaled(area, step);
                stop()?;
                let rgb = Rgb::new(small.w, small.h, small.px.iter().flatten().copied().collect())?;
                drop(small);
                (self.backend.encode(&rgb, &cancelled)?, area, step, true)
            }
        };
        let found = self.ask(image, canvas, step, &stop);
        if own {
            // Our own encoding: give the memory back (the shared one stays for Object Selection).
            self.backend.release(image);
        }
        found
    }

    fn ask(&self, image: EncodedImage, canvas: Rect, step: usize, stop: &dyn Fn() -> Result<(), MlError>) -> Result<Option<Region>, MlError> {
        for prompt in PROMPTS {
            stop()?;
            let Some(m) = self.backend.text(image, prompt, THRESHOLD)? else { continue };
            stop()?;
            if let Some(region) = mask::to_region(&m, canvas, step).map_err(|e| MlError::BadInput(e.to_string()))? {
                return Ok(Some(region));
            }
        }
        Ok(None)
    }
}

/// The status note for a SAM 3 request that failed with `e` (the choice was explicit).
pub(crate) fn failure_note(e: &MlError) -> &'static str {
    match e {
        MlError::LowMemory { .. } => NO_MEMORY,
        MlError::ModelMissing { .. } => NOT_INSTALLED,
        _ => FAILED,
    }
}

impl Session {
    /// What a Select Subject job needs for SAM 3 on the active document (`all_layers`: which pixels
    /// are sampled): the backend, whether it can run, and the shared encoding when it is ready.
    pub(crate) fn sam3_subject(&mut self, all_layers: bool) -> SubjectSam3 {
        let backend = self.sam3.backend();
        let refusal = can_run(self).err();
        let mut ready = None;
        if refusal.is_none()
            && let Some(st) = self.active()
        {
            let (doc, target) = (Arc::clone(&st.doc), super::sampled_target(&st.doc, st.active_layer, all_layers));
            self.sam3.settle(&doc, target);
            if let Entry::Ready(r) = &self.sam3.entry {
                ready = Some((r.done.image, r.done.canvas, r.done.step));
            }
        }
        SubjectSam3 { backend, refusal, ready }
    }
}
