//! Select and Mask's Protect Subject: the checkbox next to Refine Hair, and the wait for SAM 3.
//!
//! With the box ticked, Refine Hair asks SAM 3 for the subject inside the selection's bounds and
//! keeps it fully selected (`photocraft_engine::sam3_select::ProtectSubject`), so a deep shadow at
//! the edge is never cut away. SAM 3 needs the picture encoded first (about 6 s, shared with
//! Object Selection). The button then waits: the edit is added once the encoding is ready, the
//! status bar says "Preparing SAM 3…", and Esc stops the run (it adds nothing). An encoding that
//! fails or goes away adds Refine Hair without Protect Subject, with the reason in the status bar.
//! The box is ticked by default where SAM 3 is installed and the Mac has the memory for it,
//! greyed out otherwise, and its state is remembered in the preferences.

use photocraft_engine::sam3_select::{Prepare, ProtectSource, Sam3Status, SubjectFinder};
use serde_json::{Value, json};

use super::{PhotocraftApp, SelectMaskWorkspace, widgets};

/// The `prefs.dialogs` key of the remembered state of the box.
const REMEMBERED: &str = "select.refineEdge.protectSubject";

/// The workspace's Protect Subject state.
#[derive(Default)]
pub(super) struct ProtectUi {
    /// The box is ticked (kept even while SAM 3 cannot be used).
    pub on: bool,
    /// SAM 3 can be used here: installed, enough memory, Object Selection not set to Classic.
    pub possible: bool,
    /// The box was clicked this frame: [`persist`] keeps its state.
    changed: bool,
    /// The Refine Hair edit waiting for SAM 3's encoding.
    waiting: Option<Value>,
}

/// Whether SAM 3 can be used here. Unit tests never load the model (a Mac that has it would encode
/// the picture, 20 GB): they install a fake.
fn usable(app: &mut PhotocraftApp) -> bool {
    (!cfg!(test) || app.session.sam3_backend_installed()) && app.session.sam3_protect_possible()
}

impl ProtectUi {
    pub(super) fn load(app: &mut PhotocraftApp) -> Self {
        let on = app.session.prefs().dialogs.get(REMEMBERED).and_then(Value::as_bool).unwrap_or(true);
        Self { on, possible: usable(app), changed: false, waiting: None }
    }

    /// Whether a Refine Hair run is waiting for SAM 3.
    pub(super) fn waiting(&self) -> bool {
        self.waiting.is_some()
    }
}

/// The box, in the options bar after the Refine Hair button.
pub(super) fn checkbox(ui: &mut egui::Ui, w: &mut SelectMaskWorkspace) {
    let possible = w.protect.possible;
    let mut shown = w.protect.on && possible;
    let r = ui
        .add_enabled_ui(possible, |ui| widgets::checkbox(ui, &mut shown, tl!("Protect Subject")))
        .inner
        .on_disabled_hover_text(tl!("Needs SAM 3 (Preferences › AI Models)"));
    if possible && r.changed() {
        w.protect.on = shown;
        w.protect.changed = true;
    }
}

/// SAM 3 for the preview, when an edit asks for Protect Subject and the encoding is ready: the
/// finder and the Sample All Layers choice it is encoded for.
pub(super) fn finder(app: &mut PhotocraftApp) -> Option<(bool, SubjectFinder)> {
    let w = app.distort.select_mask.as_ref()?;
    if !w.edits.iter().any(|e| e.get("protectSubject").and_then(Value::as_bool) == Some(true)) {
        return None;
    }
    let all = w.sample_all;
    match app.session.sam3_protect(all) {
        ProtectSource::Ready(finder) => Some((all, finder)),
        _ => None,
    }
}

/// Once per frame, before the options bar: whether SAM 3 can be used now.
pub(super) fn refresh(app: &mut PhotocraftApp) {
    let possible = usable(app);
    if let Some(w) = app.distort.select_mask.as_mut() {
        w.protect.possible = possible;
    }
}

