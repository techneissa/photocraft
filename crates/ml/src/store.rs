//! Where the model files live. They are never committed or embedded: a build or a user puts them in
//! a models folder (see `docs/ml.md`).
//!
//! A model is a folder of one or more files, each with its own pinned size and SHA-256. It is
//! looked for, in this order:
//!
//! 1. `$PHOTOCRAFT_MODELS_DIR`, when set (and then nowhere else): `<dir>/<model-id>/`, else the
//!    files directly in `<dir>` (the layout before the shared folder).
//! 2. The folder shared by the Craft apps, so a model is never downloaded twice:
//!    `~/Library/Application Support/ArtCraft/models/<model-id>/`.
//! 3. The old `PhotoCraft/models/` folder, with the files directly in it. Read only: nothing is
//!    ever written or deleted there.
//!
//! New downloads go into step 1's folder when set, else step 2's.

use std::path::{Path, PathBuf};

/// The environment variable that replaces every other models folder.
pub const MODELS_DIR_VAR: &str = "PHOTOCRAFT_MODELS_DIR";

/// One file of a model, pinned by size and SHA-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
    /// File name inside the model's folder.
    pub name: &'static str,
    /// Published download.
    pub url: &'static str,
    /// SHA-256 of the published file, measured from the installed copy with `shasum -a 256`.
    pub sha256: &'static str,
    /// Size of the published file, to catch a truncated download.
    pub size: u64,
}

const BIREFNET_FILES: [ModelFile; 1] = [ModelFile {
    name: "birefnet_lite.onnx",
    url: "https://huggingface.co/onnx-community/BiRefNet_lite-ONNX/resolve/main/onnx/model.onnx",
    sha256: "5600024376f572a557870a5eb0afb1e5961636bef4e1e22132025467d0f03333",
    size: 224_005_088,
}];

const LAMA_FILES: [ModelFile; 1] = [ModelFile {
    name: "lama_fp32.onnx",
    url: "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx",
    sha256: "1faef5301d78db7dda502fe59966957ec4b79dd64e16f03ed96913c7a4eb68d6",
    size: 208_044_816,
}];

/// SAM 3 (Segment Anything with Concepts, Meta): the `facebook/sam3` checkpoint in Hugging Face format. No download is
/// public (the Hugging Face repository is gated: each user accepts Meta's SAM License there), so `url` is empty and the
/// files are put into the model's folder by hand. `model.safetensors` is Hugging Face's own LFS hash; the two tokenizer
/// files are pinned from the copies the Craft apps share.
const SAM3_FILES: [ModelFile; 3] = [
    ModelFile { name: "model.safetensors", url: "", sha256: "6d06f0a5f84e435071fe6603e61d0b4cc7b40e0d39d487cfd4d67d8cc11cc14a", size: 3_439_938_512 },
    ModelFile { name: "vocab.json", url: "", sha256: "5047b556ce86ccaf6aa22b3ffccfc52d391ea4accdab9c2f2407da5b742d4363", size: 862_328 },
    ModelFile { name: "merges.txt", url: "", sha256: "9fd691f7c8039210e0fced15865466c65820d09b63988b0174bfe25de299051a", size: 524_619 },
];

/// A model the editor can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    /// BiRefNet-lite, 1024 px salient-object segmentation (MIT).
    BirefnetLite,
    /// big-lama, 512 px inpainting (Apache-2.0).
    Lama,
    /// SAM 3, click and text prompts for object selection (SAM License; no public download).
    Sam3,
}

impl Model {
    /// All supported models, in display/download order.
    pub const ALL: [Self; 3] = [Self::BirefnetLite, Self::Lama, Self::Sam3];

