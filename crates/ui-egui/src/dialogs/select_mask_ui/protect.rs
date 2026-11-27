//! Select and Mask's Refine Hair and Protect Subject: the button and the box in the options bar (the framework's
//! `RefineHairControl`, a fork patch of craft-libs: fork/craft-libs.md), and the wait for SAM 3.
//!
//! With the box ticked, Refine Hair asks SAM 3 for the subject inside the selection's bounds and keeps it fully
//! selected (`photocraft_engine::sam3_select::ProtectSubject`), so a deep shadow at the edge is never cut away. SAM 3
//! needs the picture encoded first (about 6 s, shared with Object Selection). The button then waits: the edit is
//! recorded once the encoding is ready, the status bar says "Preparing SAM 3…", and the button reads Stop Refine Hair
//! (it stops the run and adds nothing; closing Select and Mask stops it too). An encoding that fails or goes away
//! records Refine Hair without Protect Subject, with the reason in the status bar. The box is ticked by default where
//! SAM 3 is installed and the Mac has the memory for it, greyed out otherwise, and its state is remembered in the
//! preferences.

use craft::imaging::ui::select_mask::{RefineHairControl, RefineHairEvent, SelectMaskEdit, SelectMaskOp};
use craft::ui::dialog::DialogId;
use photocraft_engine::sam3_select::{Prepare, ProtectSource, Sam3Status, SubjectFinder};
use serde_json::{Value, json};

use super::{Hosted, PhotocraftApp};

/// The `prefs.dialogs` key of the remembered state of the box.
const REMEMBERED: &str = "select.refineEdge.protectSubject";

/// SAM 3 for the edits that ask for Protect Subject: the finder of each Sample All Layers choice they use.
pub(super) type Finders = Vec<(bool, SubjectFinder)>;

/// The Refine Hair run waiting for SAM 3's encoding.
#[derive(Clone, Copy)]
struct Waiting {
    radius: f64,
    sample_all: bool,
}

/// Select and Mask's Protect Subject state (`PhotocraftApp::select_mask_protect`).
#[derive(Default)]
pub(crate) struct ProtectUi {
    /// The box is ticked (kept even while SAM 3 cannot be used).
    pub(super) on: bool,
    /// SAM 3 can be used here: installed, enough memory, Object Selection not set to Classic.
    pub(super) possible: bool,
    /// The Refine Hair run waiting for SAM 3's encoding.
    waiting: Option<Waiting>,
}

impl ProtectUi {
    /// Whether a Refine Hair run is waiting for SAM 3.
    pub(super) fn waiting(&self) -> bool {
        self.waiting.is_some()
    }
}

/// Whether SAM 3 can be used here. Unit tests never load the model (a Mac that has it would encode the picture,
/// 20 GB): they install a fake.
fn usable(app: &mut PhotocraftApp) -> bool {
    (!cfg!(test) || app.session.sam3_backend_installed()) && app.session.sam3_protect_possible()
}

/// The state for a workspace that opens now: the remembered box, and whether SAM 3 can be used.
pub(super) fn load(app: &mut PhotocraftApp) {
    let on = app.session.prefs().dialogs.get(REMEMBERED).and_then(Value::as_bool).unwrap_or(true);
    let possible = usable(app);
    app.select_mask_protect = ProtectUi { on, possible, waiting: None };
}

/// What the options bar shows now.
pub(super) fn control(app: &PhotocraftApp) -> RefineHairControl {
    let p = &app.select_mask_protect;
    RefineHairControl::new(p.on, p.possible).waiting(p.waiting())
}

/// Once per frame: whether SAM 3 can be used now.
pub(super) fn refresh(app: &mut PhotocraftApp) {
    app.select_mask_protect.possible = usable(app);
}

/// SAM 3 for the preview: a finder for each Sample All Layers choice an edit asking for Protect Subject uses, once the
/// encoding is ready.
pub(super) fn finders(app: &mut PhotocraftApp, edits: &[SelectMaskEdit]) -> Finders {
    let mut out = Finders::new();
    for all in [false, true] {
        let asked = edits.iter().any(|e| matches!(e, SelectMaskEdit::RefineHair { sample_all_layers, protect_subject: true, .. } if *sample_all_layers == all));
        if asked && let ProtectSource::Ready(finder) = app.session.sam3_protect(all) {
            out.push((all, finder));
        }
    }
    out
}

