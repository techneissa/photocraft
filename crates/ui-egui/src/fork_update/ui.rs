//! The Check for Updates row in Settings › General: a button, the check's one-line status beside it,
//! and Update when a new build is ready. The window is `dialog.rs`.

use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Props as _, Show as _, ShowContainer as _};
use craft::ui::display::Text;
use craft::ui::layout_components::Row;

use super::{Origin, State, start, update};
use crate::PhotocraftApp;

/// What the row's buttons asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Click {
    Check,
    Update,
}

/// Draws the row for `state`; the button pressed this pass.
pub(crate) fn settings_row(ui: &mut egui::Ui, state: &State) -> Option<Click> {
    let mut click = None;
    Row::new().show(ui, |ui, _| {
        let busy = matches!(state, State::Checking | State::Updating);
        if Button::new(tl!("Check for Updates")).enabled(!busy).show(ui).clicked() {
            click = Some(Click::Check);
        }
        let label = state.label();
        if !label.is_empty() {
            let text = Text::new(tl!(label.as_str()).to_string());
            if matches!(state, State::Failed(_)) { text.tone(Tone::Warn) } else { text }.show(ui);
        }
        if *state == State::Ready && Button::new(tl!("Update")).show(ui).clicked() {
            click = Some(Click::Update);
        }
    });
    click
}

/// Does what the row asked for: the check answers beside the button and opens no window.
pub(crate) fn run(app: &mut PhotocraftApp, click: Click) {
    match click {
        Click::Check => start(app, Origin::Settings),
        Click::Update => update(app),
    }
}
