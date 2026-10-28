//! "Assign Shortcut…" on a right-click: on an icon button, a toolbar tool, an in-window menu row
//! and a command palette row. The right-click only leaves an offer in egui's memory ([`on`]); one
//! small menu with that single row is drawn at the pointer ([`show`]), and picking it opens
//! Keyboard Shortcuts on the command with key capture on, so a widget needs no access to the app.

use egui::{Id, Order, Pos2, Response};

use crate::PhotocraftApp;
use crate::theme::Tokens;

/// A right-click waiting for its menu: the command, where the pointer was, and the frame it came
/// in (the press of that same click must not close the menu it opened).
#[derive(Clone)]
struct Offer {
    command: String,
    pos: Pos2,
    frame: u64,
}

fn offer_id() -> Id {
    Id::new("assign-shortcut-offer")
}

fn request_id() -> Id {
    Id::new("assign-shortcut-request")
}

/// Offer the menu for `command` at `pos`.
pub(crate) fn offer(ctx: &egui::Context, command: &str, pos: Pos2) {
    let offer = Offer { command: command.to_string(), pos, frame: ctx.cumulative_frame_nr() };
    ctx.data_mut(|d| d.insert_temp(offer_id(), offer));
    ctx.request_repaint();
}

/// Ask for Keyboard Shortcuts on `command` (it opens in [`show`], this frame or the next).
pub(crate) fn request(ctx: &egui::Context, command: &str) {
    ctx.data_mut(|d| d.insert_temp(request_id(), command.to_string()));
    ctx.request_repaint();
}

/// The command a pick asked for since the last [`show`].
fn take_request(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(request_id()))
}

/// Is the small menu open?
pub(crate) fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Offer>(offer_id())).is_some()
}

/// A right-click on `response` (enabled or not) offers the menu for `command`; true when it did.
pub(crate) fn on(ui: &egui::Ui, response: &Response, command: &str) -> bool {
    let ctx = ui.ctx();
    let Some(pos) = ctx.input(|i| if i.pointer.secondary_clicked() { i.pointer.interact_pos() } else { None }) else { return false };
    if !response.rect.contains(pos) || !ui.clip_rect().contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
        return false;
    }
    offer(ctx, command, pos);
    true
}

/// The "Assign Shortcut…" row for a context menu a widget already has (the options bar's tool icon).
pub(crate) fn item(ui: &mut egui::Ui, command: &str) {
    if ui.button(tl!("Assign Shortcut…")).clicked() {
        request(ui.ctx(), command);
        ui.close();
    }
}

/// [`crate::icons::button`] that offers "Assign Shortcut…" for `command` on a right-click.
pub(crate) fn button(ui: &mut egui::Ui, name: &str, box_size: f32, selected: bool, tooltip: &str, command: &str) -> Response {
    let response = crate::icons::button(ui, name, box_size, selected, tooltip);
    on(ui, &response, command);
    response
}

/// Open the small menu if offered, handle a pick, and open the dialog for a request.
pub(crate) fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if let Some(command) = take_request(ctx) {
        app.ui.palette_open = false;
        crate::prefs_ui::open_shortcuts_for(app, &command);
    }
    let Some(offer) = ctx.data(|d| d.get_temp::<Offer>(offer_id())) else { return };
    let t = Tokens::get(ctx);
    let mut picked = false;
    let area = egui::Area::new(Id::new("assign-shortcut-menu")).order(Order::Tooltip).fixed_pos(offer.pos).constrain(true).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            crate::widgets::style_spectrum_popup_menu(ui);
            ui.set_min_width(150.0);
            let mut b = egui::Button::new(tl!("Assign Shortcut…"));
            if let Some(sc) = app.session.prefs().shortcut(&offer.command, crate::shortcuts::default_shortcut(&offer.command).as_deref()) {
                b = b.shortcut_text(egui::RichText::new(crate::shortcuts::pretty(sc)).color(t.text_faint));
            }
            picked = ui.add(b).clicked();
        });
    });
    let outside = offer.frame != ctx.cumulative_frame_nr()
        && ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
    let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if picked {
        request(ctx, &offer.command);
    }
    if picked || outside || esc {
        ctx.data_mut(|d| d.remove::<Offer>(offer_id()));
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests;