fn say(app: &mut PhotocraftApp, message: &str) {
    app.ui.status = message.into();
    app.ui.status_error = false;
}

/// The note for a Refine Hair that ran without Protect Subject, and why when SAM 3 said.
fn without(note: impl Into<Option<&'static str>>) -> String {
    match note.into() {
        Some(why) => format!("{}: {}", tl!("Refine Hair ran without Protect Subject"), crate::retouch_ui::smart_note(why)),
        None => tl!("SAM 3 isn't ready, Refine Hair ran without Protect Subject").into(),
    }
}

/// Records a Refine Hair edit in the workspace `id`.
fn record(app: &mut PhotocraftApp, ctx: &egui::Context, id: DialogId, run: Waiting, protect: bool) {
    let edit = SelectMaskEdit::refine_hair(run.radius, run.sample_all, protect);
    if let Some(hosted) = app.framework.dialogs.get_mut_by_id::<Hosted>(id) {
        // A limit that refuses it is worded in the panel by the workspace.
        let _ = hosted.workspace_mut().perform(SelectMaskOp::Edit(edit));
        // The next pass reports it and asks for the preview.
        ctx.request_repaint();
    }
}

/// What the Refine Hair button, its Stop and the box ask of the app: Refine Hair with the box ticked is held back until
/// SAM 3's encoding is ready, or recorded with Protect Subject when it is; without the box it is recorded as it is.
pub(super) fn on_event(app: &mut PhotocraftApp, ctx: &egui::Context, id: DialogId, event: RefineHairEvent) {
    match event {
        RefineHairEvent::Protect(on) => {
            if app.select_mask_protect.possible {
                app.select_mask_protect.on = on;
                app.session.prefs.edit(|p| p.dialogs.insert(REMEMBERED.into(), json!(on)));
            }
        }
        RefineHairEvent::Stop => {
            if app.select_mask_protect.waiting.take().is_some() {
                app.session.sam3_cancel();
                say(app, tl!("Refine Hair stopped"));
            }
        }
        RefineHairEvent::Run { radius, sample_all_layers } => {
            let run = Waiting { radius, sample_all: sample_all_layers };
            if app.select_mask_protect.waiting() {
                return;
            }
            if !(app.select_mask_protect.on && app.select_mask_protect.possible) {
                return record(app, ctx, id, run, false);
            }
            match app.session.sam3_prepare(run.sample_all, Prepare::Force) {
                Ok(Sam3Status::Ready) => record(app, ctx, id, run, true),
                Ok(Sam3Status::Preparing) => {
                    say(app, tl!("Preparing SAM 3 for this image… (Esc closes Select and Mask)"));
                    app.select_mask_protect.waiting = Some(run);
                }
                Ok(Sam3Status::Unavailable(note)) => {
                    say(app, &without(note));
                    record(app, ctx, id, run, false);
                }
                Ok(Sam3Status::Failed(note)) => {
                    say(app, &without(Some(note)));
                    record(app, ctx, id, run, false);
                }
                _ => {
                    say(app, &without(None));
                    record(app, ctx, id, run, false);
                }
            }
        }
        _ => {}
    }
}

/// One step per frame: records the waiting Refine Hair once SAM 3 is ready, or without Protect Subject once it cannot
/// be. Keeps frames coming while the encoding runs.
pub(super) fn tick(app: &mut PhotocraftApp, ctx: &egui::Context, id: DialogId) {
    let Some(run) = app.select_mask_protect.waiting else { return };
    let protect = match app.session.sam3_status(run.sample_all) {
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
    app.select_mask_protect.waiting = None;
    record(app, ctx, id, run, protect);
}

/// Select and Mask closed (OK, Cancel or Esc) while Refine Hair waited: stops the encoding and the run.
pub(super) fn closed(app: &mut PhotocraftApp) {
    if app.select_mask_protect.waiting.take().is_some() {
        app.session.sam3_cancel();
    }
}

#[cfg(test)]
mod tests;
