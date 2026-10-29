//! The When column of Keyboard Shortcuts. At rest a condition is a row of readable chips
//! (`Brush Tool`, `Not Typing`); a click opens a text box under the list with an autocomplete
//! (keys, their negated forms, operators, values), and `+` opens a small builder (key, then Is /
//! Is Not, then value) for people who would rather not type. Nothing invalid is kept: the reason
//! shows under the box in red. The conditions live in the dialog's `when` field (command id →
//! text) until OK, like the keys.

use std::collections::BTreeMap;

use egui::{Align2, FontId, Id, Key, Modifiers, Order, PopupCloseBehavior, Rect, RichText, Sense, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use photocraft_engine::when::keys::{KEYS, Kind};
use photocraft_engine::when::suggest::{self, ItemKind};
use photocraft_engine::when::{Chip, Expr, expr, parse};

use crate::theme::Tokens;

/// The least width of a When cell; it takes the rest of the row when the dialog is wider.
const CELL_W: f32 = 180.0;
/// The width of the `+` button after it.
const PLUS_W: f32 = 24.0;
/// The width of the warning icon before it.
const WARN_W: f32 = 18.0;
const MAX_ITEMS: usize = 40;
/// Rows of the list that show before it scrolls.
const LIST_ROWS: f32 = 10.0;
const LIST_ROW_H: f32 = 22.0;

fn field_id() -> Id {
    Id::unique("pc-shortcut-when-field")
}

/// A condition being typed (the dialog's `whenEdit` field).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct WhenEdit {
    pub id: String,
    pub text: String,
    /// The highlighted suggestion.
    pub selected: usize,
    /// The arrow keys moved the highlight: Enter takes the suggestion rather than saving.
    pub navigated: bool,
    /// Return or leaving the box was tried with text that is not a condition: show every problem.
    pub tried: bool,
    /// First frame: take the focus.
    pub fresh: bool,
}

pub(super) fn load(f: &Map<String, Value>) -> Option<WhenEdit> {
    f.get("whenEdit").and_then(|v| serde_json::from_value(v.clone()).ok())
}

pub(super) fn store(f: &mut Map<String, Value>, edit: &Option<WhenEdit>) {
    f.insert("whenEdit".into(), edit.as_ref().map_or(Value::Null, |e| json!(e)));
}

