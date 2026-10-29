//! Undo / redo stress test: seeded random edits, undos and redos on synthetic documents.
//!
//! Every step is checked against a model of the history:
//! 1. nothing panics (a panic anywhere in the process counts, even one `Session::execute`
//!    contains) and nothing hangs (a watchdog fails a step stuck for [`HANG`]; steps slower than
//!    the budget are only reported);
//! 2. Undo gives exactly the document from before the edit it undoes;
//! 3. Redo gives exactly the document after that edit;
//! 4. Undo all gives the original document, Redo all the final one;
//! 5. a command that fails leaves the document and the history as they were.
//!
//! "Exactly" is a [`Digest`]: the document with every pixel surface taken out, formatted with
//! `Debug` (so layers, masks, blend modes, opacity, selection state, guides, paths and every
//! other field count, and a field added later counts too), plus a BLAKE3 hash of each surface's
//! format, default pixel and tile bytes, named by where it sits.
//!
//! The default tests run [`SHORT_STEPS`] steps per configuration with fixed seeds. The long one
//! is opt-in: `cargo test -p photocraft-engine --test undo_redo_stress -- --ignored --nocapture`.
//!
//! Environment (for a longer local run or to replay a failure):
//! - `PHOTOCRAFT_UNDO_STRESS_SEED=<n>`: run only this seed;
//! - `PHOTOCRAFT_UNDO_STRESS_STEPS=<n>`: steps per configuration;
//! - `PHOTOCRAFT_UNDO_STRESS_CONFIG=<depth>-<mode>`: only that configuration (`8-rgb`, `32-rgb`...);
//! - `PHOTOCRAFT_UNDO_STRESS_STEP_SECS=<n>`: the per-step time budget to report against (5);
//! - `PHOTOCRAFT_UNDO_STRESS_HANG_SECS=<n>`: a step stuck this long is a hang (60);
//! - `PHOTOCRAFT_UNDO_STRESS_VERBOSE=1`: print the per-command table of every run.
use photocraft_color::PixelFormat;
use photocraft_doc::{Document, Layer, LayerContent, LayerId, Surface};
use photocraft_engine::Session;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once, PoisonError, mpsc};
use std::time::{Duration, Instant};

/// Steps per configuration of the default tests (a few seconds in all).
const SHORT_STEPS: usize = 200;
/// Steps per configuration and seed of the ignored test.
const LONG_STEPS: usize = 2000;
const SHORT_SEEDS: [u64; 1] = [0x5eed_0001];
const LONG_SEEDS: [u64; 3] = [0x5eed_0001, 0x5eed_0002, 0x5eed_0003];
/// Steps between a full Undo-all / Redo-all check (which then forgets the history, so a long run
/// holds a bounded number of document copies).
const SEGMENT: usize = 400;
const WIDTH: i32 = 256;
const HEIGHT: i32 = 192;
/// Canvas-changing commands keep the sides inside this range, so a run can't grow without bound.
const MIN_SIDE: i32 = 96;
const MAX_SIDE: i32 = 320;
/// More layers than this and the generators stop adding layers.
const MAX_LAYERS: usize = 14;

/// The document configurations: (bit depth, `file.new` mode).
const CONFIGS: [(u32, &str); 5] = [(8, "rgb"), (16, "rgb"), (32, "rgb"), (8, "gray"), (8, "cmyk")];

// ---- environment -------------------------------------------------------------------------------

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.trim().parse().ok())
}

fn secs(name: &str, default: u64) -> Duration {
    Duration::from_secs(env_u64(name).unwrap_or(default))
}

fn seeds(default: &[u64]) -> Vec<u64> {
    env_u64("PHOTOCRAFT_UNDO_STRESS_SEED").map_or_else(|| default.to_vec(), |s| vec![s])
}

fn steps(default: usize) -> usize {
    env_u64("PHOTOCRAFT_UNDO_STRESS_STEPS").map_or(default, |n| n as usize)
}

fn configs() -> Vec<(u32, &'static str)> {
    let only = std::env::var("PHOTOCRAFT_UNDO_STRESS_CONFIG").ok();
    CONFIGS.iter().copied().filter(|(d, m)| only.as_deref().is_none_or(|o| o == format!("{d}-{m}"))).collect()
}

// ---- random numbers ----------------------------------------------------------------------------

/// SplitMix64: small, fast, and the same sequence on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// 0..n (n > 0).
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    /// lo..=hi.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo { lo } else { lo + self.below((hi - lo + 1) as u64) as i32 }
    }
    /// 0..1.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.unit() * (hi - lo)
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
    fn color(&mut self) -> String {
        format!("#{:06x}", self.below(1 << 24))
    }
}

// ---- the digest --------------------------------------------------------------------------------

/// A stable fingerprint of a document, split so a mismatch names what differs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Digest {
    /// Hash of the document with its surfaces taken out (`Debug` of every other field).
    structure: u64,
    /// Where each surface sits (`layer 3 pixels`, `selection`...) and the hash of its tiles.
    surfaces: Vec<(String, u64)>,
}

impl Digest {
    /// What differs between `self` (expected) and `other`, for a failure message.
    fn diff(&self, other: &Digest) -> String {
        let mut out = Vec::new();
        if self.structure != other.structure {
            out.push("document fields (layers, blend/opacity, masks' flags, selection state, guides, paths...)".to_string());
        }
        let a: BTreeMap<&str, u64> = self.surfaces.iter().map(|(n, h)| (n.as_str(), *h)).collect();
        let b: BTreeMap<&str, u64> = other.surfaces.iter().map(|(n, h)| (n.as_str(), *h)).collect();
        for (name, h) in &a {
            match b.get(name) {
                None => out.push(format!("surface `{name}` is missing")),
                Some(g) if g != h => out.push(format!("surface `{name}` has other pixels")),
                _ => {}
            }
        }
        for name in b.keys().filter(|n| !a.contains_key(*n)) {
            out.push(format!("surface `{name}` is extra"));
        }
        out.join("; ")
    }
}

