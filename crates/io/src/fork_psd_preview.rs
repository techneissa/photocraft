//! Fork: a preview of a PSD or PSB for the Home grid that never imports its layers. A big layered
//! file opens only with gigabytes of pixel memory, but a picture to look at is already in it: the
//! thumbnail resource (1036), found by reading just the header and the image resources at the start
//! of the file, and the merged composite. Neither is the layer import, so a 1 GB file gets its
//! preview in moments. `None` means the file has neither (or only one this does not read); the
//! caller then imports it as usual.

use photocraft_psd::{FileSource, PsdFile, ReadAt, Source, header::ColorMode, resources::ids};

/// A preview image: 8-bit RGBA in the colours of `icc` (sRGB when `None`). `full` is the file's own
/// size in pixels, from its header.
pub struct PsdPreview {
    pub width: u32,
    pub height: u32,
    pub full: (u32, u32),
    pub rgba: Vec<u8>,
    pub icc: Option<Vec<u8>>,
}

/// Largest image-resources section read (a real one is some KB; Photoshop's own never passes a few MB).
const MAX_RESOURCES: u64 = 64 << 20;

/// Largest decoded merged image (planar, all channels) read for a preview.
const MAX_MERGED_BYTES: u64 = 1 << 30;

/// The preview of the PSD or PSB at `path`: its thumbnail resource, else its merged composite
/// reduced to `max_side`.
pub fn preview(path: &std::path::Path, max_side: u32) -> Option<PsdPreview> {
    let source = FileSource::open(path).ok()?;
    if let Some(p) = thumbnail(&source) {
        return Some(p);
    }
    merged(Source::new(source), max_side)
}

/// The thumbnail resource (1036) of the file in `source`, JPEG-decoded. Reads the 26-byte header,
/// the colour mode data length and the image resources section, nothing else.
pub fn thumbnail(source: &dyn ReadAt) -> Option<PsdPreview> {
    let mut head = [0u8; 30];
    source.read_at(0, &mut head).ok()?;
    if &head[..4] != b"8BPS" {
        return None;
    }
    let cmd_len = u64::from(u32::from_be_bytes(head[26..30].try_into().ok()?));
    let at = 30u64.checked_add(cmd_len)?;
    let mut len = [0u8; 4];
    source.read_at(at, &mut len).ok()?;
    let len = u64::from(u32::from_be_bytes(len));
    if len > MAX_RESOURCES {
        return None;
    }
    let mut section = vec![0u8; usize::try_from(len).ok()?];
    source.read_at(at + 4, &mut section).ok()?;
    let data = find_resource(&section, ids::THUMBNAIL)?;
    // 28-byte header (format 1 = JPEG), then the JFIF stream.
    if data.len() <= 28 || u32::from_be_bytes(data[..4].try_into().ok()?) != 1 {
        return None;
    }
    let img = photocraft_codecs::decode(&data[28..]).ok()?;
    let (width, height) = (img.width(), img.height());
    let full = (u32::from_be_bytes(head[18..22].try_into().ok()?), u32::from_be_bytes(head[14..18].try_into().ok()?));
    Some(PsdPreview { width, height, full, rgba: img.to_rgba8(), icc: None })
}

/// The data of the first resource `id` in an image resources section.
fn find_resource(mut s: &[u8], id: u16) -> Option<&[u8]> {
    while s.len() >= 12 {
        if &s[..4] != b"8BIM" {
            return None;
        }
        let this = u16::from_be_bytes([s[4], s[5]]);
        // Pascal name, padded so name length byte plus text is even.
        let name = 1 + usize::from(s[6]);
        let name = name + name % 2;
        let size_at = 6 + name;
        let size = u32::from_be_bytes(s.get(size_at..size_at + 4)?.try_into().ok()?) as usize;
        let start = size_at + 4;
        let data = s.get(start..start.checked_add(size)?)?;
        if this == id {
            return Some(data);
        }
        s = s.get(start + size + size % 2..)?;
    }
    None
}

