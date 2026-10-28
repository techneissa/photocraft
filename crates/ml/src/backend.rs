//! The tract plumbing: load a model for a run, run it on flat `f32` tensors, forget it again.

use std::sync::{Arc, Mutex, PoisonError};

use tract_onnx::prelude::*;

use crate::store::{Model, expected_path, model_path};
use crate::{Cancel, MlError, check_cancel};

type Plan = Arc<TypedRunnableModel>;

static PLANS: Mutex<Vec<(Model, Plan)>> = Mutex::new(Vec::new());
/// One inference at a time: a cancelled job's worker may still be inside tract (which can't be
/// interrupted), so a retry waits for it instead of stacking a second full-size run in memory.
static RUNNING: Mutex<()> = Mutex::new(());

fn backend<E: std::fmt::Display>(e: E) -> MlError {
    MlError::Backend(e.to_string())
}

/// Forgets `model`'s plan when dropped, whatever ends the run: success, a tract or tensor error,
/// Esc after the load, or a failed load. Kept, a plan holds about 6.7 GB of the process (measured
/// with BiRefNet-lite on a 24 MP photo: 2.2 GB resident without it), for a repeat that is only 4 s
/// faster; a repeat on unchanged pixels answers from Select Subject's own cache anyway.
struct ForgetPlan(Model);

impl Drop for ForgetPlan {
    fn drop(&mut self) {
        PLANS.lock().unwrap_or_else(PoisonError::into_inner).retain(|(m, _)| *m != self.0);
    }
}

/// The optimised plan of `model`, built from the installed file (a few seconds, about 9 GB at the
/// peak). `PLANS` is locked only to look and to store, never across the load.
fn load(model: Model) -> Result<Plan, MlError> {
    let Some(path) = model_path(model) else {
        PLANS.lock().unwrap_or_else(PoisonError::into_inner).retain(|(m, _)| *m != model);
        return Err(MlError::ModelMissing { model, path: expected_path(model) });
    };
    // Load from the bytes whose hash was just checked, so what runs is what was verified.
    let bytes = verified_bytes(&path, model.size_bytes(), model.sha256())?;
    tract_onnx::onnx().model_for_read(&mut bytes.as_slice()).and_then(|m| m.into_optimized()).and_then(|m| m.into_runnable()).map_err(backend)
}

/// Runs `run` on `model`'s plan, one inference at a time (a cancelled job's worker may still be in
/// tract, which can't be interrupted, so the next one waits). In order: wait for the turn, stop if
/// cancelled, refuse a computer without the memory (`total_ram`), load the plan with `load`, stop if
/// cancelled during the load, run. The plan is forgotten afterwards on every path ([`ForgetPlan`]).
fn run_plan<T>(
    model: Model,
    total_ram: Option<u64>,
    load: &dyn Fn() -> Result<Plan, MlError>,
    cancel: Cancel<'_>,
    run: impl FnOnce(&Plan) -> Result<T, MlError>,
) -> Result<T, MlError> {
    let _one = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    let _forget = ForgetPlan(model);
    check_cancel(cancel)?;
    let cached = PLANS.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(m, _)| *m == model).map(|(_, p)| Arc::clone(p));
    let plan = match cached {
        Some(plan) => plan,
        None => {
            crate::ram::check_total(model, total_ram)?;
            let plan = load()?;
            PLANS.lock().unwrap_or_else(PoisonError::into_inner).push((model, Arc::clone(&plan)));
            plan
        }
    };
    check_cancel(cancel)?;
    run(&plan)
}

/// The contents of `path`, if they have `size` bytes and the SHA-256 `sha256`.
fn verified_bytes(path: &std::path::Path, size: u64, sha256: &str) -> Result<Vec<u8>, MlError> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| MlError::Backend(format!("read {}: {e}", path.display())))?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if bytes.len() as u64 != size || actual != sha256 {
        return Err(MlError::Backend(format!("{} does not match the published model; download it again", path.display())));
    }
    Ok(bytes)
}

