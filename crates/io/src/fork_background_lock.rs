//! Fork: Settings › General › Lock the Background Layer. Off (the default), a flat image opens
//! as a normal "Layer 0" (and File › New makes one) instead of a Background layer with its
//! position and transparency locked, so layers can go below it.

use crate::{ImportOptions, ImportResult, IoError};
use photocraft_doc::Document;

/// Unlock the Background of a one-layer document and name it "Layer 0". Returns whether it did.
pub fn unlock(doc: &mut Document) -> bool {
    let [layer] = doc.layers.as_mut_slice() else { return false };
    if !layer.has_background_traits() {
        return false;
    }
    layer.locks.transparency = false;
    layer.locks.position = false;
    layer.name = "Layer 0".into();
    true
}

/// The result of a flat import, with its Background unlocked unless `opts.lock_background`.
pub(crate) fn apply(r: Result<ImportResult, IoError>, opts: &ImportOptions) -> Result<ImportResult, IoError> {
    let mut r = r?;
    if !opts.lock_background {
        unlock(&mut r.document);
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_codecs::{ChannelLayout, EncodeOptions, Format, Image};

    fn opaque_png() -> Vec<u8> {
        let img = Image::from_u8(4, 3, ChannelLayout::Rgb, vec![200; 4 * 3 * 3]).unwrap();
        photocraft_codecs::encode(&img, Format::Png, &EncodeOptions::default()).unwrap()
    }

    #[test]
    fn opaque_photo_opens_locked_by_default_and_unlocked_when_off() {
        let png = opaque_png();
        let on = crate::import_with_options("p.png", &png, &ImportOptions::default()).unwrap().document;
        assert!(on.background().is_some());
        let off = ImportOptions { lock_background: false, ..Default::default() };
        let d = crate::import_with_options("p.png", &png, &off).unwrap().document;
        assert!(d.background().is_none());
        assert_eq!(d.layers[0].name, "Layer 0");
        assert!(!d.layers[0].locks.position && !d.layers[0].locks.transparency);
    }

    #[test]
    fn unlock_leaves_other_documents_alone() {
        let mut d = Document::new("t", photocraft_doc::Size::new(2, 2), photocraft_doc::ColorMode::Rgb, photocraft_doc::SampleType::U8);
        assert!(!unlock(&mut d));
    }
}
