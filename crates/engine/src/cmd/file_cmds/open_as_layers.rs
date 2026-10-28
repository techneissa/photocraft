//! File › Open as Layers (`file.openAsLayers`): several image files become the layers of one new,
//! unsaved document, the first file on top. This is how LightCraft hands PhotoCraft the photos of
//! a double or multiple exposure (a `.pcstack` manifest, see the UI's `open_as_layers`).
//!
//! The canvas is the largest width × the largest height of the files, each smaller image centred
//! on it, one layer per file named after the file without its extension. The colour mode, bit
//! depth, resolution and profile are the top file's; the others are converted to it. A file that
//! is missing or can't be read is skipped and listed in the result; with none left the command
//! fails and no document is made. Reading and decoding run on a worker when the command is
//! started as a job, one file at a time, so the files are never all in memory as documents.

use std::sync::Arc;

use photocraft_algo::resample::translate_surface;
use photocraft_cms::{Intent, Profile};
use photocraft_doc::Size;

use super::open::import_at_raw_ppi;
use super::paths::{file_name, is_file, read_file, stem};
use super::{Document, EngineError, Layer, LayerContent, Result, Session, Value, flattened_checked, json};
use crate::color_cmds::{convert_document, document_profile, mode_space};
use crate::commands::CommandSpec;
use crate::jobs::JobCtx;

const CMD: &str = "file.openAsLayers";
/// The most files one document takes.
pub(crate) const MAX_FILES: usize = 256;
/// The most pixels all the layers may hold together (about 9 photos of 60 MP).
const MAX_LAYER_PIXELS: u64 = 1 << 29;
/// The longest document name kept.
const MAX_NAME_CHARS: usize = 200;

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: CMD.into(), msg: msg.into() }
}

/// `{"paths": [...], "name": "..."}` checked: 1 to [`MAX_FILES`] paths, and the document's name.
fn parse(p: &Value) -> Result<(Vec<String>, String)> {
    let list = p.get("paths").and_then(Value::as_array).ok_or_else(|| bad("missing \"paths\" (a list of file paths, top layer first)"))?;
    if list.is_empty() {
        return Err(bad("no files to open"));
    }
    if list.len() > MAX_FILES {
        return Err(bad(format!("{} files is too many (the most is {MAX_FILES})", list.len())));
    }
    let paths = list
        .iter()
        .map(|v| v.as_str().filter(|s| !s.is_empty()).map(str::to_string).ok_or_else(|| bad("every path must be a non-empty string")))
        .collect::<Result<Vec<_>>>()?;
    let name = match p.get("name") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_str().ok_or_else(|| bad("\"name\" must be a string"))?.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME_CHARS).collect::<String>(),
        ),
    };
    let first = paths.first().map(|p| stem(p)).unwrap_or_default();
    let name = name.filter(|n| !n.is_empty()).unwrap_or(first);
    Ok((paths, name))
}

/// The error text without the leading `path: `, which the skipped entry carries itself.
fn reason(path: &str, e: &EngineError) -> String {
    let text = e.to_string();
    text.strip_prefix(&format!("{path}: ")).map(str::to_string).unwrap_or(text)
}

/// `d` in the top file's profile and mode: converted through the colour engine unless it already
/// has that profile (then its numbers stay as they are).
fn to_profile(mut d: Document, top: &Profile, intent: Intent, bpc: bool) -> Result<Document> {
    let own = document_profile(&d);
    if mode_space(d.mode) == top.color_space && own.to_bytes() == top.to_bytes() {
        return Ok(d);
    }
    convert_document(&mut d, top, intent, bpc)?;
    Ok(d)
}

fn skipped(path: &str, why: impl Into<String>) -> Value {
    json!({"path": path, "error": why.into()})
}

/// The new document, and the files left out (`{path, error}`).
struct Opened {
    doc: Document,
    skipped: Vec<Value>,
}