fn hash_u64(h: &blake3::Hasher) -> u64 {
    let bytes = h.finalize();
    u64::from_le_bytes(bytes.as_bytes()[..8].try_into().unwrap_or([0; 8]))
}

/// Hashes what is formatted into it, without building the string.
struct HashSink(blake3::Hasher);

impl std::fmt::Write for HashSink {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.update(s.as_bytes());
        Ok(())
    }
}

fn surface_hash(s: &Surface) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(format!("{:?}", s.format()).as_bytes());
    h.update(s.default_bytes());
    for (c, t) in s.tiles() {
        h.update(format!("{c:?}").as_bytes());
        h.update(t.bytes());
    }
    hash_u64(&h)
}

/// Move a surface out into `out` (named `name`), leaving a one-pixel stand-in.
fn take(s: &mut Surface, name: String, out: &mut Vec<(String, u64)>) {
    out.push((name, surface_hash(s)));
    *s = Surface::new(PixelFormat::GRAY8);
}

fn strip_layers(layers: &mut [Layer], out: &mut Vec<(String, u64)>) {
    for l in layers {
        let id = l.id.0;
        if let Some(m) = &mut l.mask {
            take(&mut m.surface, format!("layer {id} mask"), out);
        }
        if let Some(f) = &mut l.fill_cache {
            take(&mut f.surface, format!("layer {id} fill cache"), out);
        }
        match &mut l.content {
            LayerContent::Raster(s) => take(s, format!("layer {id} pixels"), out),
            LayerContent::Group(g) => strip_layers(&mut g.children, out),
            LayerContent::Shape(sh) => {
                if let Some(c) = &mut sh.cache {
                    take(c, format!("layer {id} shape cache"), out);
                }
            }
            LayerContent::Smart(so) => {
                if let Some(c) = &mut so.cache {
                    take(c, format!("layer {id} smart cache"), out);
                }
                if let Some(m) = &mut so.filter_mask {
                    take(&mut m.surface, format!("layer {id} smart filter mask"), out);
                }
            }
            _ => {}
        }
    }
}

/// The document without its surfaces, and the hash of each.
fn strip(doc: &Document) -> (Document, Vec<(String, u64)>) {
    let mut d = doc.clone();
    let mut out = Vec::new();
    strip_layers(&mut d.layers, &mut out);
    if let Some(s) = &mut d.selection {
        take(s, "selection".into(), &mut out);
    }
    for (i, c) in d.channels.iter_mut().enumerate() {
        take(&mut c.surface, format!("channel {i}"), &mut out);
    }
    if let Some(q) = &mut d.quick_mask {
        take(&mut q.surface, "quick mask".into(), &mut out);
    }
    for (i, p) in d.patterns.iter_mut().enumerate() {
        take(&mut p.surface, format!("pattern {i}"), &mut out);
    }
    (d, out)
}

fn digest(doc: &Document) -> Digest {
    let (d, surfaces) = strip(doc);
    let mut sink = HashSink(blake3::Hasher::new());
    // `Debug` of the stripped document: every field, floats at full precision.
    let _ = write!(sink, "{d:?}");
    Digest { structure: hash_u64(&sink.0), surfaces }
}

/// Why two documents differ, for a failure message: the digest's view, plus the first lines where
/// the documents' fields (surfaces taken out) differ.
fn explain(want: &Document, now: &Document) -> String {
    let (dw, dn) = (digest(want), digest(now));
    let mut out = dw.diff(&dn);
    if dw.structure != dn.structure {
        let (a, b) = (format!("{:#?}", strip(want).0), format!("{:#?}", strip(now).0));
        let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        if let Some(i) = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) {
            let _ = write!(
                out,
                "\n    first differing field, line {i} of the dump:\n      expected: {}\n      found:    {}",
                a.get(i).unwrap_or(&"<end>"),
                b.get(i).unwrap_or(&"<end>")
            );
            for k in (i.saturating_sub(4)..i).rev().take(1) {
                let _ = write!(out, "\n      (after: {})", a.get(k).unwrap_or(&""));
            }
        }
    }
    out
}

// ---- the commands ------------------------------------------------------------------------------

/// What the generators know about the document.
struct Ctx {
    w: i32,
    h: i32,
    layers: Vec<LayerId>,
    /// Visible pixel layers that are not locked.
    rasters: Vec<LayerId>,
    active_is_raster: bool,
    active_has_mask: bool,
    has_selection: bool,
}

impl Ctx {
    fn of(s: &Session) -> Option<Ctx> {
        let st = s.active()?;
        let d = &st.doc;
        let walk = d.walk();
        let rasters: Vec<LayerId> = walk
            .iter()
            .filter(|(_, _, l)| matches!(l.content, LayerContent::Raster(_)) && l.visible && !l.locks.pixels && !l.locks.all)
            .map(|(_, _, l)| l.id)
            .collect();
        let active = st.active_layer.and_then(|a| d.layer(a));
        Some(Ctx {
            w: d.size.width as i32,
            h: d.size.height as i32,
            layers: walk.iter().map(|(_, _, l)| l.id).collect(),
            active_is_raster: st.active_layer.is_some_and(|a| rasters.contains(&a)),
            rasters,
            active_has_mask: active.is_some_and(|l| l.mask.is_some()),
            has_selection: d.selection.is_some(),
        })
    }
    fn point(&self, r: &mut Rng) -> [i32; 2] {
        [r.range(-8, self.w + 8), r.range(-8, self.h + 8)]
    }
    fn stroke_points(&self, r: &mut Rng) -> Vec<[i32; 2]> {
        let mut p = self.point(r);
        (0..r.range(2, 7))
            .map(|_| {
                p = [(p[0] + r.range(-40, 40)).clamp(0, self.w), (p[1] + r.range(-40, 40)).clamp(0, self.h)];
                p
            })
            .collect()
    }
    /// A rectangle inside the canvas: x, y, width, height.
    fn rect(&self, r: &mut Rng) -> [i32; 4] {
        let (x, y) = (r.range(0, self.w - 24), r.range(0, self.h - 24));
        [x, y, r.range(12, (self.w - x).max(12)), r.range(12, (self.h - y).max(12))]
    }
    /// 1 in 4: name an existing layer instead of using the active one.
    fn layer(&self, mut v: Value, r: &mut Rng) -> Value {
        if r.chance(25)
            && !self.layers.is_empty()
            && let Some(o) = v.as_object_mut()
        {
            o.insert("layer".into(), json!(r.pick(&self.layers).0));
        }
        v
    }
}

