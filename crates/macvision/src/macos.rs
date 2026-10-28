//! The Vision backend: every Objective-C call of the crate, so all its `unsafe`.
//!
//! Rules: one autorelease pool per cutout, and every Objective-C object a cutout touches is created
//! and dropped inside it, so a worker thread that lives for the whole process leaks nothing. The
//! request, the handler and the image are made per call and never shared. No Rust callback is handed
//! to Vision (no completion block), so a Rust panic cannot start inside Apple's frames.
//!
//! Objective-C exceptions are **not caught**: catching them needs `objc2`'s `exception` feature,
//! which compiles an Objective-C helper with `cc`, and PhotoCraft is pure Rust. Vision is never
//! given a reason to throw instead: the request classes are looked up and the OS version checked
//! before anything is created; the picture is non-empty, at least [`MIN_SIDE`] pixels on a side, at
//! most [`crate::MAX_VISION_PIXELS`] and exactly `width × height × 4` bytes before a `CIImage` is
//! made; every object a call may return nil for (the image, the handler, the requests, the instance
//! set, the people mask) is received as an `Option` and nil becomes an error, never a `Retained`; the
//! handler is given the orientation explicitly (up); and the mask buffer's format, size and byte
//! count are checked before a pixel is read, with the buffer locked by a guard that unlocks on every
//! path. README.md lists them.

use crate::scale::{Layout, MIN_SIDE, read_mask};
use crate::{Error, Mask};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{AnyThread, ClassType, msg_send};
use objc2_core_foundation::CGSize;
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_core_image::{CIImage, kCIFormatRGBA8};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_OneComponent8,
    kCVPixelFormatType_OneComponent32Float, kCVReturnSuccess,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSIndexSet, NSOperatingSystemVersion, NSProcessInfo, NSString};
use objc2_vision::{VNGenerateForegroundInstanceMaskRequest, VNGeneratePersonSegmentationRequest, VNImageRequestHandler, VNPixelBufferObservation, VNRequest};
use std::sync::OnceLock;

/// `kCGImagePropertyOrientationUp` (CGImagePropertyOrientation is a `uint32_t`): the picture is
/// upright as given, row 0 on top, which is also how the mask buffers Vision returns are laid out.
const ORIENTATION_UP: u32 = 1;

/// Which request this Mac can run.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Backend {
    /// `VNGenerateForegroundInstanceMaskRequest` (macOS 14): every foreground instance.
    Foreground,
    /// `VNGeneratePersonSegmentationRequest` (macOS 12): people only.
    People,
}

fn at_least(major: isize) -> bool {
    let v = NSOperatingSystemVersion { majorVersion: major, minorVersion: 0, patchVersion: 0 };
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(v)
}

fn backend() -> Option<Backend> {
    static BACKEND: OnceLock<Option<Backend>> = OnceLock::new();
    *BACKEND.get_or_init(|| {
        autoreleasepool(|_| {
            if at_least(14) && AnyClass::get(c"VNGenerateForegroundInstanceMaskRequest").is_some() && AnyClass::get(c"VNInstanceMaskObservation").is_some() {
                Some(Backend::Foreground)
            } else if at_least(12) && AnyClass::get(c"VNGeneratePersonSegmentationRequest").is_some() && AnyClass::get(c"VNPixelBufferObservation").is_some() {
                Some(Backend::People)
            } else {
                None
            }
        })
    })
}

pub(crate) fn available() -> bool {
    backend().is_some()
}

pub(crate) fn subject_mask(rgba: &[u8], width: u32, height: u32) -> Result<Mask, Error> {
    let backend = backend().ok_or(Error::Unavailable)?;
    let px = u64::from(width) * u64::from(height);
    if width < MIN_SIDE || height < MIN_SIDE || px > crate::MAX_VISION_PIXELS || rgba.len() as u64 != px * 4 {
        return Err(Error::BadInput);
    }
    autoreleasepool(|_| {
        let image = image(rgba, width, height)?;
        let options = NSDictionary::<NSString, AnyObject>::new();
        // SAFETY: `initWithCIImage:orientation:options:` on a freshly allocated handler of a class
        // `backend` checked exists, with a valid image, the orientation as a `uint32_t` (the
        // `CGImagePropertyOrientation` encoding, "up") and an empty options dictionary of the declared
        // key and value types. The result is received as an `Option`: an init may return nil, and
        // nil must never be wrapped in a `Retained`.
        let handler: Option<Retained<VNImageRequestHandler>> =
            unsafe { msg_send![VNImageRequestHandler::alloc(), initWithCIImage: &*image, orientation: ORIENTATION_UP, options: &*options] };
        let handler = handler.ok_or_else(|| Error::Os("Vision gave no request handler".into()))?;
        let mask = match backend {
            Backend::Foreground => foreground(&handler)?,
            Backend::People => people(&handler)?,
        };
        // The foreground mask already has the picture's size; the people mask has its quality level's.
        if (mask.width, mask.height) == (width, height) { Ok(mask) } else { mask.resized(width, height).ok_or(Error::BadInput) }
    })
}