/// The merged composite of an 8 or 16-bit RGB file, reduced to at most `max_side`, or `None` for
/// any other kind of file, a composite that would need more than [`MAX_MERGED_BYTES`], or one that
/// cannot be read. The layers are never touched.
pub fn merged(source: Source, max_side: u32) -> Option<PsdPreview> {
    let file = PsdFile::from_source(source).ok()?;
    let h = &file.header;
    if h.color_mode != ColorMode::Rgb || !matches!(h.depth, 8 | 16) || h.channels < 3 || h.width == 0 || h.height == 0 {
        return None;
    }
    let bytes_per = u64::from(h.depth / 8);
    let plane = u64::from(h.width) * u64::from(h.height) * bytes_per;
    if plane.checked_mul(u64::from(h.channels))? > MAX_MERGED_BYTES {
        return None;
    }
    let all = file.decode_merged().ok()?;
    let plane = usize::try_from(plane).ok()?;
    let alpha = file.merged_has_alpha() && h.channels >= 4;
    let (w, hh) = (h.width as usize, h.height as usize);
    let step = (w.max(hh) as u32).div_ceil(max_side.max(1)).max(1) as usize;
    let (ow, oh) = (w.div_ceil(step), hh.div_ceil(step));
    let bp = bytes_per as usize;
    let sample = |c: usize, i: usize| -> u32 {
        let o = c * plane + i * bp;
        match all.get(o..o + bp) {
            Some([v]) => u32::from(*v),
            // 16-bit: the high byte is the 8-bit value.
            Some([hi, _]) => u32::from(*hi),
            _ => 0,
        }
    };
    let mut rgba = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        for ox in 0..ow {
            let mut sum = [0u32; 4];
            let mut n = 0u32;
            for y in oy * step..((oy + 1) * step).min(hh) {
                for x in ox * step..((ox + 1) * step).min(w) {
                    let i = y * w + x;
                    for (c, s) in sum.iter_mut().enumerate().take(3) {
                        *s += sample(c, i);
                    }
                    sum[3] += if alpha { sample(3, i) } else { 255 };
                    n += 1;
                }
            }
            let n = n.max(1);
            rgba.extend(sum.map(|s| (s / n) as u8));
        }
    }
    Some(PsdPreview { width: ow as u32, height: oh as u32, full: (h.width, h.height), rgba, icc: file.icc_profile().map(<[u8]>::to_vec) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::{ImageResource, PsdBuilder, builder::PixelData};

    fn jpeg(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        let data: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
        let img = photocraft_codecs::Image::from_raw(w, h, photocraft_codecs::ChannelLayout::Rgb, photocraft_codecs::SampleType::U8, data).unwrap();
        photocraft_codecs::encode(&img, photocraft_codecs::Format::Jpeg, &photocraft_codecs::EncodeOptions::default()).unwrap()
    }

    #[test]
    fn reads_the_thumbnail_resource_from_the_start_of_the_file_only() {
        let jfif = jpeg(8, 4, [250, 10, 10]);
        let mut res = Vec::new();
        for v in [1u32, 8, 4, 24, 96, jfif.len() as u32] {
            res.extend_from_slice(&v.to_be_bytes());
        }
        res.extend_from_slice(&[0, 24, 0, 1]);
        res.extend_from_slice(&jfif);
        let mut b = PsdBuilder::new(8, 4).resource(ImageResource::new(ids::THUMBNAIL, res));
        b.composite(PixelData::Rgba8(vec![0; 8 * 4 * 4]));
        let mut bytes = b.to_bytes().unwrap();
        // Everything after the resources is cut off: the thumbnail still reads.
        let at = 30 + u32::from_be_bytes(bytes[26..30].try_into().unwrap()) as usize;
        let end = at + 4 + u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        bytes.truncate(end);
        let p = thumbnail(&bytes).expect("thumbnail");
        assert_eq!((p.width, p.height), (8, 4));
        assert_eq!(p.full, (8, 4));
        assert!(p.rgba[0] > 200 && p.rgba[1] < 60, "{:?}", &p.rgba[..4]);
    }

    #[test]
    fn a_file_without_a_thumbnail_has_none() {
        let mut b = PsdBuilder::new(2, 2);
        b.composite(PixelData::Rgba8(vec![255; 16]));
        assert!(thumbnail(&b.to_bytes().unwrap()).is_none());
    }

    #[test]
    fn reads_the_merged_image_reduced() {
        let mut px = Vec::new();
        for _ in 0..8 * 8 {
            px.extend_from_slice(&[10, 200, 30, 255]);
        }
        let mut b = PsdBuilder::new(8, 8);
        b.composite(PixelData::Rgba8(px));
        let bytes = b.to_bytes().unwrap();
        let p = merged(Source::new(bytes), 4).expect("merged");
        assert_eq!((p.width, p.height), (4, 4));
        assert_eq!(p.full, (8, 8), "the file's own size, not the reduced one");
        assert_eq!(&p.rgba[..4], &[10, 200, 30, 255]);
    }
}