/// The conditions as the dialog stands.
pub(super) fn load_map(f: &Map<String, Value>) -> BTreeMap<String, String> {
    f.get("when").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

fn chip_fill(t: &Tokens, negated: bool, warn: bool) -> egui::Color32 {
    if warn {
        t.warning.gamma_multiply(0.28)
    } else if negated {
        t.danger.gamma_multiply(0.26)
    } else {
        t.accent.gamma_multiply(0.24)
    }
}

/// Draw the chips of `expr` from `left`.
fn draw_chips(painter: &egui::Painter, t: &Tokens, expr: &Expr, left: f32, cy: f32, warn: bool) {
    let mut x = left;
    for chip in expr.chips() {
        match chip {
            Chip::Atom(text) => {
                let galley = painter.layout_no_wrap(tl!(&text).to_string(), crate::theme::medium(11.0), t.text);
                let w = galley.size().x + 12.0;
                let r = Rect::from_min_size(pos2(x, cy - 9.0), vec2(w, 18.0));
                painter.rect_filled(r, 9.0, chip_fill(t, text.starts_with("Not "), warn));
                painter.galley(pos2(x + 6.0, cy - galley.size().y / 2.0), galley, t.text);
                x += w + 4.0;
            }
            Chip::Word(w) => {
                let r = painter.text(pos2(x, cy), Align2::LEFT_CENTER, tl!(w), crate::theme::medium(10.5), t.text_dim);
                x += r.width() + 4.0;
            }
            Chip::Open | Chip::Close => {
                let c = if matches!(chip, Chip::Open) { "(" } else { ")" };
                let r = painter.text(pos2(x, cy), Align2::LEFT_CENTER, c, crate::theme::medium(11.5), t.text_dim);
                x += r.width() + 2.0;
            }
        }
    }
}

/// The When cell of command `id` (named `name`): the condition as chips, "Always" when there is
/// none; a click edits it, `+` adds a part.
pub(super) fn cell(
    ui: &mut egui::Ui,
    t: &Tokens,
    (id, name, clash): (&str, &str, Option<&String>),
    whens: &mut BTreeMap<String, String>,
    edit: &mut Option<WhenEdit>,
    selected: &mut String,
) {
    let text = whens.get(id).cloned();
    let parsed = text.as_deref().and_then(|s| parse(s).ok().flatten());
    let never = parsed.as_ref().is_some_and(|e| !expr::satisfiable(e));
    let editing = edit.as_ref().is_some_and(|e| e.id == id);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let width = (ui.available_width() - PLUS_W - WARN_W - ui.spacing().item_spacing.x * 3.0).max(CELL_W);
        let (rect, r) = ui.allocate_exact_size(vec2(width, 20.0), Sense::click());
        let label = crate::i18n::fmt(tl!("Condition of {command}"), &[("command", name)]);
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
        if r.hovered() || editing {
            ui.painter().rect_filled(rect.shrink2(vec2(0.0, 1.0)), 4.0, t.hover);
        }
        let painter = ui.painter().with_clip_rect(rect);
        match (&parsed, &text) {
            (Some(e), _) => draw_chips(&painter, t, e, rect.left() + 4.0, rect.center().y, never),
            (None, Some(_)) => {
                _ = painter.text(pos2(rect.left() + 6.0, rect.center().y), Align2::LEFT_CENTER, tl!("Unreadable"), crate::theme::medium(11.5), t.danger)
            }
            (None, None) => {
                _ = painter.text(pos2(rect.left() + 6.0, rect.center().y), Align2::LEFT_CENTER, tl!("Always"), crate::theme::medium(11.5), t.text_faint)
            }
        }
        let mut tip = text.clone().unwrap_or_else(|| tl!("Works everywhere. Click to limit it to a context.").to_string());
        if never {
            tip.push('\n');
            tip.push_str(tl!("This condition can never be true, so the shortcut never works."));
        }
        if r.on_hover_text(tip).clicked() {
            *selected = id.to_string();
            *edit = Some(WhenEdit { id: id.to_string(), text: text.clone().unwrap_or_default(), fresh: true, ..Default::default() });
        }
        // a clash with another command in the same context: a small warning, the sentence on hover
        let (warn, wr) = ui.allocate_exact_size(vec2(WARN_W, 20.0), Sense::hover());
        if let Some(sentence) = clash {
            crate::icons::paint(ui, warn, "triangle-alert", 14.0, t.warning);
            wr.on_hover_text(sentence);
        }
        let plus = crate::icons::button(ui, "plus", 20.0, false, tl!("Add Condition"));
        let state_id = Id::unique("pc-when-builder").with(id);
        let mut chosen: Option<Expr> = None;
        let popup = egui::Popup::menu(&plus).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
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
            ui.data_mut(|d| d.remove_temp::<Builder>(state_id));
        }
        if let Some(added) = chosen {
            let combined = match parsed {
                Some(e) => e.and(added),
                None => added,
            };
            *selected = id.to_string();
            whens.insert(id.to_string(), combined.text());
        }
    });
}

