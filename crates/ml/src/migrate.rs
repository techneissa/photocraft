//! Moving the models of the old `PhotoCraft/models` folder into the folder shared by the Craft
//! apps (`ArtCraft/models/<model-id>/`), once, when the app (or `photocraft-cli models`) starts.
//!
//! The rules, from the strictest:
//!
//! * A model that is verified (published size and SHA-256) in the shared folder is never deleted,
//!   overwritten or downloaded again. If the old folder holds a copy of one of its files, only that
//!   old duplicate is removed.
//! * A file moves by `rename` (the same disk: nothing is copied, nothing is downloaded), and only
//!   when the old copy itself passes the size and SHA-256 check. A shared copy that fails the
//!   check is not a model anyone can use, so a verified old copy replaces it.
//! * Nothing moves while another process holds the model's install lock (a download or a removal).
//! * A move that fails (no permission, another disk, a folder where the file should go) leaves
//!   both files where they are and says why in the [`Migration`]; the app starts anyway, and the
//!   old copy is still found by the read-only lookup in `store.rs`.
//! * Old files nothing reads any more (`lama_fp32.onnx`, the ONNX LaMa) are removed once the model
//!   that replaced them is verified in the shared folder, and not before.
//! * The old empty `.lock` files go, and the old folder goes when it is empty.
//!
//! Nothing here runs while `PHOTOCRAFT_MODELS_DIR` is set (then no other folder is used) or while
//! models are switched off ([`crate::models_disabled`]), so tests never touch a real folder.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use crate::install::{lock_model, verify};
use crate::store::{Model, ModelFile, Roots, installed_in, models_disabled};

/// What a migration did, in sentences for a log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Migration {
    /// Files moved into the shared folder, old duplicates and unused old files removed.
    pub done: Vec<String>,
    /// What could not be done, and why; the files involved are still where they were.
    pub failed: Vec<String>,
}

impl Migration {
    /// Whether there was nothing to do and nothing went wrong.
    pub fn is_empty(&self) -> bool {
        self.done.is_empty() && self.failed.is_empty()
    }
}

/// One model's part of the move.
struct Entry<'a> {
    /// The model's folder name in the shared folder.
    id: &'a str,
    /// Its files; one found directly in the old folder moves to `<id>/`.
    files: &'a [ModelFile],
    /// Old files that nothing reads any more: removed once `files` are all verified in the shared folder.
    retired: &'a [&'a str],
}

/// The old files of each model PhotoCraft can download. The ONNX LaMa (`lama_fp32.onnx`) is retired:
/// the model is `big-lama.pt` now (`crates/lama`).
const RETIRED_LAMA: [&str; 1] = ["lama_fp32.onnx"];

/// Moves the old `PhotoCraft/models` files into the shared folder (see the module docs). Blocking:
/// hashing a 224 MB file takes about a second, so call it from a worker thread. Never panics.
pub fn migrate_old_models() -> Migration {
    if models_disabled() {
        return Migration::default();
    }
    let entries: Vec<Entry<'_>> = Model::ALL
        .into_iter()
        .filter(|m| m.downloadable())
        .map(|m| Entry { id: m.id(), files: m.files(), retired: if m == Model::Lama { &RETIRED_LAMA } else { &[] } })
        .collect();
    migrate(&Roots::current(), &entries)
}

