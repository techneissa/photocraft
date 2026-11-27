//! The When condition field of the Chords and Conditions tab: while a condition is typed, a list of what can come
//! next (context keys and their negated forms, operators, values) shows above the field, with Tab, the arrow keys and
//! a click taking a suggestion; `+` opens a builder (key, Is or Is Not, value) for people who would rather not type.
//! It is the autocomplete and builder of the fork's old Keyboard Shortcuts window (fork/shortcut-upgrade.md).

use egui::{Align2, FontId, Id, Key, Modifiers, Order, PopupCloseBehavior, Rect, RichText, Sense, pos2, vec2};

use photocraft_engine::when::keys::{KEYS, Kind};
use photocraft_engine::when::suggest::{self, ItemKind};
use photocraft_engine::when::{Expr, parse};

use crate::theme::Tokens;

/// The most suggestions the list holds.
const MAX_ITEMS: usize = 40;
/// Rows of the list that show before it scrolls.
const LIST_ROWS: f32 = 10.0;
const LIST_ROW_H: f32 = 22.0;
/// The width of the `+` button after the field.
pub(super) const PLUS_W: f32 = 24.0;

/// Where the list's highlight is.
#[derive(Clone, Copy, Debug, Default)]
struct Nav {
    selected: usize,
    /// The arrow keys moved the highlight: Enter takes the suggestion.
    navigated: bool,
}

fn set_cursor(ctx: &egui::Context, field: Id, text: &str, byte: usize) {
    let chars = text.get(..byte.min(text.len())).map_or(0, |h| h.chars().count());
    let mut state = egui::text_edit::TextEditState::load(ctx, field).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(chars))));
    state.store(ctx, field);
}