type Gen = fn(&mut Rng, &Ctx) -> Value;

/// What a command needs before it can run; the runner sets it up first (most of the time), as a
/// user would, so the commands are not mostly rejected.
#[derive(Clone, Copy, PartialEq)]
enum Need {
    Nothing,
    /// A pixel layer is the active layer.
    PixelLayer,
    /// A selection.
    Selection,
    /// The active layer has a pixel mask.
    Mask,
}

struct Entry {
    id: &'static str,
    need: Need,
    weight: u64,
    /// Needs the system font scan (3.5 to 5 s the first time): left out of the default tests.
    text: bool,
    make: Gen,
}

fn e(id: &'static str, weight: u64, make: Gen) -> Entry {
    Entry { id, need: need_of(id), weight, text: false, make }
}

fn need_of(id: &str) -> Need {
    match id {
        "edit.clear" | "edit.fill" | "edit.stroke" | "paint.stroke" | "paint.pencil" | "paint.bucket" | "paint.gradient" | "paint.blur" | "paint.smudge"
        | "paint.dodge" | "paint.burn" => Need::PixelLayer,
        "filter.blur.gaussianBlur" | "filter.blur.motionBlur" | "filter.noise.addNoise" | "filter.stylize.emboss" | "filter.sharpen.smartSharpen" => {
            Need::PixelLayer
        }
        "image.adjustments.hueSaturation"
        | "image.adjustments.levels"
        | "image.adjustments.curves"
        | "image.adjustments.invert"
        | "image.adjustments.brightnessContrast" => Need::PixelLayer,
        "select.deselect" | "select.inverse" | "select.modify.expand" | "select.modify.contract" | "select.modify.feather" | "select.saveSelection" => {
            Need::Selection
        }
        "layer.layerMask.apply" | "layer.layerMask.delete" | "layer.layerMask.edit" => Need::Mask,
        _ => Need::Nothing,
    }
}

/// Commands that add a layer.
const ADDS_LAYERS: [&str; 7] = [
    "layer.new.layer",
    "layer.duplicate",
    "layer.new.group",
    "layer.new.layerViaCopy",
    "shape.create",
    "layer.newAdjustmentLayer.levels",
    "layer.newAdjustmentLayer.hueSaturation",
];
const BLENDS: [&str; 14] = [
    "Normal",
    "Multiply",
    "Screen",
    "Overlay",
    "Soft Light",
    "Hard Light",
    "Darken",
    "Lighten",
    "Difference",
    "Exclusion",
    "Color Dodge",
    "Color Burn",
    "Hue",
    "Luminosity",
];
const ANCHORS: [&str; 9] = ["topLeft", "top", "topRight", "left", "center", "right", "bottomLeft", "bottom", "bottomRight"];

