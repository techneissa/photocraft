//! Installing and removing model files. This is the only network access in the crate, and it
//! happens only when a user asks for a download (Settings › AI Models, `photocraft-cli models
//! fetch`); inference never touches the network.
//!
//! Bytes stream into a `.part` file beside each model file and are checked against the published size
//! and SHA-256 before an atomic rename, so the loader never sees a partial or corrupt file.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::store::{Model, ModelFile, install_dir, model_location};
use crate::{Cancel, MlError, Source};

/// Gives the bytes of one model file.
type Fetch<'a> = &'a dyn Fn(&ModelFile) -> Result<Box<dyn Read>, MlError>;

/// Download every file of `model` into [`install_dir`] (the shared folder, else
/// `PHOTOCRAFT_MODELS_DIR`), reporting `progress(done, total)` bytes over all the files and
/// stopping with [`MlError::Cancelled`] once `cancel` says so. A file already there and verified
/// is kept as it is, so a download that stopped half way continues with the files it lacks.
/// Returns the path of the model's first file. Blocking: call it from a worker thread.
pub fn download(model: Model, progress: &dyn Fn(u64, u64), cancel: Cancel<'_>) -> Result<PathBuf, MlError> {
    let dir = install_dir(model).ok_or_else(|| MlError::Download("cannot locate the models folder; set PHOTOCRAFT_MODELS_DIR".into()))?;
    let client = std::cell::OnceCell::new();
    let fetch = |file: &ModelFile| -> Result<Box<dyn Read>, MlError> {
        if client.get().is_none() {
            let built = reqwest::blocking::Client::builder()
                .https_only(true)
                .user_agent(concat!("photocraft/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(30))
                // Per connect/read/write, so a stalled server fails (and Cancel is seen) within this.
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| MlError::Download(format!("create download client: {e}")))?;
            let _ = client.set(built);
        }
        let client = client.get().ok_or_else(|| MlError::Download("create download client".into()))?;
        let response = client.get(file.url).send().and_then(reqwest::blocking::Response::error_for_status).map_err(|e| MlError::Download(e.to_string()))?;
        Ok(Box::new(response))
    };
    download_files(&dir, model, &fetch, progress, cancel)
}

/// [`download`] with the bytes of each file from `fetch`.
fn download_files(dir: &Path, model: Model, fetch: Fetch<'_>, progress: &dyn Fn(u64, u64), cancel: Cancel<'_>) -> Result<PathBuf, MlError> {
    install_all(dir, model.files(), fetch, progress, cancel)?;
    Ok(dir.join(model.file_name()))
}

/// Put every file of `files` into `dir`, fetching only those without a verified copy.
fn install_all(dir: &Path, files: &[ModelFile], fetch: Fetch<'_>, progress: &dyn Fn(u64, u64), cancel: Cancel<'_>) -> Result<(), MlError> {
    std::fs::create_dir_all(dir).map_err(|e| MlError::Download(format!("create {}: {e}", dir.display())))?;
    let _lock = lock_model(dir)?;
    let total: u64 = files.iter().map(|f| f.size).sum();
    let mut before = 0u64;
    for file in files {
        let path = dir.join(file.name);
        let shifted = |done: u64, _: u64| progress(before.saturating_add(done), total);
        if let Ok(mut f) = File::open(&path)
            && verify(&mut f, file.size, file.sha256).is_ok()
        {
            shifted(file.size, file.size);
        } else {
            // The lock is held, so any `.part` file is left over from a download that died.
            clear_stale_part(&path)?;
            install_from(fetch(file)?, &path, file.size, file.sha256, &shifted, cancel)?;
        }
        before = before.saturating_add(file.size);
    }
    Ok(())
}

/// Delete the installed files of `model` (and any leftover partial download) from the folder
/// downloads go into and from the shared or custom folder it was found in. The old `PhotoCraft`
/// folder is read only and never touched. Removing a model that is not installed is not an error.
pub fn remove(model: Model) -> Result<(), MlError> {
    let target = install_dir(model).ok_or_else(|| MlError::Download("cannot locate the models folder".into()))?;
    let mut dirs = vec![target];
    if let Some(found) = model_location(model).filter(|l| l.source != Source::Old)
        && !dirs.contains(&found.dir)
    {
        dirs.push(found.dir);
    }
    for dir in dirs.iter().filter(|d| d.is_dir()) {
        remove_in(dir, model.files())?;
    }
    Ok(())
}

fn remove_in(dir: &Path, files: &[ModelFile]) -> Result<(), MlError> {
    let _lock = lock_model(dir)?;
    for file in files {
        let path = dir.join(file.name);
        for path in [part_path(&path), path] {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(MlError::Download(format!("remove {}: {e}", path.display()))),
            }
        }
    }
    Ok(())
}

