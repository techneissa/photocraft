//! The Check for Updates window and the row in Settings.

use super::{Origin, State, start, update};
use crate::PhotocraftApp;
use crate::widgets::{ButtonRole, DialogButton};

const TITLE: &str = "Check for Updates";

/// What the user chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Check,
    Update,
    Close,
}

/// The window (Help › Check for Updates…, and a new build found at start).
pub(crate) fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if !app.fork_update_rt.open {
        return;
    }
    let state = app.fork_update_rt.state.clone();
    let tokens = crate::theme::Tokens::get(ctx);
    let mut action = None;
    egui::Window::new(TITLE)
        .id(egui::Id::unique("fork-update-window"))
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let color = if matches!(state, State::Failed(_)) { tokens.warning } else { tokens.text };
            ui.label(egui::RichText::new(state.label()).color(color).font(crate::theme::semibold(14.0)));
            if state == State::Ready {
                ui.label(egui::RichText::new("PhotoCraft closes and opens again.").color(tokens.text_dim));
            }
            ui.add_space(8.0);
            let busy = matches!(state, State::Checking | State::Updating);
            let buttons: Vec<DialogButton> = match state {
                State::Ready => vec![DialogButton::new(ButtonRole::Default, "Update", 100.0), DialogButton::new(ButtonRole::Cancel, "Later", 100.0)],
                _ => vec![DialogButton::new(ButtonRole::Default, "Check Again", 110.0).enabled(!busy), DialogButton::new(ButtonRole::Cancel, "Close", 100.0)],
            };
            ui.horizontal(|ui| {
                action = crate::widgets::dialog_buttons(ui, &buttons).map(|role| match role {
                    ButtonRole::Default if state == State::Ready => Action::Update,
                    ButtonRole::Default => Action::Check,
                    _ => Action::Close,
                });
            });
        });
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        action = Some(Action::Close);
    }
    match action {
        Some(Action::Check) => start(app, Origin::Menu),
        Some(Action::Update) => {
            update(app);
            app.fork_update_rt.open = false;
        }
        Some(Action::Close) => app.fork_update_rt.open = false,
        None => {}
    }
}

/// Does the Settings search `filter` find the row?
pub(crate) fn row_matches(filter: &str) -> bool {
    let filter = filter.trim().to_lowercase();
    filter.is_empty() || "check for updates".contains(&filter) || "update".contains(&filter) || "build".contains(&filter)
}

/// Settings › General: the Check for Updates button, its one-line status, and Update when a new
/// build is ready.
pub(crate) fn settings_row(ui: &mut egui::Ui, app: &mut PhotocraftApp) {
    let tokens = crate::theme::Tokens::get(ui.ctx());
    let state = app.fork_update_rt.state.clone();
    let mut action = None;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let busy = matches!(state, State::Checking | State::Updating);
        if ui.add_enabled_ui(!busy, |ui| crate::widgets::secondary_button(ui, "Check for Updates", 140.0)).inner.clicked() {
            action = Some(Action::Check);
        }
        let color = if matches!(state, State::Failed(_)) { tokens.warning } else { tokens.text_dim };
        ui.add(egui::Label::new(egui::RichText::new(state.label()).color(color)).truncate());
        if state == State::Ready && crate::widgets::primary_button(ui, "Update", 90.0).clicked() {
            action = Some(Action::Update);
        }
    });
    match action {
        Some(Action::Check) => start(app, Origin::Settings),
        Some(Action::Update) => update(app),
        _ => {}
    }
}