/// The curated commands: common, undoable, and given valid parameters.
fn table() -> Vec<Entry> {
    vec![
        // Layers.
        e("layer.new.layer", 6, |r, _| json!({"name": format!("L{}", r.below(1000)), "mode": *r.pick(&BLENDS), "opacity": r.between(0.2, 1.0)})),
        e("layer.duplicate", 4, |r, c| c.layer(json!({}), r)),
        e("layer.delete", 3, |r, c| c.layer(json!({}), r)),
        e("layer.arrange.bringForward", 2, |r, c| c.layer(json!({}), r)),
        e("layer.arrange.sendBackward", 2, |r, c| c.layer(json!({}), r)),
        e("layer.arrange.bringToFront", 2, |r, c| c.layer(json!({}), r)),
        e("layer.arrange.sendToBack", 2, |r, c| c.layer(json!({}), r)),
        e("layer.layerStyle.blendingOptions", 5, |r, c| {
            c.layer(json!({"blend": *r.pick(&BLENDS), "opacity": r.range(5, 100), "fillOpacity": r.range(20, 100)}), r)
        }),
        e("layer.renameLayer", 2, |r, c| c.layer(json!({"name": format!("Name {}", r.below(10_000))}), r)),
        e("layer.hideLayers", 2, |r, c| c.layer(json!({}), r)),
        e("layer.showLayers", 2, |r, c| c.layer(json!({}), r)),
        e("layer.layerMask.revealAll", 3, |r, c| c.layer(json!({}), r)),
        e("layer.layerMask.hideAll", 2, |r, c| c.layer(json!({}), r)),
        e("layer.layerMask.revealSelection", 2, |r, c| c.layer(json!({}), r)),
        e("layer.layerMask.apply", 2, |r, c| c.layer(json!({}), r)),
        e("layer.layerMask.delete", 2, |r, c| c.layer(json!({}), r)),
        e("layer.layerMask.edit", 2, |r, c| c.layer(json!({"density": r.range(10, 100), "feather": r.range(0, 12)}), r)),
        e("layer.new.group", 1, |r, _| json!({"name": format!("Group {}", r.below(100))})),
        e("layer.mergeDown", 2, |r, c| c.layer(json!({}), r)),
        e("layer.new.layerViaCopy", 1, |_, _| json!({})),
        e("layer.select", 3, |r, c| json!({"layer": c.layers.get(r.below(c.layers.len() as u64) as usize).map_or(0, |l| l.0)})),
        e("shape.create", 3, |r, c| {
            let kind = *r.pick(&["rect", "ellipse", "roundedRect", "star", "polygon"]);
            json!({"kind": kind, "rect": c.rect(r), "fill": r.color()})
        }),
        e("layer.newAdjustmentLayer.levels", 1, |r, _| json!({"inBlack": r.range(0, 60), "inWhite": r.range(180, 255), "gamma": r.between(0.5, 2.0)})),
        e("layer.newAdjustmentLayer.hueSaturation", 1, |r, _| json!({"hue": r.range(-90, 90), "saturation": r.range(-50, 50), "lightness": r.range(-30, 30)})),
        // Painting.
        e("edit.fill", 4, |r, c| c.layer(json!({"contents": "color", "color": r.color(), "opacity": r.range(20, 100), "mode": *r.pick(&BLENDS[..6])}), r)),
        e("edit.stroke", 2, |r, _| json!({"width": r.range(1, 8), "color": r.color(), "location": *r.pick(&["inside", "center", "outside"])})),
        e("edit.clear", 2, |_, _| json!({})),
        e(
            "paint.stroke",
            8,
            |r, c| json!({"points": c.stroke_points(r), "size": r.range(4, 40), "hardness": r.between(0.0, 1.0), "opacity": r.between(0.3, 1.0), "color": r.color()}),
        ),
        e("paint.pencil", 2, |r, c| json!({"points": c.stroke_points(r), "size": r.range(1, 8), "color": r.color()})),
        e("paint.bucket", 2, |r, c| {
            let p = c.point(r);
            json!({"x": p[0].clamp(0, c.w - 1), "y": p[1].clamp(0, c.h - 1), "tolerance": r.range(10, 120), "color": r.color()})
        }),
        e(
            "paint.gradient",
            3,
            |r, c| json!({"from": c.point(r), "to": c.point(r), "style": *r.pick(&["linear", "radial", "reflected"]), "colors": [r.color(), r.color()]}),
        ),
        e("paint.blur", 1, |r, c| json!({"points": c.stroke_points(r), "size": r.range(8, 40), "opacity": r.range(30, 100)})),
        e("paint.smudge", 1, |r, c| json!({"points": c.stroke_points(r), "size": r.range(8, 40), "opacity": r.range(30, 100)})),
        e("paint.dodge", 1, |r, c| json!({"points": c.stroke_points(r), "size": r.range(8, 40)})),
        e("paint.burn", 1, |r, c| json!({"points": c.stroke_points(r), "size": r.range(8, 40)})),
        // Transforms and canvas.
        e("edit.transform", 3, |r, c| {
            c.layer(json!({"scaleX": r.between(60.0, 140.0), "scaleY": r.between(60.0, 140.0), "rotate": r.between(-45.0, 45.0), "dx": r.range(-20, 20), "dy": r.range(-20, 20)}), r)
        }),
        e("edit.transform.flipHorizontal", 1, |_, _| json!({})),
        e("edit.transform.rotate90Cw", 1, |_, _| json!({})),
        e("image.imageRotation.90cw", 1, |_, _| json!({})),
        e("image.imageRotation.flipCanvasHorizontal", 1, |_, _| json!({})),
        e(
            "image.canvasSize",
            2,
            |r, c| json!({"width": r.range((c.w - 48).max(MIN_SIDE), (c.w + 48).min(MAX_SIDE)), "height": r.range((c.h - 48).max(MIN_SIDE), (c.h + 48).min(MAX_SIDE)), "anchor": *r.pick(&ANCHORS), "extensionColor": "white"}),
        ),
        e("image.crop", 2, |r, c| {
            let (x, y) = (r.range(0, c.w / 6), r.range(0, c.h / 6));
            json!({"x": x, "y": y, "width": r.range((c.w / 2).max(MIN_SIDE).min(c.w - x), c.w - x), "height": r.range((c.h / 2).max(MIN_SIDE).min(c.h - y), c.h - y)})
        }),
        e(
            "image.imageSize",
            1,
            |r, c| json!({"width": r.range((c.w * 3 / 4).max(MIN_SIDE), (c.w * 5 / 4).min(MAX_SIDE)), "height": r.range((c.h * 3 / 4).max(MIN_SIDE), (c.h * 5 / 4).min(MAX_SIDE))}),
        ),
        // Filters and adjustments.
        e("filter.blur.gaussianBlur", 4, |r, _| json!({"radius": r.between(0.5, 12.0)})),
        e("filter.blur.motionBlur", 1, |r, _| json!({"angle": r.range(-90, 90), "distance": r.range(2, 30)})),
        e("filter.noise.addNoise", 2, |r, _| json!({"amount": r.between(1.0, 40.0), "seed": r.below(1000)})),
        e("filter.stylize.emboss", 1, |r, _| json!({"angle": r.range(-180, 180), "height": r.range(1, 6), "amount": r.range(50, 200)})),
        e("filter.sharpen.smartSharpen", 1, |r, _| json!({"amount": r.range(20, 200), "radius": r.between(0.5, 4.0)})),
        e("image.adjustments.hueSaturation", 2, |r, _| json!({"hue": r.range(-120, 120), "saturation": r.range(-60, 60), "lightness": r.range(-30, 30)})),
        e("image.adjustments.levels", 2, |r, _| json!({"inBlack": r.range(0, 60), "inWhite": r.range(180, 255), "gamma": r.between(0.5, 2.0)})),
        e("image.adjustments.curves", 1, |r, _| json!({"points": [[0, r.range(0, 40)], [128, r.range(90, 170)], [255, r.range(215, 255)]]})),
        e("image.adjustments.invert", 1, |_, _| json!({})),
        e("image.adjustments.brightnessContrast", 1, |r, _| json!({"brightness": r.range(-60, 60), "contrast": r.range(-30, 60)})),
        // Selections and guides.
        e("select.rect", 5, |r, c| {
            let [x, y, w, h] = c.rect(r);
            json!({"x": x, "y": y, "width": w, "height": h, "mode": *r.pick(&["replace", "add", "subtract", "intersect"]), "ellipse": r.chance(30), "feather": r.range(0, 6)})
        }),
        e("select.all", 1, |_, _| json!({})),
        e("select.deselect", 2, |_, _| json!({})),
        e("select.inverse", 1, |_, _| json!({})),
        e("select.modify.expand", 1, |r, _| json!({"radius": r.range(1, 8)})),
        e("select.modify.contract", 1, |r, _| json!({"radius": r.range(1, 8)})),
        e("select.modify.feather", 1, |r, _| json!({"radius": r.between(1.0, 8.0)})),
        e("select.lasso", 1, |r, c| json!({"points": c.stroke_points(r), "mode": *r.pick(&["replace", "add"])})),
        e("select.magicWand", 1, |r, c| {
            let p = c.point(r);
            json!({"x": p[0].clamp(0, c.w - 1), "y": p[1].clamp(0, c.h - 1), "tolerance": r.range(10, 80)})
        }),
        e("select.saveSelection", 1, |_, _| json!({})),
        e("view.newGuide", 1, |r, c| {
            let vertical = r.chance(50);
            json!({"orientation": if vertical { "vertical" } else { "horizontal" }, "position": r.range(4, if vertical { c.w } else { c.h } - 4)})
        }),
        // Type (system font scan on first use).
        Entry {
            id: "type.create",
            need: Need::Nothing,
            weight: 2,
            text: true,
            make: |r, c| {
                let p = c.point(r);
                json!({"x": p[0].clamp(0, c.w - 1), "y": p[1].clamp(10, c.h - 1), "text": format!("Fj{}", r.below(99)), "size": r.range(10, 40), "color": r.color()})
            },
        },
    ]
}