/// Exclusive, cross-process hold on one model folder for a download or removal, so a removal
/// can't unlink another worker's partial file and two fetches can't publish each other's bytes.
/// Released when the returned file is dropped (or the process exits).
fn lock_model(dir: &Path) -> Result<File, MlError> {
    let path = dir.join(".install.lock");
    let file =
        OpenOptions::new().create(true).truncate(false).write(true).open(&path).map_err(|e| MlError::Download(format!("open {}: {e}", path.display())))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => {
            Err(MlError::Download("the model is being downloaded or removed elsewhere; try again when that finishes".into()))
        }
        Err(std::fs::TryLockError::Error(e)) => Err(MlError::Download(format!("lock {}: {e}", path.display()))),
    }
}

/// Delete the partial download next to `path`, if any. Missing is fine.
fn clear_stale_part(path: &Path) -> Result<(), MlError> {
    let part = part_path(path);
    match std::fs::remove_file(&part) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(MlError::Download(format!("remove {}: {e}", part.display()))),
    }
}

fn part_path(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    PathBuf::from(part)
}

/// Check the bytes on disk, including same-size corruption.
pub(crate) fn verify(file: &mut File, size: u64, sha256: &str) -> Result<(), MlError> {
    let actual = file.metadata().map_err(|e| MlError::Download(e.to_string()))?.len();
    if actual != size {
        return Err(MlError::Download(format!("size mismatch: expected {size} bytes, got {actual}")));
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| MlError::Download(e.to_string()))?;
    let mut hash = Sha256::new();
    std::io::copy(file, &mut hash).map_err(|e| MlError::Download(e.to_string()))?;
    let actual = format!("{:x}", hash.finalize());
    if actual != sha256 {
        return Err(MlError::Download(format!("SHA-256 mismatch: expected {sha256}, got {actual}")));
    }
    Ok(())
}

