//! techneissa's fork updates itself from techneissa's own builds, never from the official
//! release. This module is the whole check (at start, Help › Check for Updates…, and the button
//! in Settings) and the Update button; the official check (`updates_ui`, `craft-update`) is turned
//! off by [`OFFICIAL_CHECK_OFF`]. See `fork/fork-update-check.md`.
//!
//! The check reads the `custom-build` release of the fork ([`release::REPO`]) and asks GitHub's
//! compare API whether its commit is ahead of the app's own ([`route::own_commit`]).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use serde_json::{Value, json};

use crate::PhotocraftApp;

mod github;
pub mod release;
mod route;
mod ui;

pub use release::Outcome;

use github::{Fetch, Github};

/// The official update check never runs in this fork's build. `false` would give the official
/// check back (its own tests run with it off).
pub const OFFICIAL_CHECK_OFF: bool = true;

/// What the check says, as shown to the user.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Ready,
    BeingMade,
    /// The update route has been started; the Update PhotoCraft app takes over.
    Updating,
    Failed(String),
}

impl State {
    /// The words shown for it (nothing before the first check).
    pub fn label(&self) -> String {
        match self {
            Self::Idle => String::new(),
            Self::Checking => "Checking…".into(),
            Self::UpToDate => "Up to Date".into(),
            Self::Ready => "A New Build Is Ready".into(),
            Self::BeingMade => "A New Build Is Being Made".into(),
            Self::Updating => "Update Started".into(),
            Self::Failed(message) => message.clone(),
        }
    }

    fn id(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::UpToDate => "upToDate",
            Self::Ready => "newBuildReady",
            Self::BeingMade => "newBuildBeingMade",
            Self::Updating => "updating",
            Self::Failed(_) => "failed",
        }
    }
}

/// Where a check was asked from; it decides whether the window opens for the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// At start: quiet unless a new build is ready.
    Start,
    /// Help › Check for Updates…: the window always opens.
    Menu,
    /// The button in Settings: the answer shows beside the button.
    Settings,
}

type Pending = Receiver<Result<Outcome, String>>;

/// The parts that reach outside the app, replaced in tests.
pub(crate) struct Parts {
    fetch: Arc<dyn Fetch>,
    commit: fn() -> Result<String, String>,
    update: fn() -> Result<(), String>,
}

impl Default for Parts {
    fn default() -> Self {
        Self { fetch: Arc::new(Github), commit: route::own_commit, update: route::start_update }
    }
}

pub(crate) struct Runtime {
    pub(crate) state: State,
    /// The official check is off (see [`OFFICIAL_CHECK_OFF`]).
    pub(crate) official_off: bool,
    /// The Check for Updates window is open.
    pub(crate) open: bool,
    started: bool,
    pending: Option<(Pending, Origin)>,
    parts: Parts,
}

impl Default for Runtime {
    fn default() -> Self {
        // The official check's own tests (`updates_ui`) run with the switch off; the tests of this
        // module turn it on (`Runtime::with_parts`), with every outside part faked.
        Self { state: State::Idle, official_off: OFFICIAL_CHECK_OFF && !cfg!(test), open: false, started: false, pending: None, parts: Parts::default() }
    }
}

#[cfg(test)]
impl Runtime {
    pub(crate) fn with_parts(parts: Parts) -> Self {
        Self { parts, official_off: true, ..Self::default() }
    }
}

/// Ask GitHub on a background thread; the answer arrives through [`poll`].
fn start(app: &mut PhotocraftApp, origin: Origin) {
    let rt = &mut app.fork_update_rt;
    if rt.pending.is_some() {
        return;
    }
    let own = match (rt.parts.commit)() {
        Ok(own) => own,
        Err(message) => return finish(app, origin, Err(message)),
    };
    let fetch = rt.parts.fetch.clone();
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new().name("fork update check".into()).spawn(move || {
        let _ = tx.send(release::decide(&*fetch, &own));
    });
    match spawned {
        Ok(_) => {
            rt.pending = Some((rx, origin));
            rt.state = State::Checking;
        }
        Err(e) => finish(app, origin, Err(format!("Could not start the check: {e}."))),
    }
}

/// Take a finished answer: the state, and whether the window opens.
fn finish(app: &mut PhotocraftApp, origin: Origin, result: Result<Outcome, String>) {
    let rt = &mut app.fork_update_rt;
    rt.state = match result {
        Ok(Outcome::UpToDate) => State::UpToDate,
        Ok(Outcome::Ready(_)) => State::Ready,
        Ok(Outcome::BeingMade) => State::BeingMade,
        Err(message) => {
            log::warn!("Update check: {message}");
            State::Failed(message)
        }
    };
    match origin {
        Origin::Start => rt.open |= rt.state == State::Ready,
        Origin::Menu => rt.open = true,
        Origin::Settings => {}
    }
}

fn poll(app: &mut PhotocraftApp) {
    let Some((rx, origin)) = &app.fork_update_rt.pending else { return };
    let origin = *origin;
    let result = match rx.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err("The check stopped before it finished.".into()),
    };
    app.fork_update_rt.pending = None;
    finish(app, origin, result);
}

/// Help › Check for Updates… (through `updates_ui::check`).
pub(crate) fn check(app: &mut PhotocraftApp, _ctx: &egui::Context, params: Value) -> Result<Value, String> {
    if params.as_object().is_none_or(|p| !p.is_empty()) {
        return Err("update commands take an empty object".into());
    }
    app.fork_update_rt.open = true;
    app.fork_update_rt.started = true;
    start(app, Origin::Menu);
    Ok(json!({"status": app.fork_update_rt.state.id()}))
}

/// Every frame (through `updates_ui::tick`): take an answer, and check once at start.
pub(crate) fn tick(app: &mut PhotocraftApp, ctx: &egui::Context) {
    poll(app);
    if !app.fork_update_rt.started {
        app.fork_update_rt.started = true;
        if app.session.prefs().general.automatically_check_updates {
            start(app, Origin::Start);
        }
    }
    if app.fork_update_rt.state == State::Checking {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Install the published build through the existing update route.
pub(crate) fn update(app: &mut PhotocraftApp) {
    if app.fork_update_rt.state != State::Ready {
        return;
    }
    app.fork_update_rt.state = match (app.fork_update_rt.parts.update)() {
        Ok(()) => State::Updating,
        Err(message) => State::Failed(message),
    };
}

pub(crate) use ui::{row_matches, settings_row, show};

#[cfg(test)]
mod snapshots;
#[cfg(test)]
mod tests;
