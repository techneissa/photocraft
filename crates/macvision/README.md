# photocraft-macvision

macOS's own subject cutout (Apple's Vision framework, the one Photos and Preview use to lift a
subject) as a soft mask. Layer 0, no workspace dependencies; a stub that reports itself unavailable
on every other system. All `unsafe` is in `src/macos.rs` (the fork's second `unsafe` crate, after
`photocraft-tablet`).

```rust
let mask = photocraft_macvision::subject_mask(&rgba8, width, height)?; // Mask { width, height, data: Vec<u8> }
```

- macOS 14+: `VNGenerateForegroundInstanceMaskRequest`, every foreground instance merged into one soft
  mask at the picture's size (`generateScaledMaskForImageForInstances`).
- macOS 12–13: `VNGeneratePersonSegmentationRequest`, people only, scaled up to the picture's size.
- Anything else: `Error::Unavailable` (`available()` is false).
- Pictures past 4 MP are shrunk for Vision and the mask scaled back up (bilinear); a caller with a
  huge picture should do both itself and pass the small one.

**Objective-C exceptions are not caught.** Catching them needs `objc2`'s `exception` feature, which
compiles an Objective-C helper with `cc`, and PhotoCraft is pure Rust. Instead Vision is never given a
reason to throw:

- the request classes (`VNGenerateForegroundInstanceMaskRequest`, `VNInstanceMaskObservation`,
  `VNGeneratePersonSegmentationRequest`, `VNPixelBufferObservation`) are looked up with
  `AnyClass::get` and the OS version checked before any object is made;
- the picture is between 8 px and 4 MP (each side) and exactly `width × height × 4` bytes before a
  `CIImage` is made (Core Image copies the bytes);
- no completion block is passed to Vision, so no Rust code runs inside Apple's frames;
- every object Vision may return nil for is read as an `Option` (`results`, `firstObject`);
- the mask buffer's pixel format (32-bit float or 8-bit), size and byte count are checked before a
  pixel is read, and it is locked read-only for the read.

One autorelease pool per cutout; the request, handler and image are made per call and never shared.

Tests: the pure parts (sizes, shrinking, bilinear scaling, reading buffers) run everywhere. The Mac
tests run real Vision on a synthetic figure by default and on a real portrait when
`PHOTOCRAFT_MACVISION_IMAGE` names a JPEG or PNG (`PHOTOCRAFT_MACVISION_OUT` = a folder also gets
`mask.png`).
