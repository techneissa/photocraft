//! The Check for Updates window: craft-update's check dialog on the framework dialog host, showing
//! this fork's check instead of the official updater's (fork/fork-update-check.md). A new build
//! shows as the dialog's "update available" state; its Update button starts the fork's update route.

use craft::update::stack_dialogs::{CheckChoice, UpdateCheckDialog};
use craft::update::updater::Status;

use super::release::{RELEASE_TAG, REPO};
use super::{Origin, State, start, update};
use crate::PhotocraftApp;

/// The dialog's status for the fork's state. The dialog has no "being made" state: that one reads
/// as a short message, as a failed check does.
fn status_of(state: &State) -> Status {
    match state {
        State::Idle => Status::Idle,
        State::Checking => Status::Checking,
        State::UpToDate | State::Ready => Status::UpToDate,
        State::BeingMade => Status::Failed("A new build is being made. Check again in a few minutes.".into()),
        State::Updating => Status::Installing,
        State::Failed(message) => Status::Failed(message.clone()),
    }
}

/// The build a check found ready, as the release the dialog offers: its short commit as the
/// version, and the `custom-build` release page as its notes.
fn available(state: &State, built: Option<&str>) -> Option<craft::update::Release> {
    (*state == State::Ready).then(|| {
        let commit = built.map_or("", |c| c.get(..7).unwrap_or(c));
        craft::update::Release {
            tag: format!("build {commit}").trim().to_string(),
            name: "PhotoCraft (techneissa's build)".into(),
            html_url: format!("https://github.com/{REPO}/releases/tag/{RELEASE_TAG}"),
            repo: REPO.into(),
            ..Default::default()
        }
    })
}

/// Takes the dialog's answer, then opens, updates or withdraws it. Runs before `DialogHost::run`.
pub(crate) fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    // A cancelled result (the dialog withdrawn below) answers nothing.
    if let Some(choice) = app.framework.dialogs.take::<UpdateCheckDialog>().and_then(|result| result.ok()) {
        app.fork_update_rt.dialog = None;
        match choice {
            CheckChoice::CheckNow | CheckChoice::CheckAgain => start(app, Origin::Menu),
            // The fork's route installs the build and opens PhotoCraft again; the window stays to
            // show a route that could not start.
            CheckChoice::Update | CheckChoice::Download | CheckChoice::RestartToUpdate => update(app),
            _ => app.fork_update_rt.open = false,
        }
    }
    let rt = &mut app.fork_update_rt;
    let dialogs = &mut app.framework.dialogs;
    if rt.open {
        let fresh = UpdateCheckDialog::new(photocraft_engine::build_info::VERSION).with_self_updating(true);
        if let Ok(id) = dialogs.open_unless_open(fresh) {
            rt.dialog = Some(id);
        }
        if let Some(dialog) = rt.dialog.and_then(|id| dialogs.get_mut_by_id::<UpdateCheckDialog>(id)) {
            dialog.set_status(status_of(&rt.state));
            dialog.set_available(available(&rt.state, rt.built.as_deref()));
            dialog.set_installing(rt.state == State::Updating);
        }
    } else if let Some(id) = rt.dialog.take() {
        dialogs.cancel_by_id(ctx, id);
    }
}
