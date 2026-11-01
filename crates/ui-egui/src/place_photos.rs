//! Placing photos on an open picture, the quick way: File › Place Embedded… (⇧⌘P) takes several
//! photos at once, and the Layers panel footer has a Place Photo button that runs it.
//!
//! The picker allows a multi-select. One photo is placed as File › Place Embedded always placed
//! it. Several are queued like files dropped together on the canvas ([`DropPlace`], placed one
//! after another by `place_next_dropped`): each photo becomes its own layer and starts in Free
//! Transform, ↩ applies it and Esc takes it back, then the next one follows. Photos are read when
//! their turn comes, so a long selection does not hold every file in memory.

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::file_dialog::{FileDialogAnswer, FileDialogRequest, last_used_dir, read_picked};
use crate::file_open::DropPlace;

/// The command the footer button and the shortcut run.
pub(crate) const COMMAND: &str = "file.placeEmbedded";

/// What the person is told when a Free Transform box is still open (as for any Place).
const BLOCKED: &str = "Commit or cancel the transform first";

/// File › Place Embedded… without parameters: ask for one or more photos, then place them on
/// document `doc` (the one that was active when the picker opened).
pub(crate) fn pick(app: &mut PhotocraftApp, ctx: &egui::Context, doc: photocraft_doc::DocId) -> Result<Value, String> {
    let ctx = ctx.clone();
    let initial_dir = last_used_dir(&app.ui.recent_files);
    app.ask_file(FileDialogRequest::Open { multiple: true, initial_dir, extensions: None }, move |app, answer| place(app, &ctx, doc, answer))
}

/// The picker is answered: place what it chose.
fn place(app: &mut PhotocraftApp, ctx: &egui::Context, doc: photocraft_doc::DocId, answer: FileDialogAnswer) -> Result<Value, String> {
    if app.ui.transform.is_some() {
        return Err(BLOCKED.into());
    }
    app.refocus(doc)?;
    let Some(queue) = queue_of(&answer) else {
        // One photo: exactly File › Place Embedded's single-file way.
        let (name, bytes) = read_picked(answer)?;
        let result = app.place_bytes(&name, bytes, None)?;
        crate::transform_tool::begin_placed(app, ctx)?;
        return Ok(result);
    };
    let count = queue.len();
    app.drop_places.extend(queue);
    // The first starts now; the rest follow as each Free Transform ends.
    app.place_next_dropped(ctx);
    Ok(json!({ "queued": count }))
}

/// The photos of an answer with two or more, in the order chosen; `None` for one (or none), which
/// is placed directly. A desktop answer keeps paths (read at the photo's turn), a browser answer
/// has the bytes already.
fn queue_of(answer: &FileDialogAnswer) -> Option<Vec<DropPlace>> {
    match answer {
        FileDialogAnswer::Paths(paths) if paths.len() > 1 => Some(paths.iter().cloned().map(DropPlace::Path).collect()),
        FileDialogAnswer::ContentsMany(files) if files.len() > 1 => Some(files.iter().cloned().map(|(name, bytes)| DropPlace::Bytes(name, bytes)).collect()),
        _ => None,
    }
}

/// The Layers panel footer's Place Photo button: icon only, greyed as the menu item is, a click
/// runs Place Embedded.
pub(crate) fn footer_button(app: &PhotocraftApp, ui: &mut egui::Ui, actions: &mut Vec<(String, Value)>) {
    let tip = crate::shortcuts::tip_label(app, "Place Photo", COMMAND);
    let enabled = crate::menus::is_enabled(app, COMMAND);
    let button = ui.add_enabled_ui(enabled, |ui| crate::icons::button(ui, "place-photo", 26.0, false, &tip)).inner;
    // Null params: the click takes the menu's path, which opens the picker.
    if button.clicked() {
        actions.push((COMMAND.into(), Value::Null));
    }
}

#[cfg(test)]
mod tests;