/// Where the builder is: no key picked yet, or the key and whether the comparison is negated.
#[derive(Clone, Copy, Default)]
struct Builder {
    key: Option<usize>,
    negated: bool,
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

/// The builder's two steps. The condition the user finished, if they did.
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
    // Is / Is Not, for every key (an on / off key too: "Is Not Typing")
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

// ------------------------------------------------------------------ typing

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

/// What the box shows under it: the problem, in red.
fn problem(edit: &WhenEdit) -> Option<String> {
    match parse(&edit.text) {
        Err(e) if !e.incomplete || edit.tried => Some(e.message),
        _ => None,
    }
}

fn take_suggestion(ctx: &egui::Context, edit: &mut WhenEdit, sug: &suggest::Suggestions, i: usize) {
    let Some(item) = sug.items.get(i) else { return };
    let (text, cursor) = suggest::apply(&edit.text, sug, item);
    edit.text = text;
    edit.selected = 0;
    edit.navigated = false;
    edit.tried = false;
    set_cursor(ctx, field_id(), &edit.text, cursor);
    ctx.request_repaint();
}

/// The arrow keys, Tab and Return of the list, while the box is focused. The suggestion to take.
fn list_keys(ctx: &egui::Context, edit: &mut WhenEdit, count: usize) -> Option<usize> {
    let (down, up, tab, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Tab),
            edit.navigated && i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if down {
        edit.selected = (edit.selected + 1) % count;
        edit.navigated = true;
    }
    if up {
        edit.selected = (edit.selected + count - 1) % count;
        edit.navigated = true;
    }
    (tab || enter).then_some(edit.selected)
}

/// The suggestion list, above the box. The row clicked, if one was.
fn list(ctx: &egui::Context, t: &Tokens, edit: &WhenEdit, field_rect: Rect, sug: &suggest::Suggestions, count: usize) -> Option<usize> {
    let problem_now = problem(edit);
    let unsatisfiable = parse(&edit.text).ok().flatten().is_some_and(|e| !expr::satisfiable(&e));
    if count == 0 && problem_now.is_none() && !unsatisfiable {
        return None;
    }
    let mut clicked = None;
    let area = egui::Area::new(Id::unique("pc-when-list")).order(Order::Tooltip).pivot(Align2::LEFT_BOTTOM).fixed_pos(field_rect.left_top() - vec2(0.0, 3.0));
    let shown = area.show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            ui.set_min_width(field_rect.width().max(400.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(message) = &problem_now {
                ui.label(RichText::new(message).size(11.5).color(t.danger));
                ui.add_space(3.0);
            } else if unsatisfiable {
                ui.label(RichText::new(tl!("This condition can never be true.")).size(11.5).color(t.warning));
                ui.add_space(3.0);
            }
            let list_h = (count as f32).min(LIST_ROWS) * LIST_ROW_H;
            egui::ScrollArea::vertical().max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, true]).id_salt("when-list-scroll").show(ui, |ui| {
                for (i, item) in sug.items.iter().take(count).enumerate() {
                    let (row, r) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &item.label));
                    if i == edit.selected {
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
                    if i == edit.selected && edit.navigated {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    });
    ctx.data_mut(|d| d.insert_temp(list_rect_id(), shown.response.rect));
    clicked
}

/// The strip under the list while a condition is being typed: the box, its suggestions above it,
/// the reason it is not valid under it. Return or leaving the box saves a valid condition;
/// Esc drops the edit.
pub(super) fn editor(ui: &mut egui::Ui, t: &Tokens, name: &str, whens: &mut BTreeMap<String, String>, slot: &mut Option<WhenEdit>) {
    let ctx = ui.ctx().clone();
    let Some(mut edit) = slot.take() else { return };
    let field = field_id();
    let sug = suggest::suggest(&edit.text, cursor_byte(&ctx, field, &edit.text));
    let count = sug.items.len().min(MAX_ITEMS);
    edit.selected = edit.selected.min(count.saturating_sub(1));
    if ui.memory(|m| m.has_focus(field))
        && count > 0
        && let Some(i) = list_keys(&ctx, &mut edit, count)
    {
        take_suggestion(&ctx, &mut edit, &sug, i);
    }
    let before = edit.text.clone();
    let mut text = std::mem::take(&mut edit.text);
    let shown = problem(&edit);
    let out = ui
        .horizontal(|ui| {
            ui.label(RichText::new(crate::i18n::fmt(tl!("When {command}:"), &[("command", name)])).size(11.5).color(t.text_dim));
            let mut te = egui::TextEdit::singleline(&mut text).id(field).font(FontId::monospace(11.5)).hint_text(tl!("Always")).desired_width(380.0);
            if shown.is_some() {
                te = te.text_color(t.danger);
            }
            ui.add(te)
        })
        .inner;
    edit.text = text;
    if edit.fresh {
        // the first frame of an edit: the box takes the keyboard, cursor at the end
        edit.fresh = false;
        out.request_focus();
        set_cursor(&ctx, field, &edit.text, edit.text.len());
    }
    if edit.text != before {
        edit.selected = 0;
        edit.navigated = false;
        edit.tried = false;
    }
    // Tab and the arrows belong to the list while it is open (egui would move the focus with them)
    if out.has_focus() && count > 0 {
        let filter = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false };
        ctx.memory_mut(|m| m.set_focus_lock_filter(field, filter));
    }
    let sug = suggest::suggest(&edit.text, cursor_byte(&ctx, field, &edit.text));
    let count = sug.items.len().min(MAX_ITEMS);
    let clicked = if out.has_focus() || out.lost_focus() { list(&ctx, t, &edit, out.rect, &sug, count) } else { None };
    if let Some(i) = clicked {
        take_suggestion(&ctx, &mut edit, &sug, i);
        ctx.memory_mut(|m| m.request_focus(field));
        *slot = Some(edit);
        return;
    }
    // Return is this box's: it must not also click the dialog's OK
    if out.lost_focus() {
        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    }
    if out.lost_focus() {
        let (esc, pointer) = ctx.input(|i| (i.key_pressed(Key::Escape), i.pointer.interact_pos().or(i.pointer.latest_pos())));
        let in_list = list_rect(&ctx).zip(pointer).is_some_and(|(r, p)| r.contains(p));
        if esc {
            // Esc ends the edit; the dialog stays
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            return;
        }
        if in_list {
            ctx.memory_mut(|m| m.request_focus(field));
        } else if let Ok(parsed) = parse(&edit.text) {
            match parsed.map(|e| e.text()) {
                Some(canonical) => _ = whens.insert(edit.id.clone(), canonical),
                None => _ = whens.remove(&edit.id),
            }
            return;
        } else {
            edit.tried = true;
            ctx.memory_mut(|m| m.request_focus(field));
        }
    }
    *slot = Some(edit);
}

fn list_rect_id() -> Id {
    Id::unique("pc-when-list-rect")
}

/// Where the suggestion list was drawn last frame (a click in it is not leaving the box).
fn list_rect(ctx: &egui::Context) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(list_rect_id()))
}