/// Keeps the box's state in the preferences once it was clicked.
pub(super) fn persist(app: &mut PhotocraftApp) {
    let Some(w) = app.distort.select_mask.as_mut().filter(|w| w.protect.changed) else { return };
    w.protect.changed = false;
    let on = w.protect.on;
    app.session.prefs.edit(|p| p.dialogs.insert(REMEMBERED.into(), json!(on)));
}

fn say(app: &mut PhotocraftApp, message: &str) {
    app.ui.status = message.into();
    app.ui.status_error = false;
}

fn protected(mut edit: Value) -> Value {
    if let Some(o) = edit.as_object_mut() {
        o.insert("protectSubject".into(), json!(true));
    }
    edit
}

/// A tool edit about to be added. A Refine Hair with the box ticked is held back until SAM 3's
/// encoding is ready (returns `None`) or goes out with `protectSubject` when it is; anything else
/// goes out as it is.
pub(super) fn route(app: &mut PhotocraftApp, edit: Value) -> Option<Value> {
    let Some(w) = app.distort.select_mask.as_ref() else { return Some(edit) };
    if edit.get("tool").and_then(Value::as_str) != Some("refineHair") || !(w.protect.on && w.protect.possible) {
        return Some(edit);
    }
    match app.session.sam3_prepare(w.sample_all, Prepare::Force) {
        Ok(Sam3Status::Ready) => Some(protected(edit)),
        Ok(Sam3Status::Preparing) => {
            say(app, tl!("Preparing SAM 3 for this image… (Esc stops it)"));
            if let Some(w) = app.distort.select_mask.as_mut() {
                w.protect.waiting = Some(edit);
            }
            None
        }
        Ok(Sam3Status::Unavailable(note)) => {
            say(app, &without(note));
            Some(edit)
        }
        Ok(Sam3Status::Failed(note)) => {
            say(app, &without(Some(note)));
            Some(edit)
        }
        _ => {
            say(app, &without(None));
            Some(edit)
        }
    }
}

/// The note for a Refine Hair that ran without Protect Subject, and why when SAM 3 said.
fn without(note: impl Into<Option<&'static str>>) -> String {
    match note.into() {
        Some(why) => format!("{}: {}", tl!("Refine Hair ran without Protect Subject"), crate::retouch_ui::smart_note(why)),
        None => tl!("SAM 3 isn't ready, Refine Hair ran without Protect Subject").into(),
    }
}

/// One step per frame: adds the waiting Refine Hair once SAM 3 is ready, or without Protect
/// Subject once it cannot be. Keeps frames coming while the encoding runs.
pub(super) fn tick(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(w) = app.distort.select_mask.as_ref().filter(|w| w.protect.waiting.is_some()) else { return };
    let status = app.session.sam3_status(w.sample_all);
    let protect = match status {
        Sam3Status::Preparing => {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        Sam3Status::Ready => true,
        Sam3Status::Unavailable(note) => {
            say(app, &without(note));
            false
        }
        Sam3Status::Failed(note) => {
            say(app, &without(Some(note)));
            false
        }
        _ => {
            say(app, &without(None));
            false
        }
    };
    if let Some(w) = app.distort.select_mask.as_mut()
        && let Some(waiting) = w.protect.waiting.take()
    {
        w.add_edit(if protect { protected(waiting) } else { waiting });
    }
}

/// Esc while Refine Hair waits for SAM 3: stops the encoding and the run. True when it did.
pub(super) fn stop(app: &mut PhotocraftApp) -> bool {
    let Some(w) = app.distort.select_mask.as_mut() else { return false };
    if w.protect.waiting.take().is_none() {
        return false;
    }
    app.session.sam3_cancel();
    say(app, tl!("Refine Hair stopped"));
    true
}

#[cfg(test)]
mod tests;
