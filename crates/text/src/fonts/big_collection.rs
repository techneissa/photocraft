//! Registering one installed font file, also a collection (TTC/OTC) with hundreds of faces.

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use parley::fontique::{Blob, Collection};

/// Most faces of a collection file that fontique's path scan still handles in good time. It
/// registers the whole file once per face and merges everything gathered so far each time, so a
/// file's cost grows with the fourth power of its face count: a 100-face collection took 0.2 s,
/// a 200-face one 3.4 s and a 400-face one minutes, with the font scan holding up every PSD with
/// type (#9624).
const MAX_FACES_BY_PATH: u32 = 32;

/// Largest collection file read into memory (bigger ones are skipped, as they were unusable).
const MAX_READ_BYTES: u64 = 256 << 20;

/// Faces named by the header of a TTC/OTC file; `None` for any other file.
fn collection_faces(path: &Path) -> Option<u32> {
    let mut head = [0u8; 12];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    (head.get(..4)? == b"ttcf").then(|| u32::from_be_bytes([head[8], head[9], head[10], head[11]]))
}

/// Registers the faces of `path` with `collection`. A collection of more than
/// [`MAX_FACES_BY_PATH`] faces is read once and registered from memory, which is linear in its
/// faces; every other file is registered by path, so fontique maps it lazily.
pub(super) fn register_file(collection: &mut Collection, path: &Path) {
    if collection_faces(path).is_some_and(|n| n > MAX_FACES_BY_PATH) {
        let small = std::fs::metadata(path).is_ok_and(|m| m.len() <= MAX_READ_BYTES);
        if let (true, Ok(bytes)) = (small, std::fs::read(path)) {
            collection.register_fonts(Blob::new(Arc::new(bytes)), None);
        }
        return;
    }
    collection.load_fonts_from_paths([path]);
}

#[cfg(test)]
mod tests;