// ---- process-wide panic counting ---------------------------------------------------------------

/// Panics anywhere in the process, counted by a hook: `Session::execute` contains a command's
/// panic and returns an ordinary `Err`, so only the hook sees it (the same as `panic_hunt`).
static PANICS: AtomicUsize = AtomicUsize::new(0);
/// One run at a time, so a panic is charged to the run that caused it.
static RUN: Mutex<()> = Mutex::new(());

fn install_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            PANICS.fetch_add(1, Ordering::SeqCst);
            default(info);
        }));
    });
}

// ---- one run -----------------------------------------------------------------------------------

/// A state of the document the model knows: the document itself, and its digest.
#[derive(Clone)]
struct State {
    doc: Arc<Document>,
    digest: Digest,
}

#[derive(Default)]
struct Stats {
    edits_ok: usize,
    edits_err: usize,
    /// Recorded steps whose document differs from the one before (the rest were no-ops).
    changed: usize,
    undos: usize,
    redos: usize,
    refused_undos: usize,
    refused_redos: usize,
    checkpoints: usize,
    /// Per command: (succeeded, rejected).
    per_command: BTreeMap<&'static str, (usize, usize)>,
    /// Per command: the distinct reasons it was rejected, with counts (for the verbose table).
    reasons: BTreeMap<(&'static str, String), usize>,
    /// Steps over the budget: what, seconds.
    slow: Vec<(String, f64)>,
    slowest: (String, f64),
}

/// What the watchdog reads while a run goes.
struct Progress {
    step: AtomicU64,
    since: Mutex<Instant>,
    what: Mutex<String>,
}

impl Progress {
    fn begin(&self, what: String) {
        *self.since.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        *self.what.lock().unwrap_or_else(PoisonError::into_inner) = what;
        self.step.fetch_add(1, Ordering::SeqCst);
    }
}

struct Runner {
    s: Session,
    rng: Rng,
    table: Vec<Entry>,
    total_weight: u64,
    /// `states[..=cursor]` are the undo side, the rest the redo side. `None`: a state inside a
    /// command that records several steps, whose document the model does not know.
    states: Vec<Option<State>>,
    cursor: usize,
    /// The last actions, for a failure message.
    log: Vec<String>,
    progress: Arc<Progress>,
    budget: Duration,
    stats: Stats,
    panics_seen: usize,
    label: String,
    n: usize,
}

type Check = Result<(), String>;

impl Runner {
    fn new(depth: u32, mode: &str, seed: u64, with_text: bool, progress: Arc<Progress>) -> Runner {
        let mut s = Session::new();
        // Enough history that nothing is dropped.
        s.edit_prefs(|p| {
            p.performance.history_states = 1_000_000;
            p.performance.memory_usage_mb = 1 << 16;
        });
        s.execute("file.new", json!({"width": WIDTH, "height": HEIGHT, "depth": depth, "mode": mode})).expect("file.new");
        // The configurations get different sequences from one seed.
        let rng = Rng(seed ^ (u64::from(depth) << 40) ^ mode.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)));
        // A starting document with a few kinds of layer, so the first steps have something to hit.
        let ctx = Ctx::of(&s).expect("a document");
        s.execute("paint.gradient", json!({"from": [0, 0], "to": [WIDTH, HEIGHT], "colors": ["#2060c0", "#f0c040"]})).expect("gradient");
        s.execute("layer.new.layer", json!({"name": "Paint"})).expect("layer");
        s.execute("paint.stroke", json!({"points": [[20, 30], [120, 60], [200, 150]], "size": 24, "hardness": 0.5, "color": "#d02020"})).expect("stroke");
        s.execute("shape.create", json!({"kind": "ellipse", "rect": [60, 40, 90, 70], "fill": "#20a040"})).expect("shape");
        s.execute("layer.layerMask.revealAll", json!({})).expect("mask");
        s.execute("paint.stroke", json!({"points": [[70, 50], [140, 90]], "size": 18, "hardness": 0.0, "color": "#000000", "target": "mask"})).ok();
        s.execute("select.rect", json!({"x": ctx.w / 4, "y": ctx.h / 4, "width": 100, "height": 60})).expect("select");
        // Where the run starts: this document, and no history behind it.
        let st = s.active_mut().expect("document");
        st.history.clear();
        let doc = st.doc.clone();
        let start = State { digest: digest(&doc), doc };
        let mut table = table();
        table.retain(|e| with_text || !e.text);
        let total_weight = table.iter().map(|e| e.weight).sum();
        Runner {
            s,
            rng,
            table,
            total_weight,
            states: vec![Some(start)],
            cursor: 0,
            log: Vec::new(),
            progress,
            budget: secs("PHOTOCRAFT_UNDO_STRESS_STEP_SECS", 5),
            stats: Stats::default(),
            panics_seen: PANICS.load(Ordering::SeqCst),
            label: format!("{depth}-bit {mode}, seed {seed:#x}"),
            n: 0,
        }
    }

    fn doc(&self) -> Result<Arc<Document>, String> {
        self.s.active().map(|st| st.doc.clone()).ok_or_else(|| "the document went away".to_string())
    }

    fn past_len(&self) -> usize {
        self.s.active().map_or(0, |st| st.history.past_len())
    }

    fn can_redo(&self) -> bool {
        self.s.active().is_some_and(|st| st.history.can_redo())
    }

    /// Run `f`, timing it against the budget and checking for a panic anywhere.
    fn timed<R>(&mut self, what: &str, f: impl FnOnce(&mut Session) -> R) -> Result<R, String> {
        self.n += 1;
        self.progress.begin(format!("step {}: {what}", self.n));
        let t = Instant::now();
        let r = f(&mut self.s);
        let took = t.elapsed();
        if took > self.budget {
            self.stats.slow.push((format!("step {}: {what}", self.n), took.as_secs_f64()));
        }
        if took.as_secs_f64() > self.stats.slowest.1 {
            self.stats.slowest = (format!("step {}: {what}", self.n), took.as_secs_f64());
        }
        let panics = PANICS.load(Ordering::SeqCst);
        if panics != self.panics_seen {
            self.panics_seen = panics;
            return Err(format!("a panic during {what} (see the panic message above)"));
        }
        Ok(r)
    }

    fn note(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > 12 {
            self.log.remove(0);
        }
    }

    /// Cheap invariants of the session after any step.
    fn invariants(&self) -> Check {
        let st = self.s.active().ok_or("the document went away")?;
        if let Some(a) = st.active_layer
            && st.doc.layer(a).is_none()
        {
            return Err(format!("the active layer {} is not in the document", a.0));
        }
        if let Some(missing) = st.selected_layers.iter().find(|id| st.doc.layer(**id).is_none()) {
            return Err(format!("the selected layer {} is not in the document", missing.0));
        }
        if self.past_len() != self.cursor {
            return Err(format!("the history holds {} past states but the model expects {}", self.past_len(), self.cursor));
        }
        if self.can_redo() != (self.cursor + 1 < self.states.len()) {
            return Err(format!("can_redo is {} but the model has {} redo states", self.can_redo(), self.states.len() - self.cursor - 1));
        }
        Ok(())
    }

    /// The document now must be `want`: same digest.
    fn expect(&self, want: &Option<State>, what: &str) -> Check {
        let Some(want) = want else { return Ok(()) };
        let doc = self.doc()?;
        if Arc::ptr_eq(&doc, &want.doc) {
            return Ok(());
        }
        if digest(&doc) == want.digest {
            return Ok(());
        }
        Err(format!("{what}: the document is not the expected one: {}", explain(&want.doc, &doc)))
    }

    /// Pick a command, set up what it needs, run it.
    fn edit(&mut self) -> Check {
        let ctx = Ctx::of(&self.s).ok_or("no document")?;
        let mut pick = self.rng.below(self.total_weight);
        let mut idx = 0;
        for (i, en) in self.table.iter().enumerate() {
            if pick < en.weight {
                idx = i;
                break;
            }
            pick -= en.weight;
        }
        // A document with many layers stops growing: the pick becomes a delete.
        if ctx.layers.len() >= MAX_LAYERS
            && ADDS_LAYERS.contains(&self.table[idx].id)
            && let Some(delete) = self.table.iter().position(|e| e.id == "layer.delete")
        {
            idx = delete;
        }
        // Mostly set up what the command needs (the rest of the time it must refuse cleanly).
        if self.rng.chance(85) {
            match self.table[idx].need {
                Need::PixelLayer if !ctx.active_is_raster && !ctx.rasters.is_empty() => {
                    let target = *self.rng.pick(&ctx.rasters);
                    self.command("layer.select", json!({"layer": target.0}))?;
                }
                Need::Selection if !ctx.has_selection => {
                    let [x, y, w, h] = ctx.rect(&mut self.rng);
                    self.command("select.rect", json!({"x": x, "y": y, "width": w, "height": h}))?;
                }
                Need::Mask if !ctx.active_has_mask => self.command("layer.layerMask.revealAll", json!({}))?,
                _ => {}
            }
        }
        let ctx = Ctx::of(&self.s).ok_or("no document")?;
        let id = self.table[idx].id;
        let params = (self.table[idx].make)(&mut self.rng, &ctx);
        self.command(id, params)
    }

    /// Run one command and check the model.
    fn command(&mut self, id: &'static str, params: Value) -> Check {
        let line = format!("{id} {params}");
        let before = self.doc()?;
        let (past, redo) = (self.past_len(), self.can_redo());
        let result = self.timed(&line, |s| s.execute(id, params.clone()))?;
        self.note(line.clone());
        let after = self.doc()?;
        let counts = self.stats.per_command.entry(id).or_default();
        if let Err(e) = result {
            counts.1 += 1;
            self.stats.edits_err += 1;
            *self.stats.reasons.entry((id, e.to_string().chars().take(110).collect())).or_default() += 1;
            // A rejected command changes nothing.
            if !Arc::ptr_eq(&before, &after) && digest(&before) != digest(&after) {
                return Err(format!("{line} failed ({e}) but changed the document: {}", explain(&before, &after)));
            }
            if self.past_len() != past || self.can_redo() != redo {
                return Err(format!(
                    "{line} failed ({e}) but changed the history ({past} -> {} past states, can_redo {redo} -> {})",
                    self.past_len(),
                    self.can_redo()
                ));
            }
            return Ok(());
        }
        counts.0 += 1;
        self.stats.edits_ok += 1;
        let recorded = self.past_len().checked_sub(past).ok_or_else(|| format!("{line} removed history states"))?;
        let now = State { digest: digest(&after), doc: after };
        match recorded {
            0 => {
                // Not a history step (selecting a layer, a no-op): the document must not have changed.
                let old = self.states.get(self.cursor).cloned().flatten();
                if let Some(old) = old
                    && old.digest != now.digest
                {
                    return Err(format!("{line} changed the document without a history step: {}", explain(&old.doc, &now.doc)));
                }
            }
            n => {
                if self.states.get(self.cursor).cloned().flatten().is_some_and(|old| old.digest != now.digest) {
                    self.stats.changed += 1;
                }
                // Linear history: the redo states are gone; a command may record several steps.
                self.states.truncate(self.cursor + 1);
                self.states.extend((1..n).map(|_| None));
                self.states.push(Some(now));
                self.cursor += n;
            }
        }
        self.invariants().map_err(|e| format!("after {line}: {e}"))
    }

    fn undo(&mut self) -> Check {
        let can = self.cursor > 0;
        let before = self.doc()?;
        let ok = self.timed("undo", |s| s.undo())?;
        self.note("undo".into());
        if ok != can {
            return Err(format!("undo returned {ok} but the model says {can} (history holds {} past states)", self.past_len()));
        }
        if can {
            self.stats.undos += 1;
            self.cursor -= 1;
            let want = self.states[self.cursor].clone();
            self.expect(&want, "after Undo")?;
        } else {
            self.stats.refused_undos += 1;
            let after = self.doc()?;
            if !Arc::ptr_eq(&before, &after) && digest(&before) != digest(&after) {
                return Err("a refused undo changed the document".into());
            }
        }
        self.invariants().map_err(|e| format!("after undo: {e}"))
    }

    fn redo(&mut self) -> Check {
        let can = self.cursor + 1 < self.states.len();
        let before = self.doc()?;
        let ok = self.timed("redo", |s| s.redo())?;
        self.note("redo".into());
        if ok != can {
            return Err(format!("redo returned {ok} but the model says {can}"));
        }
        if can {
            self.stats.redos += 1;
            self.cursor += 1;
            let want = self.states[self.cursor].clone();
            self.expect(&want, "after Redo")?;
        } else {
            self.stats.refused_redos += 1;
            let after = self.doc()?;
            if !Arc::ptr_eq(&before, &after) && digest(&before) != digest(&after) {
                return Err("a refused redo changed the document".into());
            }
        }
        self.invariants().map_err(|e| format!("after redo: {e}"))
    }

    /// Undo everything back to the original, Redo everything forward again, then forget the history.
    fn checkpoint(&mut self) -> Check {
        self.stats.checkpoints += 1;
        let end = self.states.len() - 1;
        let top = self.cursor;
        // To the end of the redo side first, so "Redo all" ends on the last state ever made.
        while self.cursor < end {
            self.redo()?;
        }
        let last = self.states[end].clone();
        self.expect(&last, "at the last state")?;
        while self.cursor > 0 {
            self.undo()?;
        }
        let first = self.states[0].clone();
        self.expect(&first, "Undo all")?;
        if self.s.undo() {
            return Err("Undo all left more to undo".into());
        }
        while self.cursor < end {
            self.redo()?;
        }
        self.expect(&last, "Redo all")?;
        if self.s.redo() {
            return Err("Redo all left more to redo".into());
        }
        // Back where the segment was, so the walk goes on from the same state.
        while self.cursor > top {
            self.undo()?;
        }
        Ok(())
    }

    /// Forget the history and start a new segment from the current document.
    fn new_segment(&mut self) -> Check {
        let Some(st) = self.s.active_mut() else { return Err("no document".into()) };
        st.history.clear();
        let doc = st.doc.clone();
        self.states = vec![Some(State { digest: digest(&doc), doc })];
        self.cursor = 0;
        self.invariants()
    }

    fn run(&mut self, steps: usize) -> Check {
        for i in 0..steps {
            if i > 0 && i % SEGMENT == 0 {
                self.checkpoint()?;
                self.new_segment()?;
            }
            match self.rng.below(100) {
                0..=69 => self.edit(),
                70..=84 => self.undo(),
                85..=94 => self.redo(),
                _ => {
                    // A burst: undo a few steps, look, then redo them.
                    let k = self.rng.range(2, 8);
                    for _ in 0..k {
                        self.undo()?;
                    }
                    for _ in 0..k {
                        self.redo()?;
                    }
                    Ok(())
                }
            }?;
        }
        self.checkpoint()?;
        self.coverage(steps)
    }

    /// A run that mostly got its commands rejected proves little: it must have changed documents
    /// with a good part of the list.
    fn coverage(&self, steps: usize) -> Check {
        if steps < 150 {
            return Ok(());
        }
        let ran = self.stats.per_command.values().filter(|(ok, _)| *ok > 0).count();
        if ran < 25 || self.stats.changed < steps / 4 {
            return Err(format!(
                "too little was exercised: {ran} of {} commands ran, {} recorded edits changed the document (in {steps} steps)",
                self.table.len(),
                self.stats.changed
            ));
        }
        Ok(())
    }

    fn report(&self, steps: usize, took: Duration) {
        let st = &self.stats;
        let distinct = st.per_command.values().filter(|(ok, _)| *ok > 0).count();
        eprintln!(
            "undo_redo_stress {}: {steps} steps in {:.1}s | {} edits ok ({} changed the document), {} rejected, {} undos, {} redos ({} undo, {} redo refused) | {} checkpoints | {distinct}/{} commands ran | slowest {:.2}s ({})",
            self.label,
            took.as_secs_f64(),
            st.edits_ok,
            st.changed,
            st.edits_err,
            st.undos,
            st.redos,
            st.refused_undos,
            st.refused_redos,
            st.checkpoints,
            self.table.len(),
            st.slowest.1,
            st.slowest.0.chars().take(100).collect::<String>(),
        );
        for (what, s) in &st.slow {
            eprintln!("  over the {}s budget: {s:.1}s: {}", self.budget.as_secs(), what.chars().take(200).collect::<String>());
        }
        if std::env::var_os("PHOTOCRAFT_UNDO_STRESS_VERBOSE").is_some() {
            for (id, (ok, err)) in &st.per_command {
                eprintln!("  {id}: {ok} ok, {err} rejected");
            }
            for ((id, why), n) in &st.reasons {
                eprintln!("  rejected {n}x {id}: {why}");
            }
        }
    }
}

