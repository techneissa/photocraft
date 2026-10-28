//! The menus that can be arranged: their keys, titles, and every row each can build.

use super::apply::SEPARATOR;
use crate::canvas_tool_menu::{NO_SELECTION_MENU, Row, SELECTION_MENU};

pub const CANVAS_SELECTION: &str = "canvas.selection";
pub const CANVAS_NONE: &str = "canvas.noSelection";
pub const LAYERS_ROW: &str = "layers.row";
pub const TAB: &str = "tab.document";

/// The menus in the order the page lists them: (key, title).
pub const MENUS: [(&str, &str); 4] = [
    (CANVAS_SELECTION, "Canvas, Selection Tools with a Selection"),
    (CANVAS_NONE, "Canvas, Selection Tools without a Selection"),
    (LAYERS_ROW, "Layers Panel Row"),
    (TAB, "Document Tab"),
];

pub fn is_key(key: &str) -> bool {
    MENUS.iter().any(|(k, _)| *k == key)
}

/// Commands of the menu bar can be added to every menu but the document tab's, whose rows act on
/// the clicked tab's document, not the active one.
pub fn can_add(key: &str) -> bool {
    key != TAB
}

/// The Layers panel row menu, every row it can build, in Photoshop's order: (command id, label,
/// when it shows). The rows `layer_menu_ui::entries` builds for a layer are these, less those
/// that don't apply to it. A separator is [`SEPARATOR`].
const LAYER_ROWS: &[(&str, &str, &str)] = &[
    ("layer.layerStyle.blendingOptions", "Blending Options…", ""),
    (SEPARATOR, "", ""),
    ("layer.duplicate", "Duplicate Layer…", ""),
    ("layer.delete", "Delete Layer", ""),
    ("layer.new.groupFromLayers", "Group from Layers…", ""),
    (SEPARATOR, "", ""),
    ("layer.quickExportAsPng", "Quick Export As PNG", ""),
    ("layer.exportAs", "Export As…", ""),
    (SEPARATOR, "", ""),
    ("layer.new.artboardFromLayers", "Artboard from Layers…", ""),
    ("layer.new.frameFromLayers", "Frame from Layers…", ""),
    (SEPARATOR, "", ""),
    ("layer.smartObjects.convertToSmartObject", "Convert to Smart Object", ""),
    ("layer.smartObjects.editContents", "Edit Contents", "Smart object"),
    ("layer.smartObjects.replaceContents", "Replace Contents…", "Smart object"),
    ("layer.smartObjects.relinkToFile", "Relink to File…", "Smart object"),
    ("layer.smartObjects.convertToLayers", "Convert to Layers", "Smart object"),
    ("layer.rasterize.layer", "Rasterize Layer", ""),
    ("layer.rasterize.type", "Rasterize Type", "Type layer"),
    ("layer.rasterize.shape", "Rasterize Layer (Shape)", "Shape layer"),
    ("layer.rasterize.smartObject", "Rasterize Layer (Smart Object)", "Smart object"),
    ("layer.rasterize.fillContent", "Rasterize Layer (Fill)", "Fill layer"),
    ("layer.rasterize.layerStyle", "Rasterize Layer Style", "Has a style"),
    (SEPARATOR, "", ""),
    ("layer.layerMask.enabled", "Disable Layer Mask", "Has a mask"),
    ("layer.layerMask.apply", "Apply Layer Mask", "Has a mask"),
    ("layer.layerMask.delete", "Delete Layer Mask", "Has a mask"),
    ("layer.layerMask.revealAll", "Add Layer Mask", "No selection"),
    ("layer.layerMask.revealSelection", "Add Layer Mask (Selection)", "With a selection"),
    ("layer.createClippingMask", "Create Clipping Mask", "Not clipped"),
    ("layer.releaseClippingMask", "Release Clipping Mask", "Clipped"),
    (SEPARATOR, "", ""),
    ("layer.linkLayers", "Link Layers", ""),
    ("layer.selectLinkedLayers", "Select Linked Layers", ""),
    (SEPARATOR, "", ""),
    ("layer.layerStyle.copyLayerStyle", "Copy Layer Style", ""),
    ("layer.layerStyle.pasteLayerStyle", "Paste Layer Style", ""),
    ("layer.layerStyle.clear", "Clear Layer Style", ""),
    (SEPARATOR, "", ""),
    ("layer.mergeLayers", "Merge Layers", "Several selected"),
    ("layer.mergeGroup", "Merge Group", "Group"),
    ("layer.mergeDown", "Merge Down", ""),
    ("layer.mergeVisible", "Merge Visible", ""),
    ("layer.flattenImage", "Flatten Image", ""),
];

/// The document tab menu's rows (its items hold their command's params and enablement).
const TAB_ROWS: &[(&str, &str)] =
    &[("file.close", "Close"), ("file.closeOthers", "Close Others"), ("file.closeAll", "Close All"), ("file.revealInFinder", "Reveal in Finder")];

/// Every row menu `key` can build, a separator as `None`, in the order the menu has them when it
/// builds them all: the arrangement a menu has until the user changes it.
pub fn rows(key: &str) -> Vec<Row> {
    match key {
        CANVAS_SELECTION => SELECTION_MENU.to_vec(),
        CANVAS_NONE => NO_SELECTION_MENU.to_vec(),
        LAYERS_ROW => LAYER_ROWS.iter().map(|(id, label, _)| (*id != SEPARATOR).then_some((*label, *id))).collect(),
        TAB => TAB_ROWS.iter().map(|(id, label)| Some((if *id == "file.revealInFinder" { crate::menus::reveal_label() } else { *label }, *id))).collect(),
        _ => Vec::new(),
    }
}

/// The command ids of [`rows`]: what the menu itself builds, as opposed to commands added.
pub fn universe(key: &str) -> Vec<&'static str> {
    rows(key).into_iter().flatten().map(|(_, id)| id).collect()
}

/// When a row of menu `key` shows (empty: always), for the editor's note.
pub fn when(key: &str, id: &str) -> &'static str {
    if key != LAYERS_ROW {
        return "";
    }
    LAYER_ROWS.iter().find(|(i, ..)| *i == id).map_or("", |(.., when)| when)
}

/// The label of a row of menu `key` (None for a command it does not build).
pub fn label(key: &str, id: &str) -> Option<&'static str> {
    rows(key).into_iter().flatten().find(|(_, i)| *i == id).map(|(label, _)| label)
}