    /// Folder name inside a models folder, shared by every Craft app.
    pub fn id(self) -> &'static str {
        match self {
            Self::BirefnetLite => "birefnet-lite",
            Self::Lama => "lama",
            Self::Sam3 => "sam3",
        }
    }

    /// Every file of the model, the one the loader reads first.
    pub fn files(self) -> &'static [ModelFile] {
        match self {
            Self::BirefnetLite => &BIREFNET_FILES,
            Self::Lama => &LAMA_FILES,
            Self::Sam3 => &SAM3_FILES,
        }
    }

    /// Whether PhotoCraft can download the model: every file has a published URL. A model that
    /// cannot be (SAM 3) is put into its folder by hand, and PhotoCraft never deletes it.
    pub fn downloadable(self) -> bool {
        self.files().iter().all(|f| !f.url.is_empty())
    }

    /// Licence of the model weights.
    pub fn licence(self) -> &'static str {
        match self {
            Self::BirefnetLite => "MIT",
            Self::Lama => "Apache-2.0",
            Self::Sam3 => "SAM License",
        }
    }

    /// Name of the first (main) file.
    pub fn file_name(self) -> &'static str {
        self.files().first().map_or("", |f| f.name)
    }

    /// Human name for messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::BirefnetLite => "subject segmentation (BiRefNet-lite)",
            Self::Lama => "inpainting (LaMa)",
            Self::Sam3 => "object selection (SAM 3)",
        }
    }

    /// Size of all the published files together.
    pub fn size_bytes(self) -> u64 {
        self.files().iter().map(|f| f.size).sum()
    }
}

/// Which models folder a model was found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// The folder `PHOTOCRAFT_MODELS_DIR` names.
    Custom,
    /// The folder shared by the Craft apps (`ArtCraft/models`).
    Shared,
    /// The old `PhotoCraft/models` folder (read only).
    Old,
}

impl Source {
    /// Short English name, for messages and the CLI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Custom => "custom folder",
            Self::Shared => "shared folder",
            Self::Old => "old PhotoCraft folder",
        }
    }
}

/// An installed model: the folder holding all its files, and which models folder that is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub dir: PathBuf,
    pub source: Source,
}

/// The models folders of this computer, kept apart from the environment so tests can build them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Roots {
    custom: Option<PathBuf>,
    shared: Option<PathBuf>,
    old: Option<PathBuf>,
}

impl Roots {
    /// `PHOTOCRAFT_MODELS_DIR`, the shared `ArtCraft/models` and the old `PhotoCraft/models` of
    /// this user. Empty in the browser.
    fn current() -> Self {
        if cfg!(target_arch = "wasm32") {
            return Self::default();
        }
        let custom = std::env::var_os(MODELS_DIR_VAR).filter(|d| !d.is_empty()).map(PathBuf::from);
        Self { custom, shared: user_models_dir("ArtCraft"), old: user_models_dir("PhotoCraft") }
    }

    /// The folder new downloads go into.
    fn install_root(&self) -> Option<&Path> {
        self.custom.as_deref().or(self.shared.as_deref())
    }

    /// Where to look, first match wins: (model folder, which folder it is in).
    fn candidates(&self, id: &str) -> Vec<(PathBuf, Source)> {
        if let Some(custom) = &self.custom {
            return vec![(custom.join(id), Source::Custom), (custom.clone(), Source::Custom)];
        }
        let shared = self.shared.iter().map(|d| (d.join(id), Source::Shared));
        let old = self.old.iter().map(|d| (d.clone(), Source::Old));
        shared.chain(old).collect()
    }

    pub(crate) fn locate(&self, id: &str, files: &[ModelFile]) -> Option<Located> {
        self.candidates(id).into_iter().find(|(dir, _)| installed_in(dir, files)).map(|(dir, source)| Located { dir, source })
    }

    pub(crate) fn install_dir(&self, id: &str) -> Option<PathBuf> {
        self.install_root().map(|d| d.join(id))
    }
}

/// `<app data folder>/<app>/models`: `~/Library/Application Support` on macOS, `$XDG_DATA_HOME`
/// (else `~/.local/share`) on Linux with the app name in lower case, `%APPDATA%` on Windows.
fn user_models_dir(app: &str) -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library").join("Application Support").join(app).join("models"))
    } else if cfg!(windows) {
        var("APPDATA").map(|a| a.join(app).join("models"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("share"))).map(|d| d.join(app.to_lowercase()).join("models"))
    }
}

