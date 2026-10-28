//! Registering one installed font file, also a collection (TTC/OTC) with hundreds of faces.

use std::io::{Read, Seek};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parley::fontique::{Blob, Collection};

/// Most faces of a collection file that fontique's path scan still handles in good time. It
/// registers the whole file once per face and merges everything gathered so far each time, so a
/// file's cost grows with the fourth power of its face count: a 100-face collection took 0.2 s,
/// a 200-face one 3.4 s and a 400-face one minutes, with the font scan holding up every PSD with
/// type (#9624).
const MAX_FACES_BY_PATH: u32 = 32;

/// Limits of the memory route: what one file may take, and what all of them may take together
/// (the bytes stay loaded for the whole process). Past either, a file is registered by path:
/// slow, but never missing.
struct Limits {
    file: u64,
    total: u64,
    loaded: &'static AtomicU64,
}

static LOADED: AtomicU64 = AtomicU64::new(0);

const LIMITS: Limits = Limits { file: 256 << 20, total: 512 << 20, loaded: &LOADED };

/// The bytes of a collection file that has more than [`MAX_FACES_BY_PATH`] faces, when the
/// memory route may take them; `None` for any other file or when a limit or an error rules it out.
fn big_collection_bytes(path: &Path, limits: &Limits) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;
    if &head[..4] != b"ttcf" || u32::from_be_bytes([head[8], head[9], head[10], head[11]]) <= MAX_FACES_BY_PATH {
        return None;
    }
    let name = path.display();
    let len = file.metadata().map_or(u64::MAX, |m| m.len());
    if len > limits.file || len.saturating_add(limits.loaded.load(Ordering::Relaxed)) > limits.total {
        log::warn!("font {name}: too big to load in memory ({len} bytes), registered by path (slow)");
        return None;
    }
    let mut bytes = Vec::new();
    // One more byte than allowed: a file that grew since `metadata` is caught too.
    let read = file.rewind().and_then(|()| file.take(limits.file + 1).read_to_end(&mut bytes));
    match read {
        Ok(n) if n as u64 <= limits.file => {
            limits.loaded.fetch_add(n as u64, Ordering::Relaxed);
            Some(bytes)
        }
        Ok(_) => {
            log::warn!("font {name}: grew past the memory limit while reading, registered by path (slow)");
            None
        }
        Err(e) => {
            log::warn!("font {name}: could not be read ({e}), registered by path (slow)");
            None
        }
    }
}

/// Registers the faces of `path` with `collection`. A collection of more than
/// [`MAX_FACES_BY_PATH`] faces is read once and registered from memory, which is linear in its
/// faces; every other file, and a collection the memory route may not take, is registered by
/// path, so fontique maps it lazily.
pub(super) fn register_file(collection: &mut Collection, path: &Path) {
    register_with(collection, path, &LIMITS);
}

fn register_with(collection: &mut Collection, path: &Path, limits: &Limits) {
    match big_collection_bytes(path, limits) {
        Some(bytes) => {
            collection.register_fonts(Blob::new(Arc::new(bytes)), None);
        }
        None => collection.load_fonts_from_paths([path]),
    }
}

#[cfg(test)]
mod tests;