fn cursor_byte(ctx: &egui::Context, field: Id, text: &str) -> usize {
    let chars =
        egui::text_edit::TextEditState::load(ctx, field).and_then(|s| s.cursor.char_range()).map_or_else(|| text.chars().count(), |r| r.primary.index.into());
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

/// The arrow keys, Tab and Enter of the list while the field is focused. The suggestion to take.
fn list_keys(ctx: &egui::Context, nav: &mut Nav, count: usize) -> Option<usize> {
    let (down, up, tab, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Tab),
            nav.navigated && i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if down {
        nav.selected = (nav.selected + 1) % count;
        nav.navigated = true;
    }
    if up {
        nav.selected = (nav.selected + count - 1) % count;
        nav.navigated = true;
    }
    (tab || enter).then_some(nav.selected)
}

/// A suggestion's description cut to a short hint: up to its first parenthesis, at most 36 characters.
fn short(detail: &str) -> String {
    let head = detail.split(" (").next().unwrap_or(detail);
    if head.chars().count() <= 36 { head.to_string() } else { format!("{}…", head.chars().take(35).collect::<String>().trim_end()) }
}

/// The suggestion list above the field. The row clicked, if one was.
fn list(ctx: &egui::Context, t: &Tokens, nav: Nav, field_rect: Rect, sug: &suggest::Suggestions, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let mut clicked = None;
    let area = egui::Area::new(Id::unique("pc-when-list")).order(Order::Tooltip).pivot(Align2::LEFT_BOTTOM).fixed_pos(field_rect.left_top() - vec2(0.0, 3.0));
    area.show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            ui.set_min_width(field_rect.width().max(400.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            let height = (count as f32).min(LIST_ROWS) * LIST_ROW_H;
            egui::ScrollArea::vertical().max_height(height).min_scrolled_height(height).auto_shrink([false, true]).id_salt("when-list-scroll").show(ui, |ui| {
                for (i, item) in sug.items.iter().take(count).enumerate() {
                    let (row, r) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &item.label));
                    if i == nav.selected {
                        ui.painter().rect_filled(row, 4.0, t.accent.gamma_multiply(0.30));
                    } else if r.hovered() {
                        ui.painter().rect_filled(row, 4.0, t.hover);
                    }
                    let tint = match item.kind {
                        ItemKind::Key | ItemKind::Value => t.text,
                        ItemKind::Operator | ItemKind::Group => t.accent_text,
                    };
                    ui.painter().text(pos2(row.left() + 8.0, row.center().y), Align2::LEFT_CENTER, &item.label, FontId::monospace(11.5), tint);
                    let detail = ui.painter().with_clip_rect(row);
                    detail.text(pos2(row.left() + 150.0, row.center().y), Align2::LEFT_CENTER, short(&item.detail), crate::theme::medium(11.0), t.text_dim);
                    if i == nav.selected && nav.navigated {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    });
    clicked
}

/// The When field of command `id`: a text box with the autocomplete list over it and the `+` builder after it. `error`
/// is the parse error to show (the text goes red). True when `when` changed.
pub(super) fn field(ui: &mut egui::Ui, id: &str, when: &mut String, error: Option<&String>) -> bool {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let field = Id::unique(("pc-when-field", id));
    let mut changed = false;
    let mut nav: Nav = ctx.data(|d| d.get_temp(field)).unwrap_or_default();
    let focused = ui.memory(|m| m.has_focus(field));
    if focused {
        let sug = suggest::suggest(when, cursor_byte(&ctx, field, when));
        let count = sug.items.len().min(MAX_ITEMS);
        nav.selected = nav.selected.min(count.saturating_sub(1));
        if count > 0
            && let Some(i) = list_keys(&ctx, &mut nav, count)
        {
            take(&ctx, field, when, &sug, i, &mut nav);
            changed = true;
        }
    }
    let before = when.clone();
    // The `+` and both gaps stay inside the row (a row wider than its area makes the scroll area grow every frame).
    let width = (ui.available_width() - PLUS_W - 2.0 * ui.spacing().item_spacing.x - 2.0 * ui.spacing().button_padding.x).max(80.0);
    let mut edit = egui::TextEdit::singleline(when).id(field).font(FontId::monospace(11.5)).hint_text(tl!("Always")).desired_width(width);
    if error.is_some() {
        edit = edit.text_color(t.danger);
    }
    let response = ui.add(edit);
    let response = match error {
        Some(e) => response.on_hover_text(e.clone()),
        None => response,
    };
    if *when != before {
        nav = Nav::default();
        changed = true;
    }
    if response.has_focus() {
        // Tab and the arrows belong to the list while it is open (egui would move the focus with them).
        let filter = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false };
        ctx.memory_mut(|m| m.set_focus_lock_filter(field, filter));
    }
    if response.has_focus() || response.lost_focus() {
        let sug = suggest::suggest(when, cursor_byte(&ctx, field, when));
        let count = sug.items.len().min(MAX_ITEMS);
        if let Some(i) = list(&ctx, &t, nav, response.rect, &sug, count) {
            take(&ctx, field, when, &sug, i, &mut nav);
            ctx.memory_mut(|m| m.request_focus(field));
            changed = true;
        }
    }
    ctx.data_mut(|d| d.insert_temp(field, nav));
    let plus = ui.add_sized([PLUS_W, 20.0], egui::Button::new("+")).on_hover_text(tl!("Add Condition"));
    if let Some(added) = builder(&plus, &t, id) {
        *when = add_part(when, added);
        changed = true;
    }
    changed
}

/// Puts suggestion `i` into the text and the cursor after it.
fn take(ctx: &egui::Context, field: Id, when: &mut String, sug: &suggest::Suggestions, i: usize, nav: &mut Nav) {
    let Some(item) = sug.items.get(i) else { return };
    let (text, cursor) = suggest::apply(when, sug, item);
    *when = text;
    *nav = Nav::default();
    set_cursor(ctx, field, when, cursor);
    ctx.request_repaint();
}

/// `part` joined to the condition `text` with `&&` (or alone when there is none).
pub(super) fn add_part(text: &str, part: Expr) -> String {
    match parse(text).ok().flatten() {
        Some(e) => e.and(part).text(),
        None => part.text(),
    }
}

/// Where the builder is: no key picked yet, or the key and whether the comparison is negated.
#[derive(Clone, Copy, Default)]
struct Builder {
    key: Option<usize>,
    negated: bool,
}

/// The `+` popup. The part the user finished, if they did.
fn builder(plus: &egui::Response, t: &Tokens, id: &str) -> Option<Expr> {
    let state_id = Id::unique(("pc-when-builder", id));
    let mut chosen: Option<Expr> = None;
    let popup = egui::Popup::menu(plus).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(300.0);
        let mut b: Builder = ui.data(|d| d.get_temp(state_id)).unwrap_or_default();
        chosen = builder_body(ui, t, &mut b);
        if chosen.is_some() {
            b = Builder::default();
            ui.close();
        }
        ui.data_mut(|d| d.insert_temp(state_id, b));
    });
    if popup.is_none() {
        plus.ctx.data_mut(|d| d.remove_temp::<Builder>(state_id));
    }
    chosen
}

