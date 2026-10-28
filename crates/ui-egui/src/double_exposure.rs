//! Layer › Double Exposure, the shell's half. The engine command `layer.doubleExposure` sets the
//! layer up (Screen, a Reveal All mask, the mask targeted); this makes the first stroke hide the
//! scene: the Brush tool with a soft round tip about an eighth of the canvas wide, black
//! foreground and white background (the mask's own colour pair, so the layer's pixel colours come
//! back with ⌘2). Tool and colours are shell state, so none of it is part of the undo step.

use serde_json::Value;

use crate::PhotocraftApp;
use crate::state::Tool;

const COMMAND: &str = "layer.doubleExposure";

/// The brush diameter for a canvas: an eighth of its shorter side, within the brush range.
pub(crate) fn brush_size(width: i32, height: i32) -> f32 {
    let side = width.min(height).max(0) as f32;
    (side / 8.0).round().clamp(1.0, photocraft_engine::paint::brush::MAX_BRUSH_SIZE)
}

/// Run after every command: sets the tool, brush and colours once `layer.doubleExposure` worked.
pub(crate) fn after_command(app: &mut PhotocraftApp, id: &str, result: &Result<Value, String>) {
    if id != COMMAND || result.is_err() {
        return;
    }
    // The Layers panel targets the new mask too (the engine already made it the channel target).
    app.ui.mask_target = true;
    app.ui.vector_mask_target = false;
    app.ui.filter_mask_target = None;
    app.ui.properties_layer = None;
    app.sync_mask_targets();

    let size = app.session.active().map_or(1.0, |st| {
        let b = st.doc.bounds();
        brush_size(b.x1 - b.x0, b.y1 - b.y0)
    });
    crate::tool_memory::select(app, Tool::Brush);
    crate::paint_mouse::sync_tool_brush(app);
    let tools = &mut app.session.tools;
    let mut brush = crate::paint_mouse::fresh_brush(&tools.brush);
    brush.size = size;
    brush.hardness = 0.0;
    brush.erase = false;
    tools.brush = brush;
    tools.current_preset = None;
    tools.brush_changed();
    // The mask is targeted, so these are the mask's colours: the pixels' pair is kept aside.
    tools.foreground = [0.0, 0.0, 0.0, 1.0];
    tools.background = [1.0, 1.0, 1.0, 1.0];
    app.ui.status = tl!("Paint black to hide, white to bring back (X swaps)").into();
    app.ui.status_error = false;
}

#[cfg(test)]
mod tests;
