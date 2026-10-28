use super::*;
use parley::fontique::{CollectionOptions, SourceKind};

/// A TTC of `faces` faces that all share the table directory of `font` (a single TrueType file).
fn collection_of(font: &[u8], faces: u32) -> Vec<u8> {
    let tables = u16::from_be_bytes([font[4], font[5]]) as usize;
    let head = 12 + 4 * faces as usize;
    let mut out = b"ttcf".to_vec();
    out.extend(0x0001_0000u32.to_be_bytes());
    out.extend(faces.to_be_bytes());
    for _ in 0..faces {
        out.extend((head as u32).to_be_bytes());
    }
    out.extend(&font[..12]);
    for i in 0..tables {
        let r = 12 + i * 16;
        out.extend(&font[r..r + 8]);
        let offset = u32::from_be_bytes(font[r + 8..r + 12].try_into().unwrap());
        out.extend((offset + head as u32).to_be_bytes());
        out.extend(&font[r + 12..r + 16]);
    }
    out.extend(&font[12 + tables * 16..]);
    out
}

fn collection_file(name: &str, faces: u32) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("photocraft-big-collection-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, collection_of(&crate::fonts::INTER_REGULAR, faces)).unwrap();
    path
}

fn empty() -> Collection {
    Collection::new(CollectionOptions { shared: false, system_fonts: false })
}

#[test]
fn a_collection_of_hundreds_of_faces_registers_each_face_once_and_fast() {
    let path = collection_file("many.ttc", 400);
    let mut c = empty();
    let start = std::time::Instant::now();
    register_file(&mut c, &path);
    let took = start.elapsed();
    let _ = std::fs::remove_file(&path);
    let faces = c.family_by_name("Inter").map_or(0, |f| f.fonts().len());
    assert_eq!(faces, 400, "each face is registered once, not once per scan of the file");
    // fontique's path scan takes minutes for this file.
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
}

#[test]
fn a_small_collection_stays_registered_by_path() {
    let path = collection_file("few.ttc", 3);
    let mut c = empty();
    register_file(&mut c, &path);
    let _ = std::fs::remove_file(&path);
    let family = c.family_by_name("Inter").unwrap();
    assert!(!family.fonts().is_empty());
    assert!(family.fonts().iter().all(|f| matches!(f.source().kind(), SourceKind::Path(_))));
}

#[test]
fn collection_faces_reads_the_header_only() {
    let many = collection_file("header.ttc", 40);
    let plain = std::env::temp_dir().join(format!("photocraft-big-collection-{}/plain.ttf", std::process::id()));
    std::fs::write(&plain, &*crate::fonts::INTER_REGULAR).unwrap();
    assert_eq!(collection_faces(&many), Some(40));
    assert_eq!(collection_faces(&plain), None);
    assert_eq!(collection_faces(Path::new("/nonexistent/font.ttc")), None);
    let _ = std::fs::remove_file(many);
    let _ = std::fs::remove_file(plain);
}
