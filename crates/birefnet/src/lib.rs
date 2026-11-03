//! BiRefNet (Bilateral Reference for high-resolution dichotomous image segmentation, Zheng et
//! al., CAAI AIR 2024; MIT) in pure Rust on candle: cuts out the main subject of a photo with very
//! fine edges (hair, fur, lace), where a promptable model's masks are coarse.
//!
//! The weights are not part of PhotoCraft: they are the `model.safetensors` of
//! `ZhengPeng7/BiRefNet` on Hugging Face (MIT), read from the folder the Craft apps share
//! (`ArtCraft/models/birefnet/`, the file LightCraft uses too; see `photocraft-ml`'s `Model::Birefnet`)
//! ([`BiRefNet::load`]). On macOS the model runs on the GPU through Metal, elsewhere on the CPU.
//! No UI dependencies. Native only: on wasm32 only the pure helpers (`image`, `combine`) build.
//!
//! Taken from LightCraft's `crates/birefnet` (commit 8807a2bca, without its download code). That is
//! a port of <https://github.com/ZhengPeng7/BiRefNet> (MIT, Copyright 2024 Peng Zheng) and the
//! Swin Transformer it builds on (MIT, Copyright Microsoft); see `NOTICE`.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(not(target_arch = "wasm32"))]
mod blocks;
pub mod combine;
#[cfg(not(target_arch = "wasm32"))]
mod deform;
pub mod image;
#[cfg(not(target_arch = "wasm32"))]
mod model;
#[cfg(not(target_arch = "wasm32"))]
mod params;
#[cfg(not(target_arch = "wasm32"))]
mod swin;
#[cfg(not(target_arch = "wasm32"))]
mod weights;

#[cfg(not(target_arch = "wasm32"))]
mod run;
#[cfg(not(target_arch = "wasm32"))]
pub use run::{BiRefNet, best_device};

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("BiRefNet model: {0}")]
    Model(String),
    #[error("BiRefNet model file not found in {0} (expected model.safetensors)")]
    Missing(PathBuf),
    #[error("BiRefNet cancelled")]
    Cancelled,
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Candle(#[from] candle_core::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The checkpoint's file name in a model directory.
pub const WEIGHTS_FILE: &str = "model.safetensors";

/// Side of the square the photo is stretched to (the general model's input).
pub const INPUT_SIDE: usize = 1024;

/// Whether `dir` holds the model's weights.
pub fn is_model_dir(dir: &std::path::Path) -> bool {
    dir.join(WEIGHTS_FILE).is_file()
}

/// The probability map the model predicts: `side × side` values 0..1 over the square the photo was
/// stretched to.
#[derive(Clone, Debug)]
pub struct Matte {
    pub side: usize,
    /// Logits (before the sigmoid), row-major.
    pub logits: Vec<f32>,
}
