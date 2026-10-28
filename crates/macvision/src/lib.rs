//! macOS's own subject cutout (Apple's Vision framework, the one Photos and Preview use to lift a
//! subject) as a soft mask for Select ▸ Subject.
//!
//! [`subject_mask`] takes straight RGBA8 pixels and returns an 8-bit coverage mask of the same size:
//! every foreground instance Vision finds (a person, a pet, an object), merged into one, with soft
//! edges. On macOS 14 and later it asks `VNGenerateForegroundInstanceMaskRequest`; on macOS 12 and 13
//! it falls back to `VNGeneratePersonSegmentationRequest` (people only). Elsewhere (Windows, Linux,
//! wasm, or a Mac older than 12) everything compiles, [`available`] is false and [`subject_mask`]
//! returns [`Error::Unavailable`].
//!
//! Vision works at a few megapixels, so a bigger picture is shrunk first ([`vision_size`]) and the
//! mask scaled back up to the picture's size ([`Mask::resized`], bilinear). A caller that holds a
//! very large picture should shrink it itself first and scale the mask itself, to avoid holding
//! the full-size RGBA at all.
//!
//! The crate is the second isolated `unsafe` helper of the fork (after `photocraft-tablet`): all
//! Objective-C calls live in the `macos` module (see README.md for how Vision is never given a
//! reason to throw); the API, the scaling and the mask conversion are safe Rust tested everywhere.
//! It has no dependency on a workspace crate.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
mod scale;
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod stub;

#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(target_os = "macos"))]
use stub as imp;

pub use scale::{MAX_INPUT_PIXELS, MAX_VISION_PIXELS, vision_size};

/// 8-bit coverage over a `width × height` picture, row-major: 0 = background, 255 = subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Mask {
    /// This mask scaled to `width × height` with bilinear sampling (soft edges stay soft). `None`
    /// for an empty target or a mask whose data does not match its size.
    pub fn resized(&self, width: u32, height: u32) -> Option<Mask> {
        scale::resize_bilinear(self, width, height)
    }
}

/// Why no mask came back. Every variant has a message a user can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a Mac, or macOS older than 12 (no subject cutout there).
    Unavailable,
    /// Vision ran and found no foreground subject, or the picture is a strip too thin (under 8
    /// pixels on a side) to hold one.
    NoSubject,
    /// The pixel buffer does not match `width × height × 4`, or the picture is empty or past
    /// [`MAX_INPUT_PIXELS`].
    BadInput,
    /// Vision or Core Image reported an error.
    Os(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unavailable => f.write_str("the macOS subject cutout needs a Mac with macOS 12 or later"),
            Error::NoSubject => f.write_str("macOS found no subject in this picture"),
            Error::BadInput => f.write_str("the picture is empty, too large or its pixels do not match its size"),
            Error::Os(msg) => write!(f, "macOS Vision: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// True when this machine has a Vision subject request: a Mac on macOS 12 or later whose Vision
/// has the foreground-instance request (macOS 14) or the person-segmentation request (macOS 12).
pub fn available() -> bool {
    imp::available()
}

/// Cut the subject out of `rgba` (straight RGBA, 8 bits, `width × height` pixels, row-major) as a
/// coverage mask of the same size. Pictures past [`MAX_VISION_PIXELS`] are shrunk for Vision and
/// the mask scaled back up. A picture under 8 pixels on a side (after shrinking) is
/// [`Error::NoSubject`]. Runs for about a second; call it off the UI thread when there is one.
pub fn subject_mask(rgba: &[u8], width: u32, height: u32) -> Result<Mask, Error> {
    let px = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || px > MAX_INPUT_PIXELS || rgba.len() as u64 != px * 4 {
        return Err(Error::BadInput);
    }
    if !available() {
        return Err(Error::Unavailable);
    }
    let (vw, vh) = vision_size(width, height);
    // A strip too thin for Vision (here or once shrunk) has no room for a subject: that is no
    // subject found, not a failure.
    if scale::too_thin(vw, vh) {
        return Err(Error::NoSubject);
    }
    if (vw, vh) == (width, height) {
        return imp::subject_mask(rgba, width, height);
    }
    let small = scale::box_downscale_rgba(rgba, (width, height), (vw, vh)).ok_or(Error::BadInput)?;
    let mask = imp::subject_mask(&small, vw, vh)?;
    mask.resized(width, height).ok_or(Error::BadInput)
}

#[cfg(all(test, target_os = "macos"))]
mod mac_tests;
#[cfg(test)]
mod tests;
