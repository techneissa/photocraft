//! "Assign Shortcut…" on a right-click: on an icon button, a toolbar tool, an in-window menu row
//! and a command palette row. The right-click only leaves an offer in egui's memory ([`on`]); one
//! small menu with that single row is drawn at the pointer ([`show`]), and picking it opens
//! Keyboard Shortcuts on the command with key capture on, so a widget needs no access to the app.

use egui::{Id, Order, Pos2, Response};

use crate::PhotocraftApp;
use crate::theme::Tokens;

/// A right-click waiting for its menu: the command, where the pointer was, the frame it came in
/// (the press of that same click must not close the menu it opened), and whether it came from a
/// palette row (the menu goes when the palette does).
#[derive(Clone)]
struct Offer {
    command: String,
    pos: Pos2,
    frame: u64,
    palette: bool,
}

/// [`next`]'s note for the icon button drawn next in `ui` (or a child of it) this frame.
#[derive(Clone)]
struct Pending {
    ui: Id,
    command: String,
    frame: u64,
}

fn offer_id() -> Id {
    Id::unique("assign-shortcut-offer")
}

fn request_id() -> Id {
    Id::unique("assign-shortcut-request")
}

fn pending_id() -> Id {
    Id::unique("assign-shortcut-pending")
}

/// Offer the menu for `command` at `pos` (tests; a widget goes through [`on`]).
#[cfg(test)]
pub(crate) fn offer(ctx: &egui::Context, command: &str, pos: Pos2) {
    offer_from(ctx, command, pos, false);
}

fn offer_from(ctx: &egui::Context, command: &str, pos: Pos2, palette: bool) {
    let offer = Offer { command: command.to_string(), pos, frame: ctx.cumulative_frame_nr(), palette };
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

/// Is the small menu open? Like an open menu it owns the keyboard (Esc, tool keys).
pub(crate) fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Offer>(offer_id())).is_some()
}

/// A right-click on `response` (enabled or not) offers the menu for `command`; true when it did.
pub(crate) fn on(ui: &egui::Ui, response: &Response, command: &str) -> bool {
    on_from(ui, response, command, false)
}

/// [`on`] for a palette row.
pub(crate) fn on_palette(ui: &egui::Ui, response: &Response, command: &str) -> bool {
    on_from(ui, response, command, true)
}

fn on_from(ui: &egui::Ui, response: &Response, command: &str, palette: bool) -> bool {
    let ctx = ui.ctx();
    let Some(pos) = ctx.input(|i| if i.pointer.secondary_clicked() { i.pointer.interact_pos() } else { None }) else { return false };
    if !response.rect.contains(pos) || !ui.clip_rect().contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
        return false;
    }
    offer_from(ctx, command, pos, palette);
    true
}

/// A one-line hook before an official `icons::button…` call: that button, drawn next in `ui` (or a
/// child of it), offers "Assign Shortcut…" for `command` on a right-click ([`attach`] picks it up).
pub(crate) fn next(ui: &egui::Ui, command: &str) {
    let pending = Pending { ui: ui.unique_id(), command: command.to_string(), frame: ui.ctx().cumulative_frame_nr() };
    ui.ctx().data_mut(|d| d.insert_temp(pending_id(), pending));
}

/// Called by the icon buttons for each button they draw: gives it [`next`]'s command, if the note
/// is from this frame and from this `ui` or one above it.
pub(crate) fn attach(ui: &egui::Ui, response: &Response) {
    let ctx = ui.ctx();
    let Some(pending) = ctx.data(|d| d.get_temp::<Pending>(pending_id())) else { return };
    if pending.frame != ctx.cumulative_frame_nr() {
        ctx.data_mut(|d| d.remove::<Pending>(pending_id()));
    } else if ui.stack().iter().any(|node| node.unique_id == pending.ui) {
        ctx.data_mut(|d| d.remove::<Pending>(pending_id()));
        on(ui, response, &pending.command);
    }
}

/// The "Assign Shortcut…" row for a context menu a widget already has (the options bar's tool icon).
pub(crate) fn item(ui: &mut egui::Ui, command: &str) {
    if ui.button(tl!("Assign Shortcut…")).clicked() {
        request(ui.ctx(), command);
        ui.close();
    }
}

/// Open the small menu if offered, handle a pick, and open the dialog for a request.
pub(crate) fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if let Some(command) = take_request(ctx) {
        app.ui.palette_open = false;
        crate::prefs_ui::open_shortcuts_for(app, &command);
    }
    let Some(offer) = ctx.data(|d| d.get_temp::<Offer>(offer_id())) else { return };
    // A menu from a palette row goes when the palette does.
    if offer.palette && !app.ui.palette_open {
        ctx.data_mut(|d| d.remove::<Offer>(offer_id()));
        return;
    }
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    // A click anywhere else closes the menu and goes no further: this layer takes it.
    let catcher = egui::Area::new(Id::unique("assign-shortcut-catcher"))
        .order(Order::Tooltip)
        .fixed_pos(screen.min)
        .show(ctx, |ui| ui.allocate_response(screen.size(), egui::Sense::click_and_drag()));
    let mut picked = false;
    let area = egui::Area::new(Id::unique("assign-shortcut-menu")).order(Order::Tooltip).fixed_pos(offer.pos).constrain(true).show(ctx, |ui| {
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
    ctx.move_to_top(catcher.response.layer_id);
    ctx.move_to_top(area.response.layer_id);
    let c = &catcher.inner;
    let outside = offer.frame != ctx.cumulative_frame_nr() && (c.clicked() || c.secondary_clicked() || c.is_pointer_button_down_on());
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
