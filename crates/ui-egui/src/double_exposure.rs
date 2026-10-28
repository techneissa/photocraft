//! Layer › Double Exposure, the shell's half. The engine command `layer.doubleExposure` sets the
//! layer up (Screen, a Reveal All mask, the mask targeted); when the person asks for it (the menu,
//! the command palette, the Layers panel button) this also makes the first stroke hide the scene:
//! the Brush tool with a soft round tip an eighth of the canvas's shorter side wide, black
//! foreground and white background (the mask's own colour pair, so the layer's pixel colours come
//! back with ⌘2). A script, the CLI or MCP running the command through `engine.execute` gets the
//! engine half only and keeps its tool. Tool and colours are shell state, so none of it is part of
//! the undo step.
//!
//! The soft brush is temporary: the Brush's own brush is kept ([`Stash`]) and never written to the
//! preferences, and comes back when another painting tool takes over the session brush.

use photocraft_engine::BrushSettings;
use serde_json::Value;

use crate::PhotocraftApp;
use crate::state::Tool;

pub(crate) const COMMAND: &str = "layer.doubleExposure";

/// The Brush's own brush while the soft one stands in for it, and the brush revision right after
/// the swap: any later change (the person edited the soft brush) makes it theirs, so it stays.
pub(crate) struct Stash {
    brush: BrushSettings,
    preset: Option<String>,
    rev: u64,
}

/// The brush diameter for a canvas: an eighth of its shorter side, within the brush range.
pub(crate) fn brush_size(width: i32, height: i32) -> f32 {
    let side = width.min(height).max(0) as f32;
    (side / 8.0).round().clamp(1.0, photocraft_engine::paint::brush::MAX_BRUSH_SIZE)
}

/// Run the command as the person asked for it: then set the tool, brush and colours.
pub(crate) fn invoke(app: &mut PhotocraftApp, params: Value) -> Result<Value, String> {
    let result = app.run(COMMAND, params)?;
    paint_setup(app);
    Ok(result)
}

fn paint_setup(app: &mut PhotocraftApp) {
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
    // The Brush's own brush is saved to the preferences first (as a frame would), and stays what
    // they hold: the soft brush below is not marked as changed.
    photocraft_engine::brush_cmds::remember_brush(&mut app.session);
    let tools = &mut app.session.tools;
    let kept = app.prefs_rt.double_exposure_brush.take().filter(|s| s.rev == tools.brush_rev);
    let stash = kept.unwrap_or_else(|| Stash { brush: tools.brush.clone(), preset: tools.current_preset.clone(), rev: tools.brush_rev });
    let mut brush = crate::paint_mouse::fresh_brush(&tools.brush);
    brush.size = size;
    brush.hardness = 0.0;
    brush.erase = false;
    tools.brush = brush;
    tools.current_preset = None;
    tools.brush_saved_rev = tools.brush_rev;
    app.prefs_rt.double_exposure_brush = Some(Stash { rev: tools.brush_rev, ..stash });
    // The mask is targeted, so these are the mask's colours: the pixels' pair is kept aside.
    tools.foreground = [0.0, 0.0, 0.0, 1.0];
    tools.background = [1.0, 1.0, 1.0, 1.0];
    app.ui.status = tl!("Paint black to hide, white to bring back (X swaps)").into();
    app.ui.status_error = false;
}

/// Another painting tool takes over the session brush: the Brush gets its own brush back, unless
/// the person changed the soft one meanwhile.
pub(crate) fn restore_brush(app: &mut PhotocraftApp) {
    let Some(stash) = app.prefs_rt.double_exposure_brush.take() else { return };
    let tools = &mut app.session.tools;
    if tools.brush_rev == stash.rev {
        tools.brush = stash.brush;
        tools.current_preset = stash.preset;
        tools.brush_changed();
    }
}

#[cfg(test)]
mod tests;