/// The picture as a `CIImage` (sRGB, 8-bit RGBA). Core Image copies the bytes, so `rgba` need not
/// outlive it.
fn image(rgba: &[u8], width: u32, height: u32) -> Result<Retained<CIImage>, Error> {
    let data = NSData::with_bytes(rgba);
    // SAFETY: reading an immutable `CFString` constant exported by CoreGraphics.
    let name = unsafe { kCGColorSpaceSRGB };
    let space = CGColorSpace::with_name(Some(name)).ok_or_else(|| Error::Os("no sRGB colour space".into()))?;
    // SAFETY: reading an immutable format constant exported by Core Image.
    let format = unsafe { kCIFormatRGBA8 };
    // SAFETY: `data` holds exactly `width × height × 4` bytes (checked by the caller), which is
    // `bytes_per_row × height` for `bytes_per_row = width × 4`; the format is 8-bit RGBA and the
    // colour space a valid one. Core Image returns nil for data it cannot use, so the result is
    // received as an `Option` and nil is never wrapped in a `Retained`.
    let image: Option<Retained<CIImage>> = unsafe {
        msg_send![
            CIImage::class(),
            imageWithBitmapData: &*data,
            bytesPerRow: width as usize * 4,
            size: CGSize::new(f64::from(width), f64::from(height)),
            format: format,
            colorSpace: Some(&*space)
        ]
    };
    image.ok_or_else(|| Error::Os("Core Image could not make the picture".into()))
}

fn describe(e: &NSError) -> String {
    e.localizedDescription().to_string()
}

/// Run `request` on `handler`; Vision fills `request.results`.
fn perform(handler: &VNImageRequestHandler, request: &VNRequest) -> Result<(), Error> {
    handler.performRequests_error(&NSArray::from_slice(&[request])).map_err(|e| Error::Os(describe(&e)))
}

fn foreground(handler: &VNImageRequestHandler) -> Result<Mask, Error> {
    // SAFETY: `new` on a request class `backend` checked exists; received as an `Option` so a nil
    // from `init` is an error, not a `Retained`.
    let request: Option<Retained<VNGenerateForegroundInstanceMaskRequest>> = unsafe { msg_send![VNGenerateForegroundInstanceMaskRequest::class(), new] };
    let request = request.ok_or_else(|| Error::Os("Vision gave no request".into()))?;
    perform(handler, &request)?;
    // SAFETY: a getter on a request that has run; read as an `Option` (nil without results).
    let results = unsafe { request.results() }.ok_or(Error::NoSubject)?;
    let observation = results.firstObject().ok_or(Error::NoSubject)?;
    // SAFETY: a getter on an observation we hold (the set of every non-background instance); received
    // as an `Option` so a nil is an error, not a `Retained`.
    let all: Option<Retained<NSIndexSet>> = unsafe { msg_send![&*observation, allInstances] };
    let all = all.ok_or(Error::NoSubject)?;
    if all.count() == 0 {
        return Err(Error::NoSubject);
    }
    // SAFETY: the instances are the observation's own, the handler is the one that produced it; the
    // result is the soft mask of all of them merged, at the picture's size, returned retained. The
    // binding's `Result` form turns a nil return into an `Err` (objc2 makes an error when Vision
    // sets none), so nil is never wrapped in a `Retained`.
    let buffer = unsafe { observation.generateScaledMaskForImageForInstances_fromRequestHandler_error(&all, handler) }.map_err(|e| Error::Os(describe(&e)))?;
    read_buffer(&buffer)
}

