//! One run from the file: load `big-lama.pt`, predict, let the model go.

use std::path::Path;

use candle_core::Device;

use crate::{Error, Lama, Result, best_device};

/// Set to a non-empty value other than `0` to keep LaMa on the CPU (the GPU is the default on a Mac).
pub const CPU_ONLY_VAR: &str = "PHOTOCRAFT_LAMA_CPU";

/// Loads the model at `path`, runs [`Lama::predict`] on it and drops it again, so the model's memory is
/// free between runs. Runs on the GPU when there is one; a failure of the tensor library there (a missing
/// GPU operation, no memory) is tried once more on the CPU. Bad input, a bad model file and a cancel are
/// not retried.
pub fn predict_from_file(path: &Path, rgb: &[f32], hole: &[f32], w: usize, h: usize, cancel: &dyn Fn() -> bool) -> Result<Vec<f32>> {
    let cpu_only = std::env::var_os(CPU_ONLY_VAR).is_some_and(|v| !v.is_empty() && v != "0");
    let device = if cpu_only { Device::Cpu } else { best_device() };
    match run_on(&device, path, rgb, hole, w, h, cancel) {
        Err(Error::Compute(why)) if !device.is_cpu() => {
            log::warn!("LaMa on the GPU failed ({why}); trying the CPU");
            run_on(&Device::Cpu, path, rgb, hole, w, h, cancel)
        }
        other => other,
    }
}

fn run_on(device: &Device, path: &Path, rgb: &[f32], hole: &[f32], w: usize, h: usize, cancel: &dyn Fn() -> bool) -> Result<Vec<f32>> {
    if cancel() {
        return Err(Error::Cancelled);
    }
    let model = Lama::load(path, device)?;
    if cancel() {
        return Err(Error::Cancelled);
    }
    model.predict(rgb, hole, w, h, cancel)
}
