//! Open as Layers from a `.pcstack` manifest: the small JSON file LightCraft writes next to the
//! photos of a double or multiple exposure and opens with PhotoCraft (`open -a`, or
//! `photocraft file.pcstack`). It arrives like any file the OS asks us to open
//! ([`PhotocraftApp::open_path`](crate::PhotocraftApp::open_path)); the files are then opened as
//! the layers of one new document by the engine command `file.openAsLayers`, in the background
//! when background jobs are on.
//!
//! ```json
//! {"kind": "photocraft-open-as-layers", "version": 1, "name": "DSC01234 + 2", "files": ["/abs/top.psd", "/abs/bottom.jpg"]}
//! ```
//!
//! `files` are absolute paths, top layer first (1 to 256); `name` is optional. A manifest of
//! another kind or a newer version, bad JSON or an empty list is refused with an error notice;
//! files that can't be opened are skipped and named in a notice. The manifest and the photos are
//! only read, never changed.

// The web has no files to open: only the result handling below is used there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::file_open::display_name;

/// The engine command that does the work.
pub(crate) const COMMAND: &str = "file.openAsLayers";
const KIND: &str = "photocraft-open-as-layers";
/// The newest manifest version understood.
const VERSION: u64 = 1;
const MAX_FILES: usize = 256;
/// A manifest of 256 long paths is far below this.
const MAX_MANIFEST_BYTES: u64 = 4 << 20;
/// Skipped files listed in the notice; the rest are counted.
const LISTED: usize = 5;

/// What a `.pcstack` file asks for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Manifest {
    /// The new document's name; `None` leaves it to the command (the first file's name).
    pub name: Option<String>,
    /// Image file paths, top layer first.
    pub files: Vec<String>,
}

/// Whether `path` names a manifest.
pub(crate) fn is_manifest(path: &str) -> bool {
    std::path::Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case("pcstack"))
}

/// Read a manifest's text. The error is a short reason for the notice "Couldn't open <file>: …".
pub(crate) fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let v: Value = serde_json::from_slice(bytes).map_err(|_| tl!("not an Open as Layers file").to_string())?;
    if v.get("kind").and_then(Value::as_str) != Some(KIND) {
        return Err(tl!("not an Open as Layers file").to_string());
    }
    match v.get("version").and_then(Value::as_u64) {
        Some(n) if n > VERSION => return Err(tl!("it needs a newer PhotoCraft").to_string()),
        Some(n) if n >= 1 => {}
        _ => return Err(tl!("not an Open as Layers file").to_string()),
    }
    let list = v.get("files").and_then(Value::as_array).ok_or_else(|| tl!("it lists no files").to_string())?;
    if list.is_empty() {
        return Err(tl!("it lists no files").to_string());
    }
    if list.len() > MAX_FILES {
        return Err(crate::i18n::fmt(tl!("it lists more than {max} files"), &[("max", &MAX_FILES.to_string())]));
    }
    let files = list
        .iter()
        .map(|f| f.as_str().filter(|s| !s.is_empty()).map(str::to_string).ok_or_else(|| tl!("its file list is not a list of paths").to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let name = v.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    Ok(Manifest { name, files })
}

/// File › Open's hook: handle `path` when it is a manifest (`None` for any other file). The
/// manifest is read and its files are opened as layers; a failure is the `Err` the caller shows.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_path(app: &mut PhotocraftApp, path: &str) -> Option<Result<Vec<String>, String>> {
    if !is_manifest(path) {
        return None;
    }
    Some(read(path).and_then(|bytes| parse_manifest(&bytes)).and_then(|m| start(app, &m)).map(|()| Vec::new()))
}

/// The manifest's bytes, at most [`MAX_MANIFEST_BYTES`].
#[cfg(not(target_arch = "wasm32"))]
fn read(path: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(tl!("not an Open as Layers file").to_string());
    }
    Ok(bytes)
}

/// Run `file.openAsLayers` for `m`: in the background when background jobs are on (then
/// [`finished`] reports), else now.
pub(crate) fn start(app: &mut PhotocraftApp, m: &Manifest) -> Result<(), String> {
    let mut params = json!({"paths": m.files});
    if let Some(name) = &m.name {
        params["name"] = json!(name);
    }
    let v = crate::jobs_ui::run(app, COMMAND, params)?;
    if v.get("pending").and_then(Value::as_bool) == Some(true) {
        let name = m.name.clone().or_else(|| m.files.first().map(|f| display_name(f))).unwrap_or_default();
        app.ui.status = crate::i18n::fmt(tl!("Opening {name}…"), &[("name", &name)]);
        app.ui.status_error = false;
    } else {
        finished(app, &v);
    }
    Ok(())
}

/// The command's result `v` (`{name, layers, skipped}`) is in: say so, and name the files that
/// were left out.
pub(crate) fn finished(app: &mut PhotocraftApp, v: &Value) {
    app.sync_views();
    let name = v.get("name").and_then(Value::as_str).unwrap_or_default();
    let layers = v.get("layers").and_then(Value::as_u64).unwrap_or(0);
    app.ui.status = crate::i18n::fmt(&crate::i18n::trn(crate::i18n::current(), layers, "Opened {name}: {n} layer", "Opened {name}: {n} layers"), &[("name", name)]);
    app.ui.status_error = false;
    let skipped = v.get("skipped").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    if skipped.is_empty() {
        return;
    }
    let title = crate::i18n::trn(crate::i18n::current(), skipped.len() as u64, "{n} file skipped", "{n} files skipped");
    let mut lines: Vec<String> = skipped
        .iter()
        .take(LISTED)
        .map(|s| {
            let file = display_name(s.get("path").and_then(Value::as_str).unwrap_or_default());
            format!("{file}: {}", s.get("error").and_then(Value::as_str).unwrap_or_default())
        })
        .collect();
    if skipped.len() > LISTED {
        lines.push(crate::i18n::fmt(tl!("and {n} more"), &[("n", &(skipped.len() - LISTED).to_string())]));
    }
    crate::notices::post(app, title, lines, false, None);
}

#[cfg(test)]
mod tests;
