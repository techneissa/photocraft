//! Select Subject's full-size model step: the full BiRefNet (Swin-L, `photocraft_ml::Model::Birefnet`, the
//! `model.safetensors` LightCraft shares) in front of BiRefNet-lite.
//!
//! The order [`super::smartselect_cmds::find_subject_region`] follows for the AI choices:
//!
//! - `auto`: full BiRefNet when installed → the Mac's cutout → BiRefNet-lite when installed → Classic;
//! - `model`: full BiRefNet when installed → BiRefNet-lite when installed → Classic.
//!
//! A full model that cannot run (it fails to load, runs out of memory, the computer has too little) never
//! stops Select Subject: the next finder takes over, with a note. Cancel (Esc) and a mask beyond the output
//! budget still end the command, as for BiRefNet-lite.

use photocraft_algo::selection::Region;
use photocraft_ml::MlError;

use crate::prefs::SubjectEngine;
use crate::{EngineError, Result};

/// The note for a full model that failed while a later engine found the subject.
pub const FULL_FAILED: &str = "Full AI model failed, used another engine";
/// The note for a computer without the memory the full model needs, while a later engine found the subject.
pub const FULL_NO_MEMORY: &str = "Not enough memory for the full AI model, used another engine";

/// The network that draws the full model's matte (`photocraft_ml::segment_subject_full_with`).
pub(super) type Segment = fn(&photocraft_ml::Rgb, photocraft_ml::Cancel<'_>) -> std::result::Result<Vec<f32>, MlError>;

/// Whether the full model is installed (the file there with its published size and SHA-256). Never in unit
/// tests unless one installs a stand-in ([`TEST_FULL`]): they run without models, even where a developer has
/// one installed, so results match CI.
#[cfg(not(test))]
pub(super) fn installed() -> bool {
    !cfg!(target_arch = "wasm32") && photocraft_ml::model_path(photocraft_ml::Model::Birefnet).is_some()
}

/// The full model's memory floor on this computer.
#[cfg(not(test))]
pub(super) fn check_memory() -> std::result::Result<(), MlError> {
    photocraft_ml::check_memory(photocraft_ml::Model::Birefnet)
}

#[cfg(not(test))]
pub(super) fn segment() -> Segment {
    photocraft_ml::segment_subject_full_with
}

// The stand-in a test installs for the full model: its matte function, which also says it is "installed".
#[cfg(test)]
thread_local! {
    pub(super) static TEST_FULL: std::cell::Cell<Option<Segment>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn installed() -> bool {
    TEST_FULL.with(std::cell::Cell::get).is_some()
}

/// A test's stand-in model is never refused for the memory of the machine running the tests.
#[cfg(test)]
pub(super) fn check_memory() -> std::result::Result<(), MlError> {
    Ok(())
}

#[cfg(test)]
pub(super) fn segment() -> Segment {
    TEST_FULL.with(std::cell::Cell::get).unwrap_or(photocraft_ml::segment_subject_full_with)
}

/// Whether `engine` tries the full model first, given whether it is `installed`.
pub(super) fn tried_first(engine: SubjectEngine, installed: bool) -> bool {
    installed && matches!(engine, SubjectEngine::Auto | SubjectEngine::Model)
}

/// What the full model's step came to.
#[derive(Debug, PartialEq)]
pub(super) enum Step {
    /// The full model answered: the subject, or `None` for "no subject" when `model` was chosen explicitly.
    Done(Option<Region>),
    /// The next finder takes over. `classic` is the note if it ends in Classic (the existing model notes), `other`
    /// the note if another engine finds the subject.
    Next { classic: Option<&'static str>, other: Option<&'static str> },
}

/// Maps the full model's `tried` result to the chain's next move. `Cancelled` and `BadInput` end the command;
/// any other error (load, memory, a failed run) is a fall-through.
pub(super) fn step(engine: SubjectEngine, tried: std::result::Result<Option<Region>, MlError>) -> Result<Step> {
    match tried {
        Ok(Some(region)) => Ok(Step::Done(Some(region))),
        // An explicit choice takes "no subject" as the answer; under `auto` the next finder may still find one.
        Ok(None) if engine == SubjectEngine::Model => Ok(Step::Done(None)),
        Ok(None) => Ok(Step::Next { classic: None, other: None }),
        Err(MlError::Cancelled) => Err(EngineError::Cancelled),
        // A mask beyond the output budget (or a failed allocation) must not fall through to another finder that may
        // allocate the same full-resolution area.
        Err(MlError::BadInput(message)) => Err(EngineError::Other(message)),
        Err(MlError::LowMemory { .. }) => Ok(Step::Next { classic: Some(super::smartselect_cmds::MODEL_NO_MEMORY), other: Some(FULL_NO_MEMORY) }),
        Err(_) => Ok(Step::Next { classic: Some(super::smartselect_cmds::MODEL_FAILED), other: Some(FULL_FAILED) }),
    }
}

#[cfg(test)]
mod tests;