/// Run one configuration and seed on a worker thread, with a watchdog for hangs.
fn run_one(depth: u32, mode: &'static str, seed: u64, steps: usize, with_text: bool) {
    install_hook();
    let _one = RUN.lock().unwrap_or_else(PoisonError::into_inner);
    let hang = secs("PHOTOCRAFT_UNDO_STRESS_HANG_SECS", 60);
    let progress = Arc::new(Progress { step: AtomicU64::new(0), since: Mutex::new(Instant::now()), what: Mutex::new("setup".into()) });
    let (tx, rx) = mpsc::channel();
    let p = progress.clone();
    let panics_before = PANICS.load(Ordering::SeqCst);
    std::thread::Builder::new()
        .name("undo-redo-stress".into())
        .stack_size(16 << 20)
        .spawn(move || {
            let t = Instant::now();
            let r = catch_unwind(AssertUnwindSafe(|| {
                let mut run = Runner::new(depth, mode, seed, with_text, p);
                let r = run.run(steps);
                run.report(steps, t.elapsed());
                r.map_err(|e| format!("{e}\n  {} (step {}); the last actions, oldest first:\n    {}", run.label, run.n, run.log.join("\n    ")))
            }));
            let _ = tx.send(r.unwrap_or_else(|_| Err("panicked outside Session::execute (see the panic message above)".into())));
        })
        .expect("spawn the stress thread");
    let result = loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(r) => break r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let stuck = progress.since.lock().unwrap_or_else(PoisonError::into_inner).elapsed();
                if stuck > hang {
                    let what = progress.what.lock().unwrap_or_else(PoisonError::into_inner).clone();
                    panic!("HANG: {depth}-bit {mode}, seed {seed:#x}: {what} has run for {}s (limit {}s)", stuck.as_secs(), hang.as_secs());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the stress thread ended without a result"),
        }
    };
    if let Err(e) = result {
        panic!(
            "UNDO/REDO STRESS FAILURE: {depth}-bit {mode}, seed {seed:#x}, {steps} steps\nreplay: PHOTOCRAFT_UNDO_STRESS_SEED={seed} PHOTOCRAFT_UNDO_STRESS_STEPS={steps} PHOTOCRAFT_UNDO_STRESS_CONFIG={depth}-{mode} cargo test -p photocraft-engine --test undo_redo_stress undo_redo_short -- --nocapture\n{e}"
        );
    }
    assert_eq!(PANICS.load(Ordering::SeqCst), panics_before, "a panic happened during the run (see the message above)");
}

