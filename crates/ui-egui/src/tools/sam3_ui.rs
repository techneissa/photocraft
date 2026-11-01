//! The shell's side of SAM 3 for Object Selection and Quick Selection: encoding the picture in
//! the background when either tool is picked (the engine does the work, off the UI thread; see
//! `photocraft_engine::sam3_select`), telling the status bar how it goes, and Esc stopping it.
//!
//! One step per frame ([`tick`]): picking one of the tools on a document starts the encoding (a
//! stopped, failed or unloaded one starts again), an edit that dropped it starts it again while
//! the tool stays picked, and a stopped or unloaded one stays that way until the tool is picked
//! again or the mouse is pressed after an idle unload, so the ten-minute idle unload is not undone
//! at once. Until the encoding is ready the Classic finders answer.

use photocraft_doc::DocId;
use photocraft_engine::sam3_select::{Prepare, Sam3Status};

use crate::PhotocraftApp;
use crate::state::Tool;

/// What the last step saw, to tell the status bar only about changes.
#[derive(Default)]
pub(crate) struct Sam3Ui {
    /// The tool and document SAM 3 was last asked for.
    picked: Option<(Tool, DocId)>,
    status: Option<Sam3Status>,
}

/// Whether `tool` selects with SAM 3, and which sampling choice (Sample All Layers) it has.
fn sampling(app: &PhotocraftApp, tool: Tool) -> Option<bool> {
    match tool {
        Tool::ObjectSelection => Some(app.ui.tool_options.sample_all.object_selection),
        Tool::QuickSelection => Some(app.ui.tool_options.sample_all.quick_selection),
        _ => None,
    }
}

/// One step per frame (not in unit tests, which would otherwise load the model on a Mac that has it;
/// they call [`step`]).
pub(crate) fn tick(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if cfg!(test) {
        return;
    }
    let (esc, pressed) = ctx.input(|i| (i.key_pressed(egui::Key::Escape), i.pointer.any_pressed()));
    let busy = step(app, esc && !ctx.text_edit_focused(), pressed);
    if busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// The step: `esc` stops a running encoding, `pressed` is a mouse press this frame. Returns
/// whether the encoding is running (keep frames coming).
pub(crate) fn step(app: &mut PhotocraftApp, esc: bool, pressed: bool) -> bool {
    let tool = app.ui.tool;
    let (Some(all_layers), Some(doc)) = (sampling(app, tool), app.session.active().map(|st| st.doc.id)) else {
        app.sam3_ui.picked = None;
        app.sam3_ui.status = None;
        return false;
    };
    let now = Some((tool, doc));
    let picked = app.sam3_ui.picked != now;
    app.sam3_ui.picked = now;
    let mut status = app.session.sam3_status(all_layers);
    if esc && status == Sam3Status::Preparing && app.session.sam3_cancel() {
        status = app.session.sam3_status(all_layers);
    } else if picked || (pressed && status == Sam3Status::Unloaded) {
        status = app.session.sam3_prepare(all_layers, Prepare::Force).unwrap_or(status);
    } else if status == Sam3Status::Idle {
        status = app.session.sam3_prepare(all_layers, Prepare::Auto).unwrap_or(status);
    }
    let before = app.sam3_ui.status.replace(status);
    if before != Some(status) {
        let message = match (before, status) {
            (_, Sam3Status::Preparing) => Some(tl!("Preparing SAM 3 for this image… (Esc stops it)")),
            (Some(Sam3Status::Preparing), Sam3Status::Ready) => Some(tl!("SAM 3 is ready")),
            (Some(Sam3Status::Preparing), Sam3Status::Cancelled) => Some(tl!("SAM 3 stopped, Classic selects for now")),
            (_, Sam3Status::Failed(note)) => Some(crate::retouch_ui::smart_note(note)),
            _ => None,
        };
        if let Some(message) = message {
            app.ui.status = message.into();
            app.ui.status_error = false;
        }
    }
    status == Sam3Status::Preparing
}

#[cfg(test)]
mod tests;