/// One clickable row of the builder: a title and a dim description.
fn menu_row(ui: &mut egui::Ui, t: &Tokens, title: &str, detail: &str) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width().max(300.0), 24.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    if r.hovered() {
        ui.painter().rect_filled(rect, 4.0, t.hover);
    }
    ui.painter().text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, title, crate::theme::medium(12.5), t.text);
    ui.painter().text(pos2(rect.left() + 130.0, rect.center().y), Align2::LEFT_CENTER, detail, crate::theme::medium(11.0), t.text_dim);
    r.clicked()
}

/// The builder's two steps.
fn builder_body(ui: &mut egui::Ui, t: &Tokens, b: &mut Builder) -> Option<Expr> {
    ui.spacing_mut().item_spacing.y = 2.0;
    let Some(key) = b.key else {
        ui.label(RichText::new(tl!("Add Condition")).font(crate::theme::semibold(12.5)).color(t.accent_text));
        ui.label(RichText::new(tl!("The shortcut works only while…")).size(11.0).color(t.text_dim));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().max_height(320.0).id_salt("when-builder-keys").show(ui, |ui| {
            for (i, k) in KEYS.iter().enumerate() {
                if menu_row(ui, t, tl!(k.title), tl!(k.desc)) {
                    b.key = Some(i);
                }
            }
        });
        return None;
    };
    let def = KEYS.get(key)?;
    ui.horizontal(|ui| {
        if ui.small_button(tl!("Back")).clicked() {
            *b = Builder::default();
        }
        ui.label(RichText::new(tl!(def.title)).font(crate::theme::semibold(12.5)).color(t.accent_text));
    });
    // Is / Is Not, for every key (an on / off key too: "Is Not Typing").
    ui.horizontal(|ui| {
        for (negated, label) in [(false, "Is"), (true, "Is Not")] {
            if ui.selectable_label(b.negated == negated, tl!(label)).clicked() {
                b.negated = negated;
            }
        }
    });
    ui.add_space(2.0);
    match &def.kind {
        Kind::Enum(values) => {
            return egui::ScrollArea::vertical()
                .max_height(320.0)
                .id_salt("when-builder-values")
                .show(ui, |ui| {
                    values
                        .iter()
                        .enumerate()
                        .find_map(|(i, v)| menu_row(ui, t, tl!(v.chip), tl!(v.desc)).then_some(Expr::Is { key, value: i, equal: !b.negated }))
                })
                .inner;
        }
        Kind::Bool { chip } => {
            if menu_row(ui, t, tl!(chip), tl!(def.desc)) {
                return Some(if b.negated { Expr::Not(Box::new(Expr::Key(key))) } else { Expr::Key(key) });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part the builder adds joins the condition already there, or stands alone.
    #[test]
    fn the_builder_adds_a_part_to_a_condition() {
        let part = || parse("tool == brush").unwrap().unwrap();
        assert_eq!(add_part("", part()), part().text());
        let both = add_part("hasSelection", part());
        let parsed = parse(&both).unwrap().unwrap();
        assert!(both.contains("hasSelection") && both.contains("brush"), "{both}");
        assert_eq!(parsed.text(), both, "it reads back as written");
    }

    /// A suggestion's detail is cut to one short hint.
    #[test]
    fn suggestions_are_short() {
        assert_eq!(short("The brush is active (with details after it)"), "The brush is active");
        assert!(short(&"x".repeat(80)).chars().count() <= 36);
    }
}
