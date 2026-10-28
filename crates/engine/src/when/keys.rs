//! The context keys a condition can use, and the [`Context`] they are read from each frame.
//!
//! A key is either an enumeration (`tool`, `layerKind`) or on / off (`textFocus`). [`KEYS`] is the
//! one table the parser, the autocomplete, the chips and the builder menus read, so a key added
//! here shows up everywhere. A [`Context`] is plain data (a value number per key); the shell fills
//! it from the app each frame, tests build it by hand.

/// One value of an enumeration key.
#[derive(Debug)]
pub struct ValueDef {
    /// How it is typed: `brush`.
    pub name: &'static str,
    /// How a chip names it: `Brush Tool`.
    pub chip: &'static str,
    pub desc: &'static str,
}

/// What a key holds.
#[derive(Debug)]
pub enum Kind {
    /// One of a list of values (compared with `==` / `!=`).
    Enum(&'static [ValueDef]),
    /// On or off (used alone: `textFocus`, `!textFocus`). The chip names the on state; "Not" in front names the off one.
    Bool { chip: &'static str },
}

/// A context key.
#[derive(Debug)]
pub struct KeyDef {
    pub name: &'static str,
    /// The name in menus (Title Case).
    pub title: &'static str,
    pub desc: &'static str,
    pub kind: Kind,
}

impl KeyDef {
    /// How many values the key can hold (two for an on / off key).
    pub fn size(&self) -> usize {
        match &self.kind {
            Kind::Enum(values) => values.len(),
            Kind::Bool { .. } => 2,
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self.kind, Kind::Bool { .. })
    }

    /// The values of an enumeration key (none for an on / off key).
    pub fn values(&self) -> &'static [ValueDef] {
        match &self.kind {
            Kind::Enum(values) => values,
            Kind::Bool { .. } => &[],
        }
    }
}

macro_rules! values {
    ($(($name:expr, $chip:expr, $desc:expr)),* $(,)?) => {
        &[$(ValueDef { name: $name, chip: $chip, desc: $desc }),*]
    };
}

pub const TOOL: usize = 0;
pub const LAYER_KIND: usize = 1;
pub const TEXT_FOCUS: usize = 2;
pub const WIDGET_FOCUS: usize = 3;
pub const DIALOG_OPEN: usize = 4;
pub const PALETTE_OPEN: usize = 5;
pub const HAS_DOCUMENT: usize = 6;
pub const HAS_SELECTION: usize = 7;
pub const HAS_PATH: usize = 8;
pub const MULTI_LAYER: usize = 9;
pub const EDITING_TEXT: usize = 10;
pub const TRANSFORM_ACTIVE: usize = 11;
pub const CROP_PENDING: usize = 12;
pub const FULLSCREEN: usize = 13;

/// Every context key, in the order menus and the autocomplete list them.
pub const KEYS: &[KeyDef] = &[
    KeyDef {
        name: "tool",
        title: "Tool",
        desc: "The tool in use (the ids of Edit › Keyboard Shortcuts › Tools)",
        kind: Kind::Enum(values![
            ("move", "Move Tool", "Move Tool"),
            ("artboard", "Artboard Tool", "Artboard Tool"),
            ("rectMarquee", "Rectangular Marquee Tool", "Rectangular Marquee Tool"),
            ("ellipseMarquee", "Elliptical Marquee Tool", "Elliptical Marquee Tool"),
            ("singleRowMarquee", "Single Row Marquee Tool", "Single Row Marquee Tool"),
            ("singleColumnMarquee", "Single Column Marquee Tool", "Single Column Marquee Tool"),
            ("lasso", "Lasso Tool", "Lasso Tool"),
            ("polygonLasso", "Polygonal Lasso Tool", "Polygonal Lasso Tool"),
            ("magneticLasso", "Magnetic Lasso Tool", "Magnetic Lasso Tool"),
            ("magicWand", "Magic Wand Tool", "Magic Wand Tool"),
            ("crop", "Crop Tool", "Crop Tool"),
            ("perspectiveCrop", "Perspective Crop Tool", "Perspective Crop Tool"),
            ("eyedropper", "Eyedropper Tool", "Eyedropper Tool"),
            ("colorSampler", "Color Sampler Tool", "Color Sampler Tool"),
            ("ruler", "Ruler Tool", "Ruler Tool"),
            ("note", "Note Tool", "Note Tool"),
            ("count", "Count Tool", "Count Tool"),
            ("brush", "Brush Tool", "Brush Tool"),
            ("pencil", "Pencil Tool", "Pencil Tool"),
            ("mixerBrush", "Mixer Brush Tool", "Mixer Brush Tool"),
            ("colorReplacement", "Color Replacement Tool", "Color Replacement Tool"),
            ("eraser", "Eraser Tool", "Eraser Tool"),
            ("backgroundEraser", "Background Eraser Tool", "Background Eraser Tool"),
            ("magicEraser", "Magic Eraser Tool", "Magic Eraser Tool"),
            ("gradient", "Gradient Tool", "Gradient Tool"),
            ("paintBucket", "Paint Bucket Tool", "Paint Bucket Tool"),
            ("type", "Horizontal Type Tool", "Horizontal Type Tool"),
            ("verticalType", "Vertical Type Tool", "Vertical Type Tool"),
            ("horizontalTypeMask", "Horizontal Type Mask Tool", "Horizontal Type Mask Tool"),
            ("verticalTypeMask", "Vertical Type Mask Tool", "Vertical Type Mask Tool"),
            ("hand", "Hand Tool", "Hand Tool"),
            ("rotateView", "Rotate View Tool", "Rotate View Tool"),
            ("zoom", "Zoom Tool", "Zoom Tool"),
            ("remove", "Remove Tool", "Remove Tool"),
            ("spotHealing", "Spot Healing Brush Tool", "Spot Healing Brush Tool"),
            ("healing", "Healing Brush Tool", "Healing Brush Tool"),
            ("patch", "Patch Tool", "Patch Tool"),
            ("contentAwareMove", "Content-Aware Move Tool", "Content-Aware Move Tool"),
            ("redEye", "Red Eye Tool", "Red Eye Tool"),
            ("cloneStamp", "Clone Stamp Tool", "Clone Stamp Tool"),
            ("patternStamp", "Pattern Stamp Tool", "Pattern Stamp Tool"),
            ("historyBrush", "History Brush Tool", "History Brush Tool"),
            ("artHistoryBrush", "Art History Brush Tool", "Art History Brush Tool"),
            ("blur", "Blur Tool", "Blur Tool"),
            ("sharpen", "Sharpen Tool", "Sharpen Tool"),
            ("smudge", "Smudge Tool", "Smudge Tool"),
            ("dodge", "Dodge Tool", "Dodge Tool"),
            ("burn", "Burn Tool", "Burn Tool"),
            ("sponge", "Sponge Tool", "Sponge Tool"),
            ("quickSelection", "Quick Selection Tool", "Quick Selection Tool"),
            ("objectSelection", "Object Selection Tool", "Object Selection Tool"),
            ("pen", "Pen Tool", "Pen Tool"),
            ("freeformPen", "Freeform Pen Tool", "Freeform Pen Tool"),
            ("curvaturePen", "Curvature Pen Tool", "Curvature Pen Tool"),
            ("addAnchorPoint", "Add Anchor Point Tool", "Add Anchor Point Tool"),
            ("deleteAnchorPoint", "Delete Anchor Point Tool", "Delete Anchor Point Tool"),
            ("convertPoint", "Convert Point Tool", "Convert Point Tool"),
            ("pathSelection", "Path Selection Tool", "Path Selection Tool"),
            ("directSelection", "Direct Selection Tool", "Direct Selection Tool"),
            ("rectangle", "Rectangle Tool", "Rectangle Tool"),
            ("ellipseShape", "Ellipse Tool", "Ellipse Tool"),
            ("triangle", "Triangle Tool", "Triangle Tool"),
            ("polygon", "Polygon Tool", "Polygon Tool"),
            ("line", "Line Tool", "Line Tool"),
            ("customShape", "Custom Shape Tool", "Custom Shape Tool"),
            ("slice", "Slice Tool", "Slice Tool"),
            ("sliceSelect", "Slice Select Tool", "Slice Select Tool"),
            ("frame", "Frame Tool", "Frame Tool"),
        ]),
    },
    KeyDef {
        name: "layerKind",
        title: "Layer Kind",
        desc: "The kind of the active layer",
        kind: Kind::Enum(values![
            ("none", "No Layer", "There is no active layer"),
            ("raster", "Pixel Layer", "A pixel layer"),
            ("text", "Type Layer", "A type layer"),
            ("shape", "Shape Layer", "A shape layer"),
            ("smart", "Smart Object", "A smart object"),
            ("group", "Group", "A layer group"),
            ("adjustment", "Adjustment Layer", "An adjustment layer"),
            ("fill", "Fill Layer", "A solid colour, gradient or pattern fill layer"),
        ]),
    },
    KeyDef { name: "textFocus", title: "Typing", desc: "A text box has the keyboard (shortcuts normally wait for it)", kind: Kind::Bool { chip: "Typing" } },
    KeyDef {
        name: "widgetFocus",
        title: "Control Focused",
        desc: "A slider, button or list has the keyboard (arrows and Space belong to it)",
        kind: Kind::Bool { chip: "Control Focused" },
    },
    KeyDef { name: "dialogOpen", title: "Dialog", desc: "A dialog is open (shortcuts normally wait for it)", kind: Kind::Bool { chip: "Dialog Open" } },
    KeyDef {
        name: "paletteOpen",
        title: "Command Palette",
        desc: "The Command Palette is open (shortcuts normally wait for it)",
        kind: Kind::Bool { chip: "Palette Open" },
    },
    KeyDef { name: "hasDocument", title: "Document", desc: "A document is open", kind: Kind::Bool { chip: "Document Open" } },
    KeyDef { name: "hasSelection", title: "Selection", desc: "The document has a selection", kind: Kind::Bool { chip: "Selection Active" } },
    KeyDef { name: "hasPath", title: "Saved File", desc: "The document has been saved to a file", kind: Kind::Bool { chip: "Saved File" } },
    KeyDef { name: "multiLayer", title: "Several Layers", desc: "Two or more layers are selected", kind: Kind::Bool { chip: "Several Layers" } },
    KeyDef { name: "editingText", title: "Editing Type", desc: "Type is being edited on the canvas", kind: Kind::Bool { chip: "Editing Type" } },
    KeyDef {
        name: "transformActive",
        title: "Transforming",
        desc: "Free Transform (or a warp) is waiting to be committed",
        kind: Kind::Bool { chip: "Transforming" },
    },
    KeyDef { name: "cropPending", title: "Crop Box", desc: "A crop box is waiting to be committed", kind: Kind::Bool { chip: "Crop Box Showing" } },
    KeyDef { name: "fullscreen", title: "Full Screen", desc: "A full-screen mode is on", kind: Kind::Bool { chip: "Full Screen" } },
];

/// The index of the key called `name`.
pub fn key_index(name: &str) -> Option<usize> {
    KEYS.iter().position(|k| k.name == name)
}

/// The index of `value` among the values of the enumeration key `key`.
pub fn value_index(key: usize, value: &str) -> Option<usize> {
    KEYS.get(key)?.values().iter().position(|v| v.name == value)
}

/// Keys that need another to be on: (key, needs). A selection needs a document.
const NEEDS: &[(usize, usize)] = &[
    (HAS_SELECTION, HAS_DOCUMENT),
    (HAS_PATH, HAS_DOCUMENT),
    (MULTI_LAYER, HAS_DOCUMENT),
    (EDITING_TEXT, HAS_DOCUMENT),
    (TRANSFORM_ACTIVE, HAS_DOCUMENT),
    (CROP_PENDING, HAS_DOCUMENT),
    (LAYER_KIND, HAS_DOCUMENT),
    (TEXT_FOCUS, WIDGET_FOCUS),
    (WIDGET_FOCUS, TEXT_FOCUS),
];

/// The keys that have to be enumerated with `keys`: each key's own, and the ones it depends on (so
/// an impossible mix, such as a selection with no document, is never offered as a way two
/// conditions can both hold).
pub fn closure(keys: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = keys.to_vec();
    for k in keys {
        for (a, b) in NEEDS {
            if a == k && !out.contains(b) {
                out.push(*b);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// What the keys hold at one moment: a value number per key (`0`/`1` for an on / off key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    values: [u8; KEYS.len()],
}

impl Default for Context {
    /// Nothing going on: the first tool, no document, every flag off.
    fn default() -> Self {
        Context { values: [0; KEYS.len()] }
    }
}

impl Context {
    pub fn get(&self, key: usize) -> usize {
        self.values.get(key).map_or(0, |v| usize::from(*v))
    }

    pub fn set(&mut self, key: usize, value: usize) {
        if let Some(v) = self.values.get_mut(key) {
            *v = u8::try_from(value).unwrap_or(0);
        }
    }

    /// An on / off key.
    pub fn flag(&self, key: usize) -> bool {
        self.get(key) == 1
    }

    pub fn set_flag(&mut self, key: usize, on: bool) {
        self.set(key, usize::from(on));
    }

    /// Whether the values agree with each other: no selection without a document, no typing and
    /// control focus at once.
    pub fn consistent(&self) -> bool {
        let needs_document = NEEDS.iter().any(|(a, b)| *b == HAS_DOCUMENT && if *a == LAYER_KIND { self.get(LAYER_KIND) != 0 } else { self.flag(*a) });
        (self.flag(HAS_DOCUMENT) || !needs_document) && !(self.flag(TEXT_FOCUS) && self.flag(WIDGET_FOCUS))
    }

    /// A context with `key` set to the value called `value` (`true` / `false` for an on / off
    /// key). A test helper; an unknown key or value leaves it as it was.
    pub fn with(mut self, key: &str, value: &str) -> Self {
        if let Some(k) = key_index(key) {
            match KEYS.get(k).is_some_and(KeyDef::is_bool) {
                true => self.set_flag(k, value == "true"),
                false => self.set(k, value_index(k, value).unwrap_or(0)),
            }
        }
        self
    }

    /// The chip of `key` as this context holds it (`Brush Tool`, `Not Typing`).
    pub fn chip(&self, key: usize) -> String {
        match KEYS.get(key).map(|k| &k.kind) {
            Some(Kind::Enum(values)) => values.get(self.get(key)).map_or("", |v| v.chip).to_string(),
            Some(Kind::Bool { chip }) => {
                if self.flag(key) {
                    chip.to_string()
                } else {
                    format!("Not {chip}")
                }
            }
            None => String::new(),
        }
    }

    /// The value of `key` as a condition spells it (`brush`, `true`).
    pub fn value_name(&self, key: usize) -> String {
        match KEYS.get(key).map(|k| &k.kind) {
            Some(Kind::Enum(values)) => values.get(self.get(key)).map_or("", |v| v.name).to_string(),
            Some(Kind::Bool { .. }) => self.flag(key).to_string(),
            None => String::new(),
        }
    }
}

/// Every context key with its values and what it holds in `now`, as JSON (for agents and the
/// control channel).
pub fn listing(now: &Context) -> serde_json::Value {
    let keys: Vec<serde_json::Value> = KEYS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let values: Vec<serde_json::Value> = match &k.kind {
                Kind::Enum(values) => values.iter().map(|v| serde_json::json!({"name": v.name, "chip": v.chip, "description": v.desc})).collect(),
                Kind::Bool { .. } => vec![serde_json::json!("true"), serde_json::json!("false")],
            };
            serde_json::json!({
                "key": k.name,
                "title": k.title,
                "description": k.desc,
                "type": if k.is_bool() { "bool" } else { "enum" },
                "values": values,
                "now": now.value_name(i),
            })
        })
        .collect();
    serde_json::json!({"keys": keys, "grammar": "key, key == value, key != value, !x, x && y, x || y, (x)"})
}