/// Stream `reader` into `path` through a `.part` file, verify it and publish it. Nothing is left
/// behind on failure or cancel, and an existing `path` is only replaced by a verified file.
pub(crate) fn install_from(
    mut reader: impl Read,
    path: &Path,
    size: u64,
    sha256: &str,
    progress: &dyn Fn(u64, u64),
    cancel: Cancel<'_>,
) -> Result<(), MlError> {
    let part = part_path(path);
    // Exclusive creation keeps two downloads of one model from corrupting each other.
    let mut file = OpenOptions::new().read(true).write(true).create_new(true).open(&part).map_err(|e| {
        let hint = if e.kind() == std::io::ErrorKind::AlreadyExists { " (another download is running, or remove the stale .part file)" } else { "" };
        MlError::Download(format!("create {}: {e}{hint}", part.display()))
    })?;
    let result = (|| {
        let mut buf = vec![0u8; 1 << 16];
        let mut done = 0u64;
        progress(0, size);
        loop {
            if cancel() {
                return Err(MlError::Cancelled);
            }
            let n = reader.read(&mut buf).map_err(|e| MlError::Download(e.to_string()))?;
            if n == 0 {
                break;
            }
            done = done.saturating_add(n as u64);
            // Bound disk use even if a server ignores the published size.
            if done > size {
                return Err(MlError::Download(format!("the download is larger than the published {size} bytes")));
            }
            file.write_all(buf.get(..n).unwrap_or_default()).map_err(|e| MlError::Download(e.to_string()))?;
            progress(done, size);
        }
        file.sync_all().map_err(|e| MlError::Download(e.to_string()))?;
        verify(&mut file, size, sha256)?;
        if cancel() {
            return Err(MlError::Cancelled);
        }
        drop(file);
        std::fs::rename(&part, path).map_err(|e| MlError::Download(format!("install {}: {e}", path.display())))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> std::io::Result<Self> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("photocraft-ml-install-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn verifies_size_and_hash() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let path = t.0.join("m.onnx");
        std::fs::write(&path, b"abc")?;
        let mut f = File::open(&path)?;
        verify(&mut f, 3, ABC_SHA256)?;
        assert!(verify(&mut f, 4, ABC_SHA256).is_err());
        assert!(verify(&mut f, 3, &"0".repeat(64)).is_err());
        Ok(())
    }

    #[test]
    fn publishes_only_verified_downloads_and_reports_progress() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let path = t.0.join("m.onnx");
        std::fs::write(&path, b"old")?;
        for bytes in [&b"ab"[..], &b"abcd"[..], &b"bad"[..]] {
            assert!(install_from(bytes, &path, 3, ABC_SHA256, &|_, _| {}, &never).is_err());
            assert_eq!(std::fs::read(&path)?, b"old");
            assert!(!part_path(&path).exists());
        }
        let last = Cell::new((0, 0));
        install_from(&b"abc"[..], &path, 3, ABC_SHA256, &|d, t| last.set((d, t)), &never)?;
        assert_eq!(std::fs::read(&path)?, b"abc");
        assert_eq!(last.get(), (3, 3));
        assert!(!part_path(&path).exists());
        Ok(())
    }

    #[test]
    fn cancel_leaves_nothing_behind() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let path = t.0.join("m.onnx");
        assert_eq!(install_from(&b"abc"[..], &path, 3, ABC_SHA256, &|_, _| {}, &|| true), Err(MlError::Cancelled));
        assert!(!path.exists());
        assert!(!part_path(&path).exists());
        Ok(())
    }

    #[test]
    fn refuses_a_running_download_of_the_same_model() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let path = t.0.join("m.onnx");
        std::fs::write(part_path(&path), b"other")?;
        assert!(install_from(&b"abc"[..], &path, 3, ABC_SHA256, &|_, _| {}, &never).is_err());
        assert_eq!(std::fs::read(part_path(&path))?, b"other");
        Ok(())
    }

    /// A lock held on a folder blocks a removal and a second holder; other folders are independent.
    #[test]
    fn a_held_model_lock_blocks_removal_and_a_second_holder() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let model = Model::Lama;
        let files = model.files();
        let path = t.0.join(model.file_name());
        std::fs::write(&path, b"x")?;
        let held = lock_model(&t.0)?;
        assert!(lock_model(&t.0).is_err());
        assert!(remove_in(&t.0, files).is_err());
        assert!(path.exists(), "removal must wait for the download");
        let other = Temp::new()?;
        drop(lock_model(&other.0)?);
        drop(held);
        remove_in(&t.0, files)?;
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn remove_deletes_every_file_and_a_partial_download() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let files = [ModelFile { name: "a.bin", url: "", sha256: ABC_SHA256, size: 3 }, ModelFile { name: "b.json", url: "", sha256: ABC_SHA256, size: 3 }];
        let (a, b) = (t.0.join("a.bin"), t.0.join("b.json"));
        std::fs::write(&a, b"x")?;
        std::fs::write(&b, b"x")?;
        std::fs::write(part_path(&b), b"y")?;
        let keep = t.0.join("other.txt");
        std::fs::write(&keep, b"z")?;
        remove_in(&t.0, &files)?;
        assert!(!a.exists() && !b.exists() && !part_path(&b).exists());
        assert!(keep.exists(), "only the model's own files go");
        // Not installed: still fine.
        remove_in(&t.0, &files)?;
        Ok(())
    }

    #[test]
    fn a_model_with_several_files_installs_each_file_and_continues_after_a_stop() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let files = [
            ModelFile { name: "one.bin", url: "", sha256: ABC_SHA256, size: 3 },
            ModelFile { name: "two.bin", url: "", sha256: ABC_SHA256, size: 3 },
            ModelFile { name: "three.bin", url: "", sha256: ABC_SHA256, size: 3 },
        ];
        let fetched = std::cell::RefCell::new(Vec::new());
        let last = Cell::new((0, 0));
        let fetch = |f: &ModelFile| -> Result<Box<dyn Read>, MlError> {
            fetched.borrow_mut().push(f.name);
            // The second file comes back corrupt on the first attempt.
            let bytes: &'static [u8] = if f.name == "two.bin" && fetched.borrow().len() == 2 { b"bad" } else { b"abc" };
            Ok(Box::new(bytes))
        };
        let first = install_all(&t.0, &files, &fetch, &|d, tot| last.set((d, tot)), &never);
        assert!(first.is_err(), "the corrupt second file stops the install");
        assert!(t.0.join("one.bin").exists() && !t.0.join("two.bin").exists() && !t.0.join("three.bin").exists());
        assert_eq!(*fetched.borrow(), ["one.bin", "two.bin"]);
        // Retry: the verified first file is kept, only the missing two are fetched.
        fetched.borrow_mut().clear();
        install_all(&t.0, &files, &fetch, &|d, tot| last.set((d, tot)), &never)?;
        assert_eq!(*fetched.borrow(), ["two.bin", "three.bin"]);
        assert_eq!(last.get(), (9, 9), "progress counts all the files together");
        assert!(files.iter().all(|f| t.0.join(f.name).exists()));
        assert!(crate::store::installed_in(&t.0, &files));
        // Everything is there: nothing is fetched.
        fetched.borrow_mut().clear();
        install_all(&t.0, &files, &fetch, &|_, _| {}, &never)?;
        assert!(fetched.borrow().is_empty());
        Ok(())
    }

    #[test]
    fn a_stale_partial_download_does_not_block_a_retry() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let path = t.0.join("m.onnx");
        // Left by a download that was killed before it could clean up.
        std::fs::write(part_path(&path), b"stale")?;
        clear_stale_part(&path)?;
        install_from(&b"abc"[..], &path, 3, ABC_SHA256, &|_, _| {}, &never)?;
        assert_eq!(std::fs::read(&path)?, b"abc");
        // Nothing to clear is fine.
        clear_stale_part(&path)?;
        Ok(())
    }
}
