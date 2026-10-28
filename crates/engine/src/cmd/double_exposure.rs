//! Layer › Double Exposure (`layer.doubleExposure`, fork-only): sets a layer up as the second
//! exposure of a double exposure in one step. Its blend mode becomes Screen (or Lighten), it gets
//! a Reveal All layer mask when it has none, and the mask becomes the edit target, so painting
//! black hides parts of it and white brings them back.
//!
//! The layer is the selected one when exactly one pixel layer other than the lowest is selected,
//! else the topmost pixel layer. The brush, the colours and the tool are the shell's (see the
//! UI's `double_exposure`): this command only changes the document, in one history step.

use photocraft_color::BlendMode;
use photocraft_doc::{LayerContent, LayerId, LayerMask};
use serde_json::{Value, json};

use super::channel_cmds::{self, ChannelTarget};
use super::check_all_unlocked;
use super::layer_params::mask_at_depth;
use crate::commands::CommandSpec;
use crate::{DocState, EngineError, Result, Session};

const CMD: &str = "layer.doubleExposure";
/// The history label of the step.
const LABEL: &str = "Double Exposure";

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: CMD.into(), msg: msg.into() }
}

/// The blend mode of the second exposure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Exposure {
    Screen,
    Lighten,
}

impl Exposure {
    fn blend(self) -> BlendMode {
        match self {
            Exposure::Screen => BlendMode::Screen,
            Exposure::Lighten => BlendMode::Lighten,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Exposure::Screen => "screen",
            Exposure::Lighten => "lighten",
        }
    }
}

/// `{"mode": "screen" | "lighten"}` checked; no `mode` is Screen.
fn parse(p: &Value) -> Result<Exposure> {
    match p {
        Value::Null => return Ok(Exposure::Screen),
        Value::Object(_) => {}
        _ => return Err(bad("params must be an object")),
    }
    match p.get("mode") {
        None | Some(Value::Null) => Ok(Exposure::Screen),
        Some(Value::String(m)) if m.eq_ignore_ascii_case("screen") => Ok(Exposure::Screen),
        Some(Value::String(m)) if m.eq_ignore_ascii_case("lighten") => Ok(Exposure::Lighten),
        Some(other) => Err(bad(format!("`mode` must be \"screen\" or \"lighten\", got {other}"))),
    }
}

const NEEDS_TWO: &str = "Double Exposure needs two visible pixel layers, the upper one unlocked";

/// The visible pixel layers, bottom to top, each with whether it is unlocked. Only the active
/// artboard's layers count while one is active, else all of the document's. A layer in a hidden
/// group is hidden too.
fn pixel_layers(st: &DocState) -> Vec<(LayerId, bool)> {
    let doc = &st.doc;
    let artboard = st
        .active_layer
        .and_then(|id| doc.path_of(id))
        .and_then(|p| p.first().copied())
        .filter(|i| doc.layers.get(*i).is_some_and(|l| matches!(&l.content, LayerContent::Group(g) if g.artboard.is_some())));
    doc.walk()
        .into_iter()
        .filter(|(path, _, l)| {
            matches!(l.content, LayerContent::Raster(_))
                && (artboard.is_none() || path.first().copied() == artboard)
                && (1..=path.len()).all(|n| path.get(..n).and_then(|p| doc.layer_at(p)).is_some_and(|a| a.visible))
        })
        .map(|(_, _, l)| (l.id, !doc.effective_locks(l.id).all))
        .collect()
}

/// The layer that becomes the second exposure, or why there is none: the selected one when it is
/// the only one selected, unlocked and has a pixel layer below it, else the topmost such layer.
fn target(s: &Session) -> std::result::Result<LayerId, String> {
    let st = s.active().ok_or("no document open")?;
    let pool = pixel_layers(st);
    let upper: Vec<LayerId> = pool.iter().skip(1).filter(|(_, unlocked)| *unlocked).map(|(id, _)| *id).collect();
    let selected = st.selected_layers();
    match selected.as_slice() {
        [one] if upper.contains(one) => Ok(*one),
        _ => upper.last().copied().ok_or_else(|| NEEDS_TWO.into()),
    }
}

fn double_exposure(s: &mut Session, p: &Value) -> Result<Value> {
    let exposure = parse(p)?;
    let id = target(s).map_err(EngineError::Other)?;
    let added = s.edit(LABEL, |doc, active| {
        check_all_unlocked(doc, id)?;
        let depth = doc.depth;
        let layer = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
        layer.blend = exposure.blend();
        let added = layer.mask.is_none();
        if added {
            layer.mask = Some(mask_at_depth(LayerMask::reveal_all(), depth)?);
        }
        *active = Some(id);
        Ok(added)
    })?;
    if let Some(st) = s.active_mut() {
        st.selected_layers = vec![id];
    }
    // Targeting the mask is view state, not part of the history step; it can't fail.
    channel_cmds::target(s, ChannelTarget::LayerMask);
    Ok(json!({"layer": id.0, "mode": exposure.name(), "maskAdded": added}))
}

/// After Redo: the step brings the mask back, so target it again, as running the command did.
/// (Undo needs nothing: with the mask gone the view falls back to the pixels.)
pub(crate) fn after_redo(s: &mut Session) {
    let masked = s.active().is_some_and(|st| {
        st.history.undo_label() == Some(LABEL) && st.active_layer.and_then(|id| st.doc.layer(id)).is_some_and(|l| l.mask.is_some())
    });
    if masked {
        channel_cmds::target(s, ChannelTarget::LayerMask);
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: CMD,
        label: "Double Exposure",
        menu: &["Layer"],
        shortcut: None,
        params: r##"{"mode":"screen"|"lighten"="screen"?} → {"layer": id, "mode", "maskAdded": bool} (one step: sets the layer's blend mode, adds a Reveal All mask unless it has one and targets the mask; the layer is the selected visible, unlocked pixel layer above another, else the topmost such layer)"##,
        enabled: |s| target(s).map(|_| ()),
        run: double_exposure,
        journal: true,
    }]
}

#[cfg(test)]
mod tests;
