//! Select › Refine Hair: the selection (or layer mask) becomes a matte that follows hair, fur and
//! other fine edges, as Select and Mask's Refine Hair button does (#2671). A band `radius` px wide
//! each side of the edge is solved for alpha with closed-form matting; see `photocraft_algo::hair`.
//! One history step.

use photocraft_algo::hair;
use photocraft_algo::matting;
use photocraft_color::PixelFormat;
use photocraft_raster::Surface;
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::sam3_select::ProtectSource;
use crate::smartselect_cmds::with_doc_sampler;
use crate::{EngineError, Result, Session};

const CMD: &str = "select.refineHair";

fn bad(msg: &str) -> EngineError {
    EngineError::BadParams { cmd: CMD.into(), msg: msg.into() }
}

fn enabled(s: &Session) -> std::result::Result<(), String> {
    let d = s.active().ok_or_else(|| "no document open".to_string())?;
    if d.doc.selection.is_some() || d.active_layer.and_then(|id| d.doc.layer(id)).is_some_and(|l| l.mask.is_some()) {
        return Ok(());
    }
    Err("no selection or layer mask".into())
}

fn refine_hair(s: &mut Session, p: &Value) -> Result<Value> {
    let radius = match p.get("radius") {
        None | Some(Value::Null) => 0.0,
        Some(v) => v.as_f64().filter(|r| r.is_finite() && (0.0..=1000.0).contains(r)).ok_or_else(|| bad("radius must be a number from 0 to 1000"))? as f32,
    };
    let all_layers = match p.get("sampleAllLayers") {
        None | Some(Value::Null) => false,
        Some(v) => v.as_bool().ok_or_else(|| bad("sampleAllLayers must be true or false"))?,
    };
    let protect_subject = match p.get("protectSubject") {
        None | Some(Value::Null) => false,
        Some(v) => v.as_bool().ok_or_else(|| bad("protectSubject must be true or false"))?,
    };
    let target_mask = match p.get("target").map(Value::as_str) {
        None | Some(Some("selection")) => false,
        Some(Some("mask")) => true,
        Some(_) => return Err(bad("target must be selection|mask")),
    };
    let effective = hair::effective_radius(radius).map_err(|e| bad(&e.to_string()))?;
    // Protect Subject needs SAM 3's encoding of the picture ready (`select.objectPrepare` with
    // `wait`); without it the run is Refine Hair as before and the result says so.
    let finder = if protect_subject {
        match s.sam3_protect(all_layers) {
            ProtectSource::Ready(finder) => Some(finder),
            _ => None,
        }
    } else {
        None
    };
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let active = d.active_layer.filter(|id| d.doc.layer(*id).is_some());
    let mask_surf = active.and_then(|id| d.doc.layer(id)).and_then(|l| l.mask.as_ref()).map(|m| &m.surface);
    // The selection unless the layer mask is asked for (or there is only the mask), as Select and
    // Mask starts; the result goes back where it came from.
    let to_mask = if target_mask { mask_surf.is_some() } else { d.doc.selection.is_none() && mask_surf.is_some() };
    let start = if to_mask { mask_surf } else { d.doc.selection.as_ref() };
    let Some(start) = start else { return Err(EngineError::Other("no selection or layer mask".into())) };
    let content = start.content_bounds();
    hair::check_size(content, d.doc.bounds(), radius).map_err(|e| bad(&e.to_string()))?;
    // The subject SAM 3 finds inside the selection's bounds, sure foreground for the matting.
    let subject = finder.and_then(|f| f.find(content).ok().flatten());
    let region = with_doc_sampler(&d.doc, active, all_layers, |smp, doc| {
        hair::refine_hair_protected(smp, &matting::surface_reader(start), content, doc.bounds(), radius, subject.as_ref())
    })
    .map_err(|e| bad(&e.to_string()))?;
    let protected = subject.is_some();
    let surface = region.as_ref().map(matting::region_surface);
    if to_mask {
        let id = active.ok_or_else(|| EngineError::Other("no active layer".into()))?;
        s.edit("Refine Hair", |doc, _| {
            let layer = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
            if let Some(m) = layer.mask.as_mut() {
                m.surface = surface.unwrap_or_else(|| Surface::new(PixelFormat::GRAY8));
            }
            Ok(())
        })?;
        Ok(json!({ "output": "layerMask", "layer": id.0, "radius": effective, "protected": protected }))
    } else {
        let selected = s.edit("Refine Hair", |doc, _| {
            doc.selection = surface;
            Ok(doc.selection.is_some())
        })?;
        Ok(json!({ "output": "selection", "selected": selected, "radius": effective, "protected": protected }))
    }
}

/// Refine Hair's command spec.
pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: CMD,
        label: "Refine Hair",
        menu: &[],
        shortcut: None,
        params: r##"{"radius":px=0,"sampleAllLayers":bool=false,"target":"selection|mask"="selection","protectSubject":bool=false} (protectSubject: SAM 3's subject inside the selection's bounds, from the encoding Object Selection shares, stays fully selected: prepare it first with select.objectPrepare; "protected" says whether it was used; radius: the band's width each side of the selection's edge, 0 = 10, at most 64; the result replaces the selection, or the layer mask when there is no selection or target is "mask"; returns the radius used)"##,
        enabled,
        run: refine_hair,
        journal: true,
    }]
}

#[cfg(test)]
mod tests;