fn migrate(roots: &Roots, entries: &[Entry<'_>]) -> Migration {
    let mut report = Migration::default();
    if roots.custom().is_some() {
        return report;
    }
    let (Some(shared), Some(old)) = (roots.shared(), roots.old()) else { return report };
    if !old.is_dir() {
        return report;
    }
    for entry in entries {
        for file in entry.files {
            move_file(&mut report, entry.id, file, old, shared);
        }
        // The replacement must be verified where it is, not just present.
        // Not an error when it is not: the file is removed at the first start after the replacement is installed.
        if entry.retired.iter().any(|name| exists(&old.join(name))) && installed_in(&shared.join(entry.id), entry.files) {
            for name in entry.retired {
                retire(&mut report, &old.join(name), entry.id);
            }
        }
    }
    clean_locks(&mut report, old, entries);
    // Only an empty folder goes; a file of someone else (or a `.part` of a running download) keeps it.
    if std::fs::read_dir(old).is_ok_and(|mut d| d.next().is_none()) && std::fs::remove_dir(old).is_ok() {
        report.done.push(format!("removed the empty old folder {}", old.display()));
    }
    report
}

fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Moves `old/<file.name>` to `shared/<id>/<file.name>` if it is there and fine.
fn move_file(report: &mut Migration, id: &str, file: &ModelFile, old: &Path, shared: &Path) {
    let from = old.join(file.name);
    let Ok(meta) = std::fs::symlink_metadata(&from) else { return };
    if !meta.file_type().is_file() {
        report.failed.push(format!("left {} alone: it is not a plain file", from.display()));
        return;
    }
    // Only a copy that is the published model moves; a wrong or half-written one stays where it is.
    if let Err(e) = std::fs::File::open(&from).map_err(|e| e.to_string()).and_then(|mut f| verify(&mut f, file.size, file.sha256).map_err(|e| e.to_string())) {
        report.failed.push(format!("left {} in the old folder: it is not the published {} file ({e})", from.display(), file.name));
        return;
    }
    let dir = shared.join(id);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        report.failed.push(format!("left {} in the old folder: cannot create {} ({e})", from.display(), dir.display()));
        return;
    }
    let _lock = match lock_model(&dir) {
        Ok(lock) => lock,
        Err(e) => {
            report.failed.push(format!("left {} in the old folder: {e}", from.display()));
            return;
        }
    };
    let to = dir.join(file.name);
    // A verified copy is already there: it stays exactly as it is, and only the old duplicate goes.
    if exists(&to) && std::fs::File::open(&to).is_ok_and(|mut f| verify(&mut f, file.size, file.sha256).is_ok()) {
        match std::fs::remove_file(&from) {
            Ok(()) => report.done.push(format!("removed the old duplicate {} ({} is already in the shared folder)", from.display(), to.display())),
            Err(e) => report.failed.push(format!("could not remove the old duplicate {}: {e}", from.display())),
        }
        return;
    }
    // Nothing usable is there (nothing at all, or a copy that fails the check): the verified old file takes its place.
    match std::fs::rename(&from, &to) {
        Ok(()) => report.done.push(format!("moved {} to {}", from.display(), to.display())),
        Err(e) => report.failed.push(format!("could not move {} to {}: {e}; both stay where they are", from.display(), to.display())),
    }
}

/// Removes an old file nothing reads any more.
fn retire(report: &mut Migration, path: &Path, id: &str) {
    if !exists(path) {
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => report.done.push(format!("removed {}: the {id} model in the shared folder replaces it", path.display())),
        Err(e) => report.failed.push(format!("could not remove {}: {e}", path.display())),
    }
}