/// A line under the list when the settings hold conditions that could not be read: they are ignored.
pub(super) fn damaged_note(ui: &mut egui::Ui, t: &Tokens, whens: &BTreeMap<String, String>) {
    let damaged = photocraft_engine::prefs::damaged_when(whens);
    if let Some((id, text, why)) = damaged.first() {
        let name = crate::shortcut_dispatch::label(id);
        let message = crate::i18n::fmt(
            tl!("The condition of {command} could not be read and is ignored (“{text}”): {why}"),
            &[("command", &name), ("text", text), ("why", why)],
        );
        ui.label(RichText::new(message).size(11.5).color(t.danger));
    }
}

type Item = (String, String, Vec<String>, Option<String>);

fn name_of(items: &[Item], id: &str) -> String {
    let label = items.iter().find(|i| i.0 == id).map_or(id, |i| i.1.as_str());
    crate::i18n::tr_id(crate::i18n::current(), id, label).trim_end_matches('…').to_string()
}

/// The strip under the list: the box while a condition is typed, and the note about conditions
/// that could not be read. Only on the Keyboard Shortcuts tab.
pub(super) fn strip(ui: &mut egui::Ui, t: &Tokens, tab: u64, items: &[Item], whens: &mut BTreeMap<String, String>, slot: &mut Option<WhenEdit>) {
    if tab != 0 {
        *slot = None;
        return;
    }
    damaged_note(ui, t, whens);
    let name = slot.as_ref().map(|e| name_of(items, &e.id)).unwrap_or_default();
    editor(ui, t, &name, whens, slot);
}

/// The When column's header, a dim "When" with what a condition does and doesn't limit.
pub(super) fn header(ui: &mut egui::Ui, t: &Tokens, tab: u64) {
    if tab == 0 {
        ui.label(RichText::new(tl!("When")).size(11.0).color(t.text_faint))
            .on_hover_text(tl!("Limits this command's keys to a context. Menus and the palette still run it."));
    } else {
        ui.label("");
    }
}