/// `<app data folder>/<app>`: the folder above [`user_models_dir`]'s.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn app_data_dir(app: &str) -> Option<PathBuf> {
    user_models_dir(app)?.parent().map(Path::to_path_buf)
}

/// The models folder new downloads go into: `PHOTOCRAFT_MODELS_DIR`, else the shared folder
/// (`~/Library/Application Support/ArtCraft/models`). Models are in its `<model-id>/` folders.
/// `None` when no home folder is known (or in the browser).
pub fn models_dir() -> Option<PathBuf> {
    Roots::current().install_root().map(Path::to_path_buf)
}

/// The folder a download of `model` goes into (it may not exist yet).
pub fn install_dir(model: Model) -> Option<PathBuf> {
    Roots::current().install_dir(model.id())
}

static MODELS_DISABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Treat every model as missing in this process from now on (tests, fuzzing), so results never
/// depend on what a developer has installed. `PHOTOCRAFT_NO_MODELS` does the same for a shell.
pub fn disable_models() {
    MODELS_DISABLED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Whether models are switched off by [`disable_models`] or `PHOTOCRAFT_NO_MODELS`.
pub fn models_disabled() -> bool {
    MODELS_DISABLED.load(std::sync::atomic::Ordering::Relaxed) || std::env::var_os("PHOTOCRAFT_NO_MODELS").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Where `model` is installed, with every file of it there at the published size and SHA-256. A
/// partial or corrupt file counts as missing rather than being handed to the loader. `None`
/// while [`models_disabled`].
pub fn model_location(model: Model) -> Option<Located> {
    if models_disabled() {
        return None;
    }
    Roots::current().locate(model.id(), model.files())
}

/// Whether `model` seems to be installed: every file of it is in a models folder at the published
/// size. Looks at sizes only, so it answers at once even for SAM 3's 3.4 GB (the SHA-256 check of
/// [`model_location`] takes seconds the first time); the model's loader still goes through
/// [`model_location`]. `false` while [`models_disabled`].
pub fn model_present(model: Model) -> bool {
    if models_disabled() {
        return false;
    }
    let files = model.files();
    !files.is_empty()
        && Roots::current()
            .candidates(model.id())
            .into_iter()
            .any(|(dir, _)| files.iter().all(|f| std::fs::metadata(dir.join(f.name)).is_ok_and(|m| m.len() == f.size)))
}

/// The first file of the installed `model`, if it is installed (see [`model_location`]).
pub fn model_path(model: Model) -> Option<PathBuf> {
    model_location(model).map(|l| l.dir.join(model.file_name()))
}

/// Whether every file of `files` is in `dir` with its published size and SHA-256.
pub(crate) fn installed_in(dir: &Path, files: &[ModelFile]) -> bool {
    !files.is_empty()
        && files.iter().all(|f| {
            let p = dir.join(f.name);
            std::fs::metadata(&p).is_ok_and(|meta| meta.len() == f.size && hash_ok(&p, f, &meta))
        })
}

/// Whether the file at `p` has `file`'s SHA-256. The hash of a ~200 MB file is computed once
/// per file version, not on every lookup. The version includes the inode and the change time
/// on Unix, which every write, copy-over or rename moves and which a user can't set back, so a
/// replaced file is always hashed again even when its size and modification time match.
#[cfg(not(target_arch = "wasm32"))]
fn hash_ok(p: &Path, file: &ModelFile, meta: &std::fs::Metadata) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Version = (u64, Option<std::time::SystemTime>, u64, i64, i64);
    static SEEN: OnceLock<Mutex<HashMap<PathBuf, (Version, bool)>>> = OnceLock::new();
    #[cfg(unix)]
    let (ino, ctime, ctime_ns) = {
        use std::os::unix::fs::MetadataExt;
        (meta.ino(), meta.ctime(), meta.ctime_nsec())
    };
    // Windows: no inode or change time; the creation time at least moves when a file is
    // deleted and written anew. Loading re-checks the hash of the bytes it uses either way.
    #[cfg(windows)]
    let (ino, ctime, ctime_ns) = {
        use std::os::windows::fs::MetadataExt;
        (0, i64::try_from(meta.creation_time()).unwrap_or(i64::MAX), 0)
    };
    #[cfg(not(any(unix, windows)))]
    let (ino, ctime, ctime_ns) = (0, 0, 0);
    let version: Version = (meta.len(), meta.modified().ok(), ino, ctime, ctime_ns);
    let seen = SEEN.get_or_init(Default::default);
    if let Ok(map) = seen.lock()
        && let Some(&(v, ok)) = map.get(p)
        && v == version
    {
        return ok;
    }
    // A big file that passed the check in an earlier process is not hashed again while nothing
    // about it has changed (`marker.rs`).
    let big = meta.len() >= crate::marker::MIN_SIZE;
    let marker = big.then(crate::marker::marker_path).flatten();
    let stamp = marker.is_some().then(|| crate::marker::Stamp::of(p, meta, file.sha256)).flatten();
    let remembered = matches!((&marker, &stamp), (Some(m), Some(s)) if crate::marker::recorded(m, s));
    let ok = remembered || std::fs::File::open(p).is_ok_and(|mut f| crate::install::verify(&mut f, file.size, file.sha256).is_ok());
    if ok
        && !remembered
        && let (Some(m), Some(s)) = (&marker, stamp)
    {
        crate::marker::record(m, s);
    }
    if let Ok(mut map) = seen.lock() {
        map.insert(p.to_path_buf(), (version, ok));
    }
    ok
}

#[cfg(target_arch = "wasm32")]
fn hash_ok(_: &Path, _: &ModelFile, _: &std::fs::Metadata) -> bool {
    false
}

/// Where `model` is expected, installed or not (for messages).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn expected_path(model: Model) -> String {
    install_dir(model).map_or_else(|| model.file_name().to_string(), |d| d.join(model.file_name()).display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// Two small files, as a model with several files (like SAM3) would be pinned.
    const TWO: [ModelFile; 2] =
        [ModelFile { name: "a.bin", url: "", sha256: ABC_SHA256, size: 3 }, ModelFile { name: "b.json", url: "", sha256: EMPTY_SHA256, size: 0 }];

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> std::io::Result<Self> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("photocraft-ml-store-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            // Created exclusively, so a pre-placed directory or symlink can't be reused.
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
        fn dir(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Put `TWO` into `dir` (created if needed).
    fn put_two(dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("a.bin"), b"abc")?;
        std::fs::write(dir.join("b.json"), b"")
    }

    #[test]
    fn disabled_models_are_missing() {
        disable_models();
        assert!(models_disabled());
        for model in Model::ALL {
            assert_eq!(model_path(model), None);
            assert_eq!(model_location(model), None);
        }
    }

    #[test]
    fn model_ids_are_the_shared_folder_names() {
        assert_eq!(Model::BirefnetLite.id(), "birefnet-lite");
        assert_eq!(Model::Lama.id(), "lama");
        assert_eq!(Model::Sam3.id(), "sam3");
        for model in Model::ALL {
            assert!(!model.files().is_empty());
            assert_eq!(model.size_bytes(), model.files().iter().map(|f| f.size).sum::<u64>());
            assert_eq!(model.file_name(), model.files().first().map(|f| f.name).unwrap_or_default());
        }
    }

    #[test]
    fn sam3_is_three_pinned_files_with_no_download() {
        let files = Model::Sam3.files();
        assert_eq!(files.iter().map(|f| f.name).collect::<Vec<_>>(), ["model.safetensors", "vocab.json", "merges.txt"]);
        assert_eq!(Model::Sam3.file_name(), "model.safetensors");
        assert_eq!(Model::Sam3.size_bytes(), 3_439_938_512 + 862_328 + 524_619);
        assert!(files.iter().all(|f| f.sha256.len() == 64 && f.sha256.bytes().all(|b| b.is_ascii_hexdigit())));
        assert!(!Model::Sam3.downloadable(), "the Hugging Face repository is gated");
        assert!(Model::BirefnetLite.downloadable() && Model::Lama.downloadable());
    }

    #[test]
    fn a_same_size_corrupt_file_is_not_installed() -> Result<(), Box<dyn std::error::Error>> {
        let dir = Temp::new()?;
        let model = Model::Lama;
        let file = model.files().first().ok_or("no file")?;
        // Right size, wrong bytes (sparse zeros).
        std::fs::File::create_new(dir.0.join(file.name))?.set_len(file.size)?;
        assert!(!installed_in(&dir.0, model.files()));
        Ok(())
    }

    #[test]
    fn every_file_must_be_there_and_right() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let dir = t.dir("m");
        assert!(!installed_in(&dir, &TWO), "no folder");
        put_two(&dir)?;
        assert!(installed_in(&dir, &TWO));
        // One file missing.
        std::fs::remove_file(dir.join("b.json"))?;
        assert!(!installed_in(&dir, &TWO));
        // One file the right size with the wrong bytes.
        put_two(&dir)?;
        std::fs::write(dir.join("a.bin"), b"abd")?;
        assert!(!installed_in(&dir, &TWO));
        // One file of the wrong size.
        put_two(&dir)?;
        std::fs::write(dir.join("b.json"), b"{}")?;
        assert!(!installed_in(&dir, &TWO));
        assert!(!installed_in(&dir, &[]), "a model without files is never installed");
        Ok(())
    }

    fn roots(t: &Temp, custom: bool) -> Roots {
        Roots { custom: custom.then(|| t.dir("custom")), shared: Some(t.dir("shared")), old: Some(t.dir("old")) }
    }

    #[test]
    fn lookup_goes_shared_then_old_and_reports_where() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let r = roots(&t, false);
        assert_eq!(r.locate("m", &TWO), None);
        // Only the old folder (files directly in it): found there.
        put_two(&t.dir("old"))?;
        assert_eq!(r.locate("m", &TWO), Some(Located { dir: t.dir("old"), source: Source::Old }));
        // The shared folder wins once the model is in it.
        put_two(&t.dir("shared").join("m"))?;
        assert_eq!(r.locate("m", &TWO), Some(Located { dir: t.dir("shared").join("m"), source: Source::Shared }));
        // A broken shared copy does not hide the good old one.
        std::fs::write(t.dir("shared").join("m").join("a.bin"), b"bad")?;
        assert_eq!(r.locate("m", &TWO).map(|l| l.source), Some(Source::Old));
        // The shared folder is looked up per model id.
        assert_eq!(r.locate("other", &TWO).map(|l| l.source), Some(Source::Old));
        Ok(())
    }

    #[test]
    fn the_environment_folder_replaces_the_others() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        let r = roots(&t, true);
        put_two(&t.dir("shared").join("m"))?;
        put_two(&t.dir("old"))?;
        // Set but empty: the shared and old copies are not used.
        assert_eq!(r.locate("m", &TWO), None);
        // Flat files in it (the layout before the shared folder) are found...
        put_two(&t.dir("custom"))?;
        assert_eq!(r.locate("m", &TWO), Some(Located { dir: t.dir("custom"), source: Source::Custom }));
        // ... and the per-model folder wins over them.
        put_two(&t.dir("custom").join("m"))?;
        assert_eq!(r.locate("m", &TWO), Some(Located { dir: t.dir("custom").join("m"), source: Source::Custom }));
        Ok(())
    }

    #[test]
    fn downloads_go_to_the_environment_folder_else_the_shared_one() -> Result<(), Box<dyn std::error::Error>> {
        let t = Temp::new()?;
        assert_eq!(roots(&t, false).install_dir("m"), Some(t.dir("shared").join("m")));
        assert_eq!(roots(&t, true).install_dir("m"), Some(t.dir("custom").join("m")));
        let none = Roots { custom: None, shared: None, old: Some(t.dir("old")) };
        assert_eq!(none.install_dir("m"), None, "the old folder is never written to");
        Ok(())
    }
}
