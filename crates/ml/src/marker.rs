//! A small file that remembers which big model files already passed their SHA-256 check, so the
//! check (7 to 25 seconds for SAM 3's 3.4 GB on this Mac, repeated by every process) runs once
//! and not at every start of the app.
//!
//! One line per file: its path, then its size, modification time, inode, change time (Unix) and
//! the SHA-256 it was checked against. A file counts as checked only while all of them still
//! match: a copy over it, a rename, a new size, a new time or a new pinned hash in a later app
//! version checks it again. Only files of [`MIN_SIZE`] or more are remembered (the small models
//! hash in about a second). A missing, unreadable or damaged marker only means "check again", and
//! a marker that cannot be written is ignored.

use std::path::{Path, PathBuf};

/// Files smaller than this are always hashed.
pub(crate) const MIN_SIZE: u64 = 1 << 30;

/// The marker file's name, in PhotoCraft's own data folder (never in the models folder, which the
/// other Craft apps share).
const FILE_NAME: &str = "model-checks.tsv";

/// What a checked file looked like when it was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stamp {
    path: String,
    /// Everything after the path in the marker's line.
    facts: String,
}

impl Stamp {
    /// The stamp of the file at `path` with `meta`, checked against `sha256`. `None` for a path
    /// the marker's lines cannot hold.
    pub(crate) fn of(path: &Path, meta: &std::fs::Metadata, sha256: &str) -> Option<Self> {
        let path = path.to_str().filter(|p| !p.contains(['\t', '\n', '\r']))?.to_string();
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        #[cfg(unix)]
        let (ino, ctime, ctime_ns) = {
            use std::os::unix::fs::MetadataExt;
            (meta.ino(), meta.ctime(), meta.ctime_nsec())
        };
        #[cfg(not(unix))]
        let (ino, ctime, ctime_ns) = (0u64, 0i64, 0i64);
        Some(Self { path, facts: format!("{}\t{mtime}\t{ino}\t{ctime}\t{ctime_ns}\t{sha256}", meta.len()) })
    }
}

/// The marker file: `PhotoCraft/model-checks.tsv` in the user's data folder (`None` where no home
/// folder is known).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn marker_path() -> Option<PathBuf> {
    crate::store::app_data_dir("PhotoCraft").map(|d| d.join(FILE_NAME))
}

fn parse(text: &str) -> Vec<Stamp> {
    text.lines()
        .filter_map(|l| {
            let (path, facts) = l.split_once('\t')?;
            (facts.split('\t').count() == 6 && !path.is_empty()).then(|| Stamp { path: path.into(), facts: facts.into() })
        })
        .collect()
}

fn render(stamps: &[Stamp]) -> String {
    stamps.iter().map(|s| format!("{}\t{}\n", s.path, s.facts)).collect()
}

/// Whether the marker at `marker` holds exactly `stamp`.
pub(crate) fn recorded(marker: &Path, stamp: &Stamp) -> bool {
    std::fs::read_to_string(marker).is_ok_and(|text| parse(&text).contains(stamp))
}

/// Remember `stamp` in the marker at `marker` (replacing the file's older line). Written to a
/// temporary file and renamed, so a crash never leaves half a marker. Failures are ignored.
pub(crate) fn record(marker: &Path, stamp: Stamp) {
    let mut stamps = std::fs::read_to_string(marker).map(|t| parse(&t)).unwrap_or_default();
    stamps.retain(|s| s.path != stamp.path);
    stamps.push(stamp);
    // A marker for files long gone must not grow without bound.
    if stamps.len() > 64 {
        stamps.remove(0);
    }
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = marker.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, render(&stamps)).is_err() || std::fs::rename(&tmp, marker).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    struct Temp(PathBuf);
    impl Temp {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("photocraft-ml-marker-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
            Self(dir)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn stamp(file: &Path, sha: &str) -> Stamp {
        let meta = std::fs::metadata(file).unwrap();
        Stamp::of(file, &meta, sha).unwrap()
    }

    #[test]
    fn a_checked_file_is_remembered_until_it_changes() {
        let t = Temp::new("changes");
        let (file, marker) = (t.0.join("model.bin"), t.0.join("sub").join("model-checks.tsv"));
        std::fs::write(&file, b"abc").unwrap();
        assert!(!recorded(&marker, &stamp(&file, SHA)), "no marker yet");
        record(&marker, stamp(&file, SHA));
        assert!(recorded(&marker, &stamp(&file, SHA)));
        // another pinned hash (a later app version)
        assert!(!recorded(&marker, &stamp(&file, &"0".repeat(64))));
        // a new size
        std::fs::write(&file, b"abcd").unwrap();
        assert!(!recorded(&marker, &stamp(&file, SHA)));
        // the same size, a new modification time
        record(&marker, stamp(&file, SHA));
        assert!(recorded(&marker, &stamp(&file, SHA)));
        let f = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
        f.set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000)).unwrap();
        assert!(!recorded(&marker, &stamp(&file, SHA)));
    }

    #[test]
    fn recording_again_replaces_the_files_line_and_keeps_the_others() {
        let t = Temp::new("replace");
        let marker = t.0.join("m.tsv");
        let (a, b) = (t.0.join("a.bin"), t.0.join("b.bin"));
        std::fs::write(&a, b"1").unwrap();
        std::fs::write(&b, b"22").unwrap();
        record(&marker, stamp(&a, SHA));
        record(&marker, stamp(&b, SHA));
        std::fs::write(&a, b"333").unwrap();
        record(&marker, stamp(&a, SHA));
        assert!(recorded(&marker, &stamp(&a, SHA)) && recorded(&marker, &stamp(&b, SHA)));
        assert_eq!(std::fs::read_to_string(&marker).unwrap().lines().count(), 2);
    }

    #[test]
    fn a_damaged_marker_only_means_check_again() {
        let t = Temp::new("damaged");
        let (file, marker) = (t.0.join("model.bin"), t.0.join("m.tsv"));
        std::fs::write(&file, b"abc").unwrap();
        std::fs::write(&marker, b"\xff\xfe not text \t\t\n\n\tonly\n").unwrap();
        assert!(!recorded(&marker, &stamp(&file, SHA)));
        record(&marker, stamp(&file, SHA));
        assert!(recorded(&marker, &stamp(&file, SHA)));
        // lines are parsed back as written
        let text = std::fs::read_to_string(&marker).unwrap();
        assert_eq!(render(&parse(&text)), text);
    }

    #[test]
    fn a_path_the_lines_cannot_hold_is_not_recorded() {
        let t = Temp::new("odd");
        let file = t.0.join("a\tb.bin");
        std::fs::write(&file, b"abc").unwrap();
        assert!(Stamp::of(&file, &std::fs::metadata(&file).unwrap(), SHA).is_none());
    }
}
