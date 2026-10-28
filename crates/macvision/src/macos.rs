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
//! before anything is created; the picture is non-empty, between [`MIN_SIDE`] pixels and
//! [`crate::MAX_VISION_PIXELS`] and exactly `width × height × 4` bytes before a `CIImage` is made;
//! every object a call may return nil for is read as an `Option`; and the mask buffer's format,
//! size and byte count are checked before a pixel is read. README.md lists them.

use crate::scale::{Layout, read_mask};
use crate::{Error, Mask};
use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject};
use objc2_core_foundation::CGSize;
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_core_image::{CIImage, kCIFormatRGBA8};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_OneComponent8,
    kCVPixelFormatType_OneComponent32Float, kCVReturnSuccess,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSOperatingSystemVersion, NSProcessInfo, NSString};
use objc2_vision::{VNGenerateForegroundInstanceMaskRequest, VNGeneratePersonSegmentationRequest, VNImageRequestHandler, VNPixelBufferObservation, VNRequest};
use std::sync::OnceLock;

/// Vision is given nothing smaller than this on either side.
const MIN_SIDE: u32 = 8;

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
        // SAFETY: a plain `init` of a class whose existence `backend` checked, with an empty options
        // dictionary (generic types `NSString` keys and `AnyObject` values, as the header declares).
        let handler = unsafe { VNImageRequestHandler::initWithCIImage_options(VNImageRequestHandler::alloc(), &image, &NSDictionary::<NSString, AnyObject>::new()) };
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
    let data = objc2_foundation::NSData::with_bytes(rgba);
    // SAFETY: reading an immutable `CFString` constant exported by CoreGraphics.
    let name = unsafe { kCGColorSpaceSRGB };
    let space = CGColorSpace::with_name(Some(name)).ok_or_else(|| Error::Os("no sRGB colour space".into()))?;
    // SAFETY: reading an immutable format constant exported by Core Image.
    let format = unsafe { kCIFormatRGBA8 };
    // SAFETY: `data` holds exactly `width × height × 4` bytes (checked by the caller), which is
    // `bytes_per_row × height` for `bytes_per_row = width × 4`; the format is 8-bit RGBA and the
    // colour space a valid one; the binding returns the image retained.
    Ok(unsafe {
        CIImage::imageWithBitmapData_bytesPerRow_size_format_colorSpace(
            &data,
            width as usize * 4,
            CGSize::new(f64::from(width), f64::from(height)),
            format,
            Some(&space),
        )
    })
}

fn describe(e: &NSError) -> String {
    e.localizedDescription().to_string()
}

/// Run `request` on `handler`; Vision fills `request.results`.
fn perform(handler: &VNImageRequestHandler, request: &VNRequest) -> Result<(), Error> {
    handler.performRequests_error(&NSArray::from_slice(&[request])).map_err(|e| Error::Os(describe(&e)))
}

fn foreground(handler: &VNImageRequestHandler) -> Result<Mask, Error> {
    // SAFETY: `new` on a request class `backend` checked exists.
    let request = unsafe { VNGenerateForegroundInstanceMaskRequest::new() };
    perform(handler, &request)?;
    // SAFETY: a getter on a request that has run; read as an `Option` (nil without results).
    let results = unsafe { request.results() }.ok_or(Error::NoSubject)?;
    let observation = results.firstObject().ok_or(Error::NoSubject)?;
    // SAFETY: a getter on an observation we hold; the set of every non-background instance.
    let all = unsafe { observation.allInstances() };
    if all.count() == 0 {
        return Err(Error::NoSubject);
    }
    // SAFETY: the instances are the observation's own, the handler is the one that produced it; the
    // result is the soft mask of all of them merged, at the picture's size, returned retained.
    let buffer = unsafe { observation.generateScaledMaskForImageForInstances_fromRequestHandler_error(&all, handler) }.map_err(|e| Error::Os(describe(&e)))?;
    read_buffer(&buffer)
}

fn people(handler: &VNImageRequestHandler) -> Result<Mask, Error> {
    // SAFETY: `new` on a request class `backend` checked exists.
    let request = unsafe { VNGeneratePersonSegmentationRequest::new() };
    perform(handler, &request)?;
    // SAFETY: a getter on a request that has run; read as an `Option` (nil without results).
    let results: Retained<NSArray<VNPixelBufferObservation>> = unsafe { request.results() }.ok_or(Error::NoSubject)?;
    let observation = results.firstObject().ok_or(Error::NoSubject)?;
    // SAFETY: a getter on an observation we hold; the buffer is retained by it.
    let buffer = unsafe { observation.pixelBuffer() };
    read_buffer(&buffer)
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
    let len = row.checked_mul(h).ok_or(Error::BadInput)?;
    // SAFETY: a lock call on a live pixel buffer; read-only, so the buffer is not changed. It is
    // unlocked below on every path past this one.
    if unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) } != kCVReturnSuccess {
        return Err(Error::Os("could not lock the mask".into()));
    }
    let base = CVPixelBufferGetBaseAddress(buffer);
    let mask = if base.is_null() {
        None
    } else {
        // SAFETY: the buffer is locked read-only, so its base address is valid for `bytes_per_row ×
        // height` bytes of this non-planar one-component buffer and nothing writes to it until the
        // unlock below; the slice is dropped (not used past `read_mask`) before that.
        let bytes = unsafe { std::slice::from_raw_parts(base.cast::<u8>(), len) };
        read_mask(bytes, row, width, height, layout)
    };
    // SAFETY: unlocks the lock taken above, with the same flags; no slice of the buffer is alive.
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    mask.ok_or_else(|| Error::Os("the mask buffer did not match its size".into()))
}
