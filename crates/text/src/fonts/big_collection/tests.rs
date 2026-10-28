use super::*;
use parley::fontique::{CollectionOptions, SourceKind};
use std::time::{Duration, Instant};

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

/// A temporary folder that is removed when the test ends, also when it fails.
struct Dir(std::path::PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("photocraft-big-collection-{}-{test}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn collection(&self, name: &str, faces: u32) -> std::path::PathBuf {
        self.file(name, &collection_of(&crate::fonts::INTER_REGULAR, faces))
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn empty() -> Collection {
    Collection::new(CollectionOptions { shared: false, system_fonts: false })
}

fn limits(file: u64, total: u64) -> Limits {
    Limits { file, total, loaded: Box::leak(Box::new(AtomicU64::new(0))) }
}

fn sources(c: &mut Collection) -> Vec<bool> {
    let family = c.family_by_name("Inter").unwrap();
    family.fonts().iter().map(|f| matches!(f.source().kind(), SourceKind::Path(_))).collect()
}

#[test]
fn a_collection_of_hundreds_of_faces_registers_each_face_once() {
    let dir = Dir::new("many");
    let path = dir.collection("many.ttc", 400);
    let mut c = empty();
    let start = Instant::now();
    register_file(&mut c, &path);
    let faces = c.family_by_name("Inter").map_or(0, |f| f.fonts().len());
    assert_eq!(faces, 400, "each face is registered once, not once per scan of the file");
    assert!(sources(&mut c).iter().all(|by_path| !by_path), "read once, registered from memory");
    // fontique's path scan takes minutes for this file; the bound is generous for a loaded machine.
    assert!(start.elapsed() < Duration::from_secs(60), "took {:?}", start.elapsed());
}

#[test]
fn a_small_collection_stays_registered_by_path() {
    let dir = Dir::new("few");
    let path = dir.collection("few.ttc", 3);
    let mut c = empty();
    register_file(&mut c, &path);
    let by_path = sources(&mut c);
    assert!(!by_path.is_empty() && by_path.iter().all(|p| *p));
}

#[test]
fn header_edge_cases_never_panic_and_take_the_path_route() {
    let dir = Dir::new("edges");
    let mut huge = b"ttcf\0\x01\0\0".to_vec();
    huge.extend(0xFFFF_FFFFu32.to_be_bytes());
    let cases = [
        ("empty.ttc", Vec::new()),
        ("truncated.ttc", b"ttcf\0\x01".to_vec()),
        ("zero.ttc", b"ttcf\0\x01\0\0\0\0\0\0".to_vec()),
        ("plain.ttf", crate::fonts::INTER_REGULAR.to_vec()),
        ("garbage.otf", vec![7u8; 100]),
    ];
    for (name, bytes) in cases {
        let path = dir.file(name, &bytes);
        assert!(big_collection_bytes(&path, &LIMITS).is_none(), "{name}");
        register_file(&mut empty(), &path);
    }
    assert!(big_collection_bytes(Path::new("/nonexistent/font.ttc"), &LIMITS).is_none());
    // A header that claims 4 billion faces in a 12-byte file must not hang the scan either.
    let path = dir.file("huge.ttc", &huge);
    let start = Instant::now();
    register_file(&mut empty(), &path);
    assert!(start.elapsed() < Duration::from_secs(60), "took {:?}", start.elapsed());
}

#[test]
fn a_collection_over_a_limit_is_registered_by_path_not_dropped() {
    let dir = Dir::new("limits");
    let path = dir.collection("big.ttc", 40);
    let size = std::fs::metadata(&path).unwrap().len();

    // Over the per-file limit.
    let l = limits(size - 1, u64::MAX);
    assert!(big_collection_bytes(&path, &l).is_none());
    let mut c = empty();
    register_with(&mut c, &path, &l);
    // fontique's path route lists the faces more than once (that is its slowness); none is dropped.
    assert!(sources(&mut c).len() >= 40);
    assert!(sources(&mut c).iter().all(|p| *p));

    // Within the limits: loaded, and counted.
    let l = limits(size, 2 * size);
    assert_eq!(big_collection_bytes(&path, &l).map(|b| b.len() as u64), Some(size));
    assert_eq!(l.loaded.load(Ordering::Relaxed), size);

    // The second file would pass the total: by path from then on.
    assert!(big_collection_bytes(&path, &l).is_some());
    assert!(big_collection_bytes(&path, &l).is_none(), "total of {} bytes is used up", 2 * size);
    assert_eq!(l.loaded.load(Ordering::Relaxed), 2 * size);
}

static BUILDS: AtomicU64 = AtomicU64::new(0);

fn counted_scan() -> u32 {
    BUILDS.fetch_add(1, Ordering::SeqCst);
    9
}

/// The system font scan is `Warm`'s build: the launch warm-up and every later open share one run.
#[test]
fn the_font_scan_runs_once_however_many_opens_ask() {
    static W: crate::warm::Warm<u32> = crate::warm::Warm::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    while W.try_get(counted_scan, true).is_none() {
        assert!(Instant::now() < deadline, "the background build never finished");
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..2 {
        assert_eq!(W.get_or_init(counted_scan), &9);
        assert_eq!(W.try_get(counted_scan, true), Some(&9));
    }
    assert_eq!(BUILDS.load(Ordering::SeqCst), 1);
}