/// A key as the dialog holds it for a command: its edited main key (`""` removes it) or its
/// default, then its further keys, all canonical.
fn keys_of(overrides: &BTreeMap<String, String>, alternates: &BTreeMap<String, Vec<String>>, id: &str, default: Option<&str>) -> Vec<String> {
    use photocraft_engine::prefs::{default_alternates, normalize_shortcut};
    let main = match overrides.get(id) {
        Some(s) if s.is_empty() => None,
        Some(s) => normalize_shortcut(s),
        None => default.and_then(normalize_shortcut),
    };
    let mut keys: Vec<String> = main.iter().cloned().collect();
    let further: Vec<String> = match alternates.get(id) {
        Some(list) => list.iter().filter_map(|k| normalize_shortcut(k)).collect(),
        None => default_alternates(id).filter_map(normalize_shortcut).collect(),
    };
    for k in further {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys
}

/// What clashes with what, for the warning icons: command id → the sentence for its tooltip. Only
/// clashes that involve a condition or a chord count (a default key two commands share is not
/// this feature's business), and two commands whose conditions can't hold together never clash.
pub(super) fn clash_notes(
    items: &[Item],
    (overrides, alternates): (&BTreeMap<String, String>, &BTreeMap<String, Vec<String>>),
    chords: &BTreeMap<String, String>,
    whens: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut notes: BTreeMap<String, String> = BTreeMap::new();
    if whens.is_empty() && chords.is_empty() {
        return notes;
    }
    let mut by_key: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (id, _, _, def) in items {
        for k in keys_of(overrides, alternates, id, def.as_deref()) {
            by_key.entry(k).or_default().push(id);
        }
    }
    let involved = |id: &str| whens.contains_key(id) || chords.contains_key(id);
    let is_tool = |id: &str| crate::tool_shortcuts::tool(id).is_some();
    let mut note = |id: &str, other: &str, key: &str, leader: bool| {
        let template = if leader {
            tl!("{key} is also the shortcut of {command}: it waits for the chord's second key.")
        } else {
            tl!("{key} is also used by {command} in the same context.")
        };
        let text = crate::i18n::fmt(template, &[("key", &super::shortcuts_chord::shown(key)), ("command", &name_of(items, other))]);
        notes.entry(id.to_string()).or_insert(text);
    };
    for (key, ids) in &by_key {
        for (i, a) in ids.iter().enumerate() {
            for b in ids.iter().skip(i + 1) {
                if (involved(a) || involved(b)) && !(is_tool(a) && is_tool(b)) && prefs_overlap(whens, a, b) {
                    note(a, b, key, false);
                    note(b, a, key, false);
                }
            }
        }
    }
    let mut by_chord: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, c) in chords {
        by_chord.entry(c).or_default().push(id);
        let Some([first, _]) = photocraft_engine::prefs::chord_steps(c) else { continue };
        for other in by_key.get(&first).into_iter().flatten().filter(|o| **o != id && prefs_overlap(whens, id, o)) {
            note(id, other, &first, true);
        }
    }
    for (c, ids) in by_chord {
        for (i, a) in ids.iter().enumerate() {
            for b in ids.iter().skip(i + 1).filter(|b| prefs_overlap(whens, a, b)) {
                note(a, b, c, false);
                note(b, a, c, false);
            }
        }
    }
    notes
}

fn prefs_overlap(whens: &BTreeMap<String, String>, a: &str, b: &str) -> bool {
    photocraft_engine::prefs::can_overlap(whens, a, b)
}

/// A suggestion's description cut to a short hint: up to its first parenthesis, at most 36 characters.
fn short(detail: &str) -> String {
    let head = detail.split(" (").next().unwrap_or(detail);
    if head.chars().count() <= 36 { head.to_string() } else { format!("{}…", head.chars().take(35).collect::<String>().trim_end()) }
}

#[cfg(test)]
pub(super) fn short_for_test(detail: &str) -> String {
    short(detail)
}