/// Runs `model` on `inputs` (shape, row-major data) and returns its first output's data. Stops with [`MlError::Cancelled`] after waiting for
/// another inference to finish and after loading the model. tract can't be interrupted mid-run, so that is the last check.
pub(crate) fn run_with(model: Model, inputs: &[(&[usize], Vec<f32>)], cancel: Cancel<'_>) -> Result<Vec<f32>, MlError> {
    run_plan(model, crate::ram::physical_memory(), &|| load(model), cancel, |plan| {
        let mut feed: TVec<TValue> = TVec::new();
        for (shape, data) in inputs {
            feed.push(Tensor::from_shape(shape, data).map_err(backend)?.into());
        }
        let out = plan.run(feed).map_err(backend)?;
        let first = out.first().ok_or_else(|| MlError::Backend("the model returned no output".into()))?;
        let view = first.to_plain_array_view::<f32>().map_err(backend)?;
        Ok(view.iter().copied().collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bytes_with_the_published_hash_are_loaded() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("photocraft-ml-backend-{}-{:?}.onnx", std::process::id(), std::thread::current().id()));
        std::fs::File::create_new(&path)?;
        let wrong = verified_bytes(&path, 3, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        // Same size, different bytes.
        std::fs::write(&path, b"abd")?;
        let same_size = verified_bytes(&path, 3, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        std::fs::write(&path, b"abc")?;
        let right = verified_bytes(&path, 3, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let _ = std::fs::remove_file(&path);
        assert!(wrong.is_err(), "an empty file must not load");
        assert!(same_size.is_err(), "a same-size corrupt file must not load");
        assert_eq!(right?, b"abc");
        Ok(())
    }

    /// The tests share `PLANS` and `RUNNING`, so they take turns.
    static TURN: Mutex<()> = Mutex::new(());

    /// A one-input identity plan, small enough to build in a test.
    fn tiny_plan() -> Result<Plan, MlError> {
        let mut m = TypedModel::default();
        let x = m.add_source("x", f32::fact([2])).map_err(backend)?;
        m.select_output_outlets(&[x]).map_err(backend)?;
        m.into_runnable().map_err(backend)
    }

    fn held() -> usize {
        PLANS.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    #[test]
    fn a_finished_run_a_failed_run_and_a_cancelled_run_all_forget_the_plan() {
        let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);
        let model = Model::Lama;
        // A run that sees the plan held, and ends well.
        let ok = run_plan(model, None, &tiny_plan, &|| false, |_| {
            assert_eq!(held(), 1, "the plan is held while the run uses it");
            Ok(7)
        });
        assert_eq!(ok, Ok(7));
        assert_eq!(held(), 0, "success");
        // A tract or tensor error inside the run.
        let failed = run_plan::<()>(model, None, &tiny_plan, &|| false, |_| Err(MlError::Backend("tensor".into())));
        assert_eq!(failed, Err(MlError::Backend("tensor".into())));
        assert_eq!(held(), 0, "error in the run");
        // The plan built, then Esc: stops before the run.
        let built = std::cell::Cell::new(false);
        let cancelled = run_plan::<()>(
            model,
            None,
            &|| {
                built.set(true);
                tiny_plan()
            },
            &|| built.get(),
            |_| Err(MlError::Backend("the run must not start".into())),
        );
        assert_eq!(cancelled, Err(MlError::Cancelled));
        assert_eq!(held(), 0, "cancel after the load");
        // A load that fails.
        let no_load = run_plan::<()>(model, None, &|| Err(MlError::Backend("load".into())), &|| false, |_| Ok(()));
        assert_eq!(no_load, Err(MlError::Backend("load".into())));
        assert_eq!(held(), 0, "failed load");
    }

    #[test]
    fn a_cancel_before_the_load_never_builds_the_plan() {
        let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);
        let r = run_plan::<()>(Model::Lama, None, &|| Err(MlError::Backend("must not load".into())), &|| true, |_| Ok(()));
        assert_eq!(r, Err(MlError::Cancelled));
        assert_eq!(held(), 0);
    }

    #[test]
    fn too_little_memory_refuses_before_the_load() {
        let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);
        let r = run_plan::<()>(Model::BirefnetLite, Some(8 << 30), &|| Err(MlError::Backend("must not load".into())), &|| false, |_| Ok(()));
        assert_eq!(r, Err(MlError::LowMemory { have: 8 << 30, needs: 12 << 30 }));
        assert_eq!(held(), 0);
        // 16 GB passes the check (the load then fails here, as no model is built).
        let r = run_plan::<()>(Model::BirefnetLite, Some(16 << 30), &|| Err(MlError::Backend("load".into())), &|| false, |_| Ok(()));
        assert_eq!(r, Err(MlError::Backend("load".into())));
    }

    #[test]
    fn the_lock_is_free_while_the_plan_loads() {
        let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);
        let r = run_plan(
            Model::Lama,
            None,
            &|| {
                // Preferences or a second caller can look at the plans during the multi-second load.
                assert!(PLANS.try_lock().is_ok());
                tiny_plan()
            },
            &|| false,
            |_| Ok(()),
        );
        assert_eq!(r, Ok(()));
    }
}
