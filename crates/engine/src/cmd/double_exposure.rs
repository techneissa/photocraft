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

use super::check_all_unlocked;
use super::layer_params::mask_at_depth;
use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

const CMD: &str = "layer.doubleExposure";

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

/// The layer that becomes the second exposure, or why there is none.
fn target(s: &Session) -> std::result::Result<LayerId, String> {
    let st = s.active().ok_or("no document open")?;
    let pixels: Vec<LayerId> = st.doc.walk().into_iter().filter(|(_, _, l)| matches!(l.content, LayerContent::Raster(_))).map(|(_, _, l)| l.id).collect();
    if pixels.len() < 2 {
        return Err("Double Exposure needs two pixel layers".into());
    }
    let selected = st.selected_layers();
    // One selected pixel layer that has another below it; else the one on top.
    let pick = match selected.as_slice() {
        [one] if pixels.first() != Some(one) && pixels.contains(one) => Some(*one),
        _ => None,
    };
    pick.or_else(|| pixels.last().copied()).ok_or_else(|| "Double Exposure needs two pixel layers".into())
}

fn double_exposure(s: &mut Session, p: &Value) -> Result<Value> {
    let exposure = parse(p)?;
    let id = target(s).map_err(EngineError::Other)?;
    let added = s.edit("Double Exposure", |doc, active| {
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
    // Targeting the mask is view state, not part of the history step.
    s.execute("channel.target", json!({"channel": "mask"}))?;
    Ok(json!({"layer": id.0, "mode": exposure.name(), "maskAdded": added}))
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: CMD,
        label: "Double Exposure",
        menu: &["Layer"],
        shortcut: None,
        params: r##"{"mode":"screen"|"lighten"="screen"?} → {"layer": id, "mode", "maskAdded": bool} (one step: the layer becomes the second exposure: the selected pixel layer when exactly one is selected and it isn't the lowest, else the topmost pixel layer; its blend mode is set, a Reveal All layer mask is added unless it has one, and the mask is targeted so painting black hides it and white brings it back; needs two pixel layers)"##,
        enabled: |s| target(s).map(|_| ()),
        run: double_exposure,
        journal: true,
    }]
}

#[cfg(test)]
mod tests;