fn people(handler: &VNImageRequestHandler) -> Result<Mask, Error> {
    // SAFETY: `new` on a request class `backend` checked exists; received as an `Option` so a nil
    // from `init` is an error, not a `Retained`.
    let request: Option<Retained<VNGeneratePersonSegmentationRequest>> = unsafe { msg_send![VNGeneratePersonSegmentationRequest::class(), new] };
    let request = request.ok_or_else(|| Error::Os("Vision gave no request".into()))?;
    perform(handler, &request)?;
    // SAFETY: a getter on a request that has run; read as an `Option` (nil without results).
    let results: Retained<NSArray<VNPixelBufferObservation>> = unsafe { request.results() }.ok_or(Error::NoSubject)?;
    let observation = results.firstObject().ok_or(Error::NoSubject)?;
    // SAFETY: a getter on an observation we hold; the buffer is retained by it. Received as an
    // `Option` so a nil is an error, not a `Retained`.
    let buffer: Option<Retained<CVPixelBuffer>> = unsafe { msg_send![&*observation, pixelBuffer] };
    let buffer = buffer.ok_or(Error::NoSubject)?;
    read_buffer(&buffer)
}

/// A read-only lock on a pixel buffer's base address, released when it is dropped, so every path
/// out (an early return, a panic, an allocation failure) unlocks.
struct ReadLock<'a>(&'a CVPixelBuffer);

impl<'a> ReadLock<'a> {
    fn new(buffer: &'a CVPixelBuffer) -> Result<Self, Error> {
        // SAFETY: a lock call on a live pixel buffer; read-only, so the buffer is not changed. The
        // guard made on success unlocks it with the same flags.
        if unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) } == kCVReturnSuccess {
            Ok(Self(buffer))
        } else {
            Err(Error::Os("could not lock the mask".into()))
        }
    }
}

impl Drop for ReadLock<'_> {
    fn drop(&mut self) {
        // SAFETY: unlocks the lock taken in `new`, with the same flags; the guard outlives every slice
        // made from the buffer (it is declared before them, so it drops after them).
        unsafe { CVPixelBufferUnlockBaseAddress(self.0, CVPixelBufferLockFlags::ReadOnly) };
    }
}

/// A one-component mask buffer as a [`Mask`]: 32-bit float or 8-bit, rows may be padded.
fn read_buffer(buffer: &CVPixelBuffer) -> Result<Mask, Error> {
    let layout = match CVPixelBufferGetPixelFormatType(buffer) {
        f if f == kCVPixelFormatType_OneComponent32Float => Layout::Float32,
        f if f == kCVPixelFormatType_OneComponent8 => Layout::Byte,
        other => return Err(Error::Os(format!("unexpected mask format {other:#x}"))),
    };
    let (w, h, row) = (CVPixelBufferGetWidth(buffer), CVPixelBufferGetHeight(buffer), CVPixelBufferGetBytesPerRow(buffer));
    let (width, height) = (u32::try_from(w).map_err(|_| Error::BadInput)?, u32::try_from(h).map_err(|_| Error::BadInput)?);
    // Vision's masks are at most the size it was given (at most MAX_VISION_PIXELS): refuse anything
    // else before a slice of that size is built.
    let px = u64::from(width) * u64::from(height);
    if px == 0 || px > crate::MAX_VISION_PIXELS {
        return Err(Error::Os(format!("unexpected mask size {width}×{height}")));
    }
    let len = row.checked_mul(h).ok_or(Error::BadInput)?;
    let _lock = ReadLock::new(buffer)?;
    let base = CVPixelBufferGetBaseAddress(buffer);
    if base.is_null() {
        return Err(Error::Os("the mask buffer has no pixels".into()));
    }
    // SAFETY: the buffer is locked read-only by `_lock` (alive to the end of this function), so its
    // base address is valid for `bytes_per_row × height` bytes of this non-planar one-component
    // buffer, `len` is exactly that (checked above, and at most a few MB by the size check), and
    // nothing writes to it while the slice lives; the slice is dropped before the guard.
    let bytes = unsafe { std::slice::from_raw_parts(base.cast::<u8>(), len) };
    read_mask(bytes, row, width, height, layout).ok_or_else(|| Error::Os("the mask buffer did not match its size".into()))
}