fn run_all(default_seeds: &[u64], default_steps: usize, with_text: bool) {
    for (depth, mode) in configs() {
        for seed in seeds(default_seeds) {
            run_one(depth, mode, seed, steps(default_steps), with_text);
        }
    }
}

#[test]
fn undo_redo_short_run_every_configuration() {
    run_all(&SHORT_SEEDS, SHORT_STEPS, false);
}

#[test]
#[ignore = "slow: 2,000 steps x 3 seeds x 5 configurations, with type layers; run with --ignored"]
fn undo_redo_long_run_every_configuration() {
    run_all(&LONG_SEEDS, LONG_STEPS, true);
}

/// The digest sees what Undo is supposed to restore, and nothing it should not.
#[test]
fn digest_tells_documents_apart() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 64, "height": 48})).unwrap();
    let d0 = digest(&s.active().unwrap().doc);
    assert_eq!(d0, digest(&s.active().unwrap().doc), "stable");
    let changes: [(&str, Value); 6] = [
        ("paint.stroke", json!({"points": [[5, 5], [40, 30]], "size": 8, "color": "#ff0000"})),
        ("layer.layerStyle.blendingOptions", json!({"blend": "Multiply"})),
        ("layer.layerStyle.blendingOptions", json!({"opacity": 40})),
        ("layer.layerMask.revealAll", json!({})),
        ("select.rect", json!({"x": 2, "y": 2, "width": 9, "height": 9})),
        ("view.newGuide", json!({"orientation": "vertical", "position": 10})),
    ];
    let mut seen = vec![d0];
    for (id, p) in changes {
        s.execute(id, p).unwrap();
        let d = digest(&s.active().unwrap().doc);
        assert!(!seen.contains(&d), "{id} did not change the digest");
        seen.push(d);
    }
    for want in seen.iter().rev().skip(1) {
        assert!(s.undo());
        assert_eq!(&digest(&s.active().unwrap().doc), want);
    }
    assert!(!s.undo());
}

/// Two runs of one seed make the same choices, so a failure replays.
#[test]
fn the_same_seed_makes_the_same_choices() {
    let (mut a, mut b) = (Rng(7), Rng(7));
    let ctx = Ctx {
        w: WIDTH,
        h: HEIGHT,
        layers: vec![LayerId(1), LayerId(2)],
        rasters: vec![LayerId(1)],
        active_is_raster: true,
        active_has_mask: false,
        has_selection: true,
    };
    for en in table() {
        assert_eq!((en.make)(&mut a, &ctx), (en.make)(&mut b, &ctx), "{}", en.id);
    }
    assert!(table().iter().all(|e| photocraft_engine::command_specs().iter().any(|s| s.id == e.id)), "every curated command is registered");
}