/// Read, decode and flatten the files one at a time (the top file first), then lay them out.
fn build(ctx: &JobCtx, paths: &[String], name: &str, raw_ppi: f64, intent: Intent, bpc: bool) -> Result<Opened> {
    let mut skips = Vec::new();
    // The top file's colour (mode, depth, profile) and its pixels, then the other files' pixels
    // in its format, each with its size.
    let mut head: Option<Document> = None;
    // The top file's profile (its own, else the working one of its mode): what the others convert to.
    let mut target: Option<Arc<Profile>> = None;
    let mut layers: Vec<(String, photocraft_raster::Surface, Size)> = Vec::new();
    let mut pixels = 0u64;
    for (i, path) in paths.iter().enumerate() {
        ctx.check()?;
        ctx.progress(i as f32 / paths.len() as f32, &format!("Reading {}", file_name(path)));
        if !is_file(path) {
            skips.push(skipped(path, "not found"));
            continue;
        }
        let d = match read_file(path).and_then(|bytes| import_at_raw_ppi(raw_ppi, &file_name(path), &bytes)).and_then(|d| match &target {
            Some(top) => to_profile(d, top, intent, bpc),
            None => Ok(d),
        }) {
            Ok(d) => d,
            Err(e) => {
                skips.push(skipped(path, reason(path, &e)));
                continue;
            }
        };
        let area = u64::from(d.size.width) * u64::from(d.size.height);
        if pixels.saturating_add(area) > MAX_LAYER_PIXELS {
            skips.push(skipped(path, "not enough room for it with the other files"));
            continue;
        }
        let fmt = head.as_ref().map_or_else(|| d.pixel_format(), Document::pixel_format);
        let surface = match flattened_checked(&d, fmt, CMD) {
            Ok(s) => s,
            Err(e) => {
                skips.push(skipped(path, reason(path, &e)));
                continue;
            }
        };
        pixels = pixels.saturating_add(area);
        layers.push((stem(path), surface, d.size));
        if head.is_none() {
            // Only the top file's settings are kept; its layers are not.
            let mut shell = Document::new(name.to_string(), Size::new(1, 1), d.mode, d.depth);
            shell.resolution_dpi = d.resolution_dpi;
            shell.icc_profile = d.icc_profile.clone();
            target = Some(document_profile(&d));
            head = Some(shell);
        }
    }
    let Some(head) = head else {
        let why =
            skips.iter().take(3).filter_map(|s| Some(format!("{} ({})", file_name(s["path"].as_str()?), s["error"].as_str()?))).collect::<Vec<_>>().join(", ");
        return Err(EngineError::Other(format!("no file could be opened: {why}")));
    };
    let width = layers.iter().map(|l| l.2.width).max().unwrap_or(1).max(1);
    let height = layers.iter().map(|l| l.2.height).max().unwrap_or(1).max(1);
    if u64::from(width) * u64::from(height) > photocraft_compose::MAX_RENDER_PIXELS {
        return Err(bad(format!("the canvas would be {width}×{height}, which is too large")));
    }
    let mut doc = Document::new(name.to_string(), Size::new(width, height), head.mode, head.depth);
    doc.resolution_dpi = head.resolution_dpi;
    doc.icc_profile = head.icc_profile.clone();
    // The layers list runs bottom to top: the last file goes in first.
    for (layer_name, surface, size) in layers.into_iter().rev() {
        ctx.check()?;
        let dx = i32::try_from(width.saturating_sub(size.width) / 2).unwrap_or(0);
        let dy = i32::try_from(height.saturating_sub(size.height) / 2).unwrap_or(0);
        let surface = if dx == 0 && dy == 0 { surface } else { translate_surface(&surface, dx, dy) };
        doc.layers.push(Layer::new(layer_name, LayerContent::Raster(surface)));
    }
    ctx.progress(1.0, "Opening");
    Ok(Opened { doc, skipped: skips })
}

fn open_as_layers(s: &mut Session, p: &Value) -> Result<Value> {
    let (paths, name) = parse(p)?;
    let raw_ppi = s.prefs().raw_defaults.resolution;
    let (intent, bpc) = (s.color.settings.intent(), s.color.settings.bpc);
    crate::jobs::run(
        s,
        "Open as Layers",
        false,
        move |ctx| build(ctx, &paths, &name, raw_ppi, intent, bpc),
        |s, o| {
            let layers = o.doc.layers.len();
            let name = o.doc.name.clone();
            let document = s.add_document(o.doc, None);
            Ok(json!({"document": document, "name": name, "layers": layers, "skipped": o.skipped}))
        },
    )
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: CMD,
        label: "Open as Layers",
        menu: &[],
        shortcut: None,
        params: r##"{"paths":[str] (1 to 256 image files, top layer first),"name":str? (the new document's name; default: the first file's name without its extension)} → {"document": index, "name", "layers": n, "skipped": [{"path","error"}…]} (a new unsaved document: the canvas is the largest width × the largest height, each image centred, one layer per file named after it without its extension, in the colour mode, depth, resolution and profile of the top file; a file that is missing or can't be read is skipped; with none left the command fails)"##,
        enabled: super::native,
        run: open_as_layers,
        journal: false,
    }]
}

#[cfg(test)]
mod tests;