/// Removes the empty `<file>.lock` files the old downloads left, unless a process holds one.
fn clean_locks(report: &mut Migration, old: &Path, entries: &[Entry<'_>]) {
    let names = entries.iter().flat_map(|e| e.files.iter().map(|f| f.name).chain(e.retired.iter().copied()));
    for name in names {
        let lock: PathBuf = old.join(format!("{name}.lock"));
        if !std::fs::metadata(&lock).is_ok_and(|m| m.is_file() && m.len() == 0) {
            continue;
        }
        let Ok(file) = OpenOptions::new().write(true).open(&lock) else { continue };
        // Held by an old PhotoCraft that is downloading right now: leave it.
        if file.try_lock().is_ok() {
            match std::fs::remove_file(&lock) {
                Ok(()) => report.done.push(format!("removed the old lock file {}", lock.display())),
                Err(e) => report.failed.push(format!("could not remove the old lock file {}: {e}", lock.display())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const ABC_FILE: [ModelFile; 1] = [ModelFile { name: "m.bin", url: "", sha256: ABC, size: 3 }];

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> std::io::Result<Self> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("photocraft-ml-migrate-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
        fn shared(&self) -> PathBuf {
            self.0.join("shared")
        }
        fn old(&self) -> PathBuf {
            self.0.join("old")
        }
        fn roots(&self) -> Roots {
            Roots::new(None, Some(self.shared()), Some(self.old()))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn entry<'a>(files: &'a [ModelFile], retired: &'a [&'a str]) -> Vec<Entry<'a>> {
        vec![Entry { id: "m", files, retired }]
    }

    fn put(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, bytes)
    }

    #[test]
    fn a_fresh_old_file_moves_and_the_old_folder_goes() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin"), b"abc")?;
        put(&t.old().join("m.bin.lock"), b"")?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert!(report.failed.is_empty(), "{report:?}");
        assert_eq!(std::fs::read(t.shared().join("m").join("m.bin"))?, b"abc");
        assert!(!t.old().exists(), "the old file, its empty lock and the then empty folder are gone");
        assert!(report.done.iter().any(|l| l.starts_with("moved ")), "{report:?}");
        // A second start finds nothing to do.
        assert!(migrate(&t.roots(), &entry(&ABC_FILE, &[])).is_empty());
        Ok(())
    }

    #[test]
    fn a_verified_shared_copy_is_left_alone_and_only_the_old_duplicate_goes() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let shared = t.shared().join("m").join("m.bin");
        put(&shared, b"abc")?;
        let before = std::fs::metadata(&shared)?.modified()?;
        put(&t.old().join("m.bin"), b"abc")?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert!(report.failed.is_empty(), "{report:?}");
        assert_eq!(std::fs::read(&shared)?, b"abc");
        assert_eq!(std::fs::metadata(&shared)?.modified()?, before, "the shared file is not rewritten");
        assert!(!t.old().exists());
        assert!(report.done.iter().any(|l| l.contains("old duplicate")), "{report:?}");
        Ok(())
    }

    #[test]
    fn a_corrupt_shared_copy_is_replaced_by_the_verified_old_one() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let shared = t.shared().join("m").join("m.bin");
        put(&shared, b"abd")?;
        put(&t.old().join("m.bin"), b"abc")?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert!(report.failed.is_empty(), "{report:?}");
        assert_eq!(std::fs::read(&shared)?, b"abc");
        assert!(!t.old().join("m.bin").exists());
        Ok(())
    }

    #[test]
    fn a_corrupt_old_file_is_not_moved() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin"), b"abd")?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert_eq!(std::fs::read(t.old().join("m.bin"))?, b"abd");
        assert!(!t.shared().join("m").join("m.bin").exists());
        assert!(report.failed.iter().any(|l| l.contains("not the published")), "{report:?}");
        Ok(())
    }

    #[test]
    fn a_failed_rename_leaves_both_files_and_says_why() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin"), b"abc")?;
        // A folder where the file should go: the rename cannot succeed, and nothing may be deleted to make room.
        std::fs::create_dir_all(t.shared().join("m").join("m.bin"))?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert_eq!(std::fs::read(t.old().join("m.bin"))?, b"abc", "the old file stays");
        assert!(t.shared().join("m").join("m.bin").is_dir(), "the shared folder is as it was");
        assert!(report.failed.iter().any(|l| l.contains("could not move") && l.contains("both stay")), "{report:?}");
        assert!(t.old().exists(), "a folder with a file in it stays");
        Ok(())
    }

    #[test]
    fn a_shared_folder_that_cannot_be_made_is_a_failure_not_a_crash() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin"), b"abc")?;
        // The shared folder is a plain file.
        put(&t.shared(), b"not a folder")?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert_eq!(std::fs::read(t.old().join("m.bin"))?, b"abc");
        assert!(report.failed.iter().any(|l| l.contains("cannot create")), "{report:?}");
        Ok(())
    }

    #[test]
    fn a_model_being_downloaded_elsewhere_is_not_moved_onto() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin"), b"abc")?;
        let dir = t.shared().join("m");
        std::fs::create_dir_all(&dir)?;
        let _held = lock_model(&dir)?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert_eq!(std::fs::read(t.old().join("m.bin"))?, b"abc");
        assert!(report.failed.iter().any(|l| l.contains("downloaded or removed")), "{report:?}");
        Ok(())
    }

    #[test]
    fn a_retired_old_file_goes_only_when_its_replacement_is_verified() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("retired.onnx"), b"old model")?;
        put(&t.old().join("retired.onnx.lock"), b"")?;
        let entries = [Entry { id: "m", files: &ABC_FILE, retired: &["retired.onnx"] }];
        // The replacement is not there: the old file stays, so a later start can still remove it.
        migrate(&t.roots(), &entries);
        assert!(t.old().join("retired.onnx").exists());
        // The replacement is there but corrupt: still not enough.
        put(&t.shared().join("m").join("m.bin"), b"abd")?;
        migrate(&t.roots(), &entries);
        assert!(t.old().join("retired.onnx").exists());
        // Verified: the old file, its lock and the folder go; the shared model is untouched.
        put(&t.shared().join("m").join("m.bin"), b"abc")?;
        let report = migrate(&t.roots(), &entries);
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(!t.old().exists());
        assert_eq!(std::fs::read(t.shared().join("m").join("m.bin"))?, b"abc");
        Ok(())
    }

    #[test]
    fn nothing_happens_with_a_custom_folder_or_no_old_folder() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin"), b"abc")?;
        let custom = Roots::new(Some(t.0.join("custom")), Some(t.shared()), Some(t.old()));
        assert!(migrate(&custom, &entry(&ABC_FILE, &[])).is_empty());
        assert!(t.old().join("m.bin").exists());
        let none = Roots::new(None, Some(t.shared()), Some(t.0.join("nowhere")));
        assert!(migrate(&none, &entry(&ABC_FILE, &[])).is_empty());
        assert!(!t.shared().exists());
        Ok(())
    }

    #[test]
    fn a_held_lock_file_stays_and_so_does_a_non_empty_folder() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        put(&t.old().join("m.bin.lock"), b"")?;
        put(&t.old().join("other.txt"), b"mine")?;
        let held = OpenOptions::new().write(true).open(t.old().join("m.bin.lock"))?;
        held.try_lock()?;
        let report = migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert!(t.old().join("m.bin.lock").exists(), "{report:?}");
        assert!(t.old().join("other.txt").exists());
        drop(held);
        migrate(&t.roots(), &entry(&ABC_FILE, &[]));
        assert!(!t.old().join("m.bin.lock").exists());
        assert!(t.old().join("other.txt").exists(), "files that are not the app's are never touched");
        Ok(())
    }
}
