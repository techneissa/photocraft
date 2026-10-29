//! The hint that follows the first step of a chord: the pressed key and the keys that finish a
//! chord, each beside its command, drawn as keycaps.

use egui::{Align2, RichText};

use super::{chord, runtime};
use crate::PhotocraftApp;
use crate::theme::Tokens;

/// Once, when the settings hold a condition that can't be read: say it is ignored.
fn damaged_notice(app: &mut PhotocraftApp) {
    if app.keybinds.damage_noticed {
        return;
    }
    app.keybinds.damage_noticed = true;
    let n = photocraft_engine::prefs::damaged_when(&app.session.prefs().shortcut_when).len();
    if n > 0 {
        let template = "{count} shortcut condition(s) in your settings could not be read and are ignored";
        app.ui.status = crate::i18n::fmt(tl!(template), &[("count", &n.to_string())]);
        app.ui.status_error = true;
    }
}

/// Draw the chord hint (one call per frame, after the dialogs).
pub fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    damaged_notice(app);
    let Some(p) = app.keybinds.pending else { return };
    let now = ctx.input(|i| i.time);
    let blocked = !app.ui.dialogs.is_empty() || app.ui.palette_open || app.discard.is_some();
    if blocked || now - p.since > runtime::TIMEOUT {
        app.keybinds.pending = None;
        return;
    }
    let t = Tokens::get(ctx);
    let lead = chord::step_text(&p.first);
    let next = runtime::continuations(app, crate::shortcut_dispatch::Focus::of(ctx), &p);
    let label = |id: &str| crate::i18n::tr_id(crate::i18n::current(), id, &crate::shortcut_dispatch::label(id)).to_string();
    egui::Area::new(egui::Id::unique("pc-chord-hint")).order(egui::Order::Tooltip).anchor(Align2::CENTER_BOTTOM, [0.0, -48.0]).interactable(false).show(
        ctx,
        |ui| {
            egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_min_width(200.0);
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                // the key that started the chord, then each key that finishes it beside its command
                keycap(ui, &t, &lead, crate::theme::semibold(16.0));
                for (step, id) in next.iter().take(10) {
                    ui.horizontal(|ui| {
                        keycap(ui, &t, &chord::step_text(step), crate::theme::semibold(13.0));
                        ui.label(RichText::new(label(id)).color(t.text));
                    });
                }
                if next.len() > 10 {
                    let more = crate::i18n::fmt(tl!("and {count} more"), &[("count", &(next.len() - 10).to_string())]);
                    ui.label(RichText::new(more).size(11.0).color(t.text_dim));
                }
                ui.horizontal(|ui| {
                    keycap(ui, &t, "Esc", crate::theme::medium(11.0));
                    ui.label(RichText::new(tl!("Cancel")).size(11.0).color(t.text_dim));
                });
            });
        },
    );
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}

/// `text` drawn as a key on a keyboard: a rounded box with the key's name in it.
fn keycap(ui: &mut egui::Ui, t: &Tokens, text: &str, font: egui::FontId) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, t.text);
    let size = galley.size() + egui::vec2(14.0, 8.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    p.rect(rect, 4.0, t.field, egui::Stroke::new(1.0, t.field_border), egui::StrokeKind::Inside);
    p.galley(rect.center() - galley.size() / 2.0, galley, t.text);
}
