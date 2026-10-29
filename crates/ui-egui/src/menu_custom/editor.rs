//! Preferences › Context Menus: pick a menu, then its rows in order on the left (Shown) and what can
//! be added on the right (Available: the menu's own rows that are out, then every command of the
//! menu bar), with a search box.
//!
//! - Drag a row up or down to move it; drag an item from the right into the list to put it there.
//! - ↑ ↓ on the picked row move it, × takes it out; + on the right (or a double-click) adds the item
//!   below the picked row, or at the end. The separator button above does the same with a line.
//! - Keys, with a row picked and no text box in use: ↑ / ↓ pick another, ⌥↑ / ⌥↓ move it, Delete or
//!   Backspace takes it out. Opening the page picks nothing.
//! - The reset button puts the menu back as it came.
//!
//! The page edits a draft held by the Preferences dialog ([`super::DRAFT`]): OK and Apply save it,
//! Cancel drops it. The search text and the picked row live in egui's memory: they are the page's,
//! not settings.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use egui::{Color32, Key, Modifiers, Rect, Sense, Stroke, pos2, vec2};
use serde_json::Value;

use super::menus::{self, MENUS};
use super::{Arrangement, Custom, Slot, catalog, presets_ui};
use crate::PhotocraftApp;
use crate::theme::Tokens;

const ROW_H: f32 = 26.0;
/// The least height of the lists; in a window with less room the page scrolls.
const MIN_LIST_H: f32 = 60.0;
/// The most: the page itself is never taller than the Preferences dialog's content.
const MAX_LIST_H: f32 = 1400.0;
/// What the page takes under the lists: a line of margin.
const BELOW_LIST: f32 = 8.0;
const GAP: f32 = 12.0;
/// The inner margin of a list's frame.
const FRAME_MARGIN: f32 = 4.0;
/// The share of the width the shown list takes; the rest is the list to add from.
const LEFT_SHARE: f32 = 0.54;
/// The most rows of Available drawn for a search that matches many commands.
const MAX_AVAILABLE: usize = 400;

/// What is being dragged: a row of the menu (its index), or an item to add.
#[derive(Clone, Debug)]
enum Drag {
    Row(usize),
    Add(String),
}

/// The page's own state: the menu being arranged, the search text and the row picked on the left.
#[derive(Clone, Debug, Default)]
struct View {
    menu: usize,
    query: String,
    picked: Option<usize>,
    /// Scroll the picked row into view on the next draw (it was just added or moved by key).
    reveal: bool,
    /// The height of the lists, from the room the page has.
    list_h: f32,
    /// The pass the page was last drawn in: when the page opens again nothing is picked.
    drawn: u64,
    /// The offers last built, for the menu, search and shown rows they were built for.
    offers: Option<OffersCache>,
}

#[derive(Clone, Debug)]
struct OffersCache {
    key: &'static str,
    query: String,
    shown: u64,
    offers: Arc<Vec<Offer>>,
}

/// One thing the right-hand list offers.
#[derive(Clone, Debug)]
pub struct Available {
    pub id: &'static str,
    pub label: &'static str,
    /// The group it is listed under: `This Menu`, or the menu bar's menu (`Layer`).
    pub group: String,
    /// Where the menu bar has it (`Layer › Layer Style`), for its tooltip; empty for the menu's own rows.
    pub path: String,
}

/// A line of the right-hand list: a group's name, or one offer.
#[derive(Clone, Debug)]
enum Offer {
    Header(String),
    Item(Available),
}

/// The slots menu `key` shows now: the arrangement, or all its rows when it has none.
pub fn slots(custom: &Custom, key: &str) -> Vec<Slot> {
    match custom.menus.get(key) {
        Some(a) => a.slots(),
        None => default_slots(key),
    }
}

pub fn default_slots(key: &str) -> Vec<Slot> {
    menus::rows(key).into_iter().map(|row| row.map_or(Slot::Separator, |(_, id)| Slot::Cmd(id.to_string()))).collect()
}

/// Make `slots` menu `key`'s arrangement; all its rows in the order it builds them is no arrangement.
pub fn set_slots(custom: &mut Custom, key: &str, slots: &[Slot]) {
    let items: Vec<String> = slots.iter().map(Slot::key).collect();
    let default: Vec<String> = default_slots(key).iter().map(Slot::key).collect();
    if items == default {
        custom.menus.remove(key);
        return;
    }
    let shown: std::collections::HashSet<&str> = slots.iter().filter_map(|s| if let Slot::Cmd(id) = s { Some(id.as_str()) } else { None }).collect();
    let hidden = menus::universe(key).into_iter().filter(|id| !shown.contains(id)).map(str::to_string).collect();
    custom.menus.insert(key.to_string(), Arrangement { items, hidden });
}

/// What can be added to menu `key` showing `shown`, matching `query` (every word in the label or
/// group or path, any case): the menu's own rows not in it, then the menu-bar commands not in it.
pub fn available(key: &str, shown: &[Slot], query: &str) -> Vec<Available> {
    let in_menu: std::collections::HashSet<&str> = shown.iter().filter_map(|s| if let Slot::Cmd(id) = s { Some(id.as_str()) } else { None }).collect();
    let own = menus::universe(key);
    let mut out: Vec<Available> = own
        .iter()
        .filter(|id| !in_menu.contains(**id))
        .filter_map(|id| Some(Available { id, label: menus::label(key, id)?, group: THIS_MENU.to_string(), path: String::new() }))
        .collect();
    if menus::can_add(key) {
        out.extend(catalog::all().iter().filter(|c| !in_menu.contains(c.id) && !own.contains(&c.id)).map(|c| Available {
            id: c.id,
            label: c.label,
            group: c.path.split(" › ").next().filter(|g| !g.is_empty()).unwrap_or(OTHER).to_string(),
            path: c.path.clone(),
        }));
    }
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    if !words.is_empty() {
        out.retain(|a| {
            let hay = format!("{} {} {} {}", a.label, tl!(a.label), a.group, a.path).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        });
    }
    out
}

const THIS_MENU: &str = "This Menu";
const OTHER: &str = "Other";

/// `items` under their group's header, groups in the order they first appear, at most
/// [`MAX_AVAILABLE`] items.
fn grouped(items: Vec<Available>) -> Vec<Offer> {
    let mut groups: Vec<(String, Vec<Available>)> = Vec::new();
    for item in items.into_iter().take(MAX_AVAILABLE) {
        match groups.iter_mut().find(|(g, _)| *g == item.group) {
            Some((_, list)) => list.push(item),
            None => groups.push((item.group.clone(), vec![item])),
        }
    }
    groups.into_iter().flat_map(|(group, list)| std::iter::once(Offer::Header(group)).chain(list.into_iter().map(Offer::Item))).collect()
}

fn shown_hash(shown: &[Slot]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    shown.hash(&mut h);
    h.finish()
}

/// A row's label in the list: the menu's own, else the menu bar's, else the id (a command this
/// build does not have).
fn row_label(key: &str, slot: &Slot) -> String {
    match slot {
        Slot::Separator => String::new(),
        Slot::Cmd(id) => menus::label(key, id).or_else(|| catalog::find(id).map(|c| c.label)).map_or_else(|| id.clone(), |l| tl!(l).to_string()),
    }
}

fn known(key: &str, id: &str) -> bool {
    menus::label(key, id).is_some() || catalog::find(id).is_some()
}

/// The note a row carries: when it shows.
fn note(key: &str, slot: &Slot) -> Option<String> {
    let Slot::Cmd(id) = slot else { return None };
    if !known(key, id) {
        return Some(tl!("not available").to_string());
    }
    let when = menus::when(key, id);
    (!when.is_empty()).then(|| tl!(when).to_string())
}

/// The height of the lists: what `height` has left below `used` (what the page holds above them),
/// never under [`MIN_LIST_H`]: a page with less room scrolls instead.
pub fn list_height(height: f32, used: f32) -> f32 {
    (height - used - BELOW_LIST).clamp(MIN_LIST_H, MAX_LIST_H)
}

/// A small icon button for the page's tool row; true when clicked. A disabled one is dimmed.
fn tool_button(ui: &mut egui::Ui, t: &Tokens, icon: &str, tip: &str, enabled: bool) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(26.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
    if enabled && r.hovered() {
        ui.painter().rect_filled(rect, t.radius_sm, t.hover);
    }
    let color = if !enabled {
        t.text_faint
    } else if r.hovered() {
        t.text
    } else {
        t.icon
    };
    crate::icons::paint(ui, rect, icon, 16.0, color);
    r.on_hover_text(tip).clicked()
}

/// The page, `height` points tall (what the Preferences dialog gives its content), editing `draft`
/// (the dialog's copy of the arrangement).
pub fn page(app: &mut PhotocraftApp, ui: &mut egui::Ui, height: f32, draft: &mut Value) {
    let t = Tokens::get(ui.ctx());
    let origin = ui.cursor().top();
    let view_id = egui::Id::unique("context-menus-page");
    let mut view: View = ui.ctx().data(|d| d.get_temp(view_id)).unwrap_or_default();
    let pass = ui.ctx().cumulative_pass_nr();
    if pass > view.drawn.saturating_add(1) {
        view.picked = None;
    }
    view.menu = view.menu.min(MENUS.len() - 1);
    let mut custom = Custom::from_value(draft);
    let before = custom.clone();
    let mut key = MENUS[view.menu].0;
    let mut menu = slots(&custom, key);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!(super::PAGE_TITLE)).font(crate::theme::semibold(14.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| presets_ui::show(app, ui, &mut custom, &t));
    });
    // A preset may have changed this menu.
    if custom != before {
        menu = slots(&custom, key);
    }
    let mut menu_before = menu.clone();
    ui.add_space(4.0);
    let mut chosen = view.menu;
    ui.horizontal(|ui| {
        let options: Vec<(usize, &str)> = MENUS.iter().enumerate().map(|(i, (_, title))| (i, *title)).collect();
        crate::widgets::dropdown(ui, "context-menus-menu", tl!("Menu"), &mut chosen, &options, 170.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if tool_button(ui, &t, "undo-2", tl!("Reset"), custom.menus.contains_key(key)) {
                menu = default_slots(key);
                view.picked = None;
            }
            if tool_button(ui, &t, "minus", tl!("Add Separator"), true) {
                insert(&mut menu, &mut view, Slot::Separator);
            }
        });
    });
    // Another menu picked: the lists below show it.
    if chosen != view.menu && chosen < MENUS.len() {
        view.menu = chosen;
        view.picked = None;
        key = MENUS[chosen].0;
        menu = slots(&custom, key);
        menu_before = menu.clone();
    }
    ui.add_space(6.0);

    // The "Shown" heading and the list frame's margins sit above and around the lists too: leaving
    // them out made the page taller than it was given, so the dialog grew back after every resize.
    let around_lists = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y + 2.0 * (FRAME_MARGIN + 1.0);
    view.list_h = list_height(height, ui.cursor().top() - origin + around_lists);
    let width = ui.available_width() - 2.0;
    let left_w = ((width - GAP) * LEFT_SHARE).floor();
    let right_w = (width - GAP - left_w).max(120.0);
    let mut left_top = 0.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        ui.vertical(|ui| {
            ui.set_width(left_w);
            ui.label(egui::RichText::new(tl!("Shown")).color(t.text_dim));
            left_top = ui.cursor().top();
            shown_list(ui, &t, key, &mut menu, &mut view, left_w);
        });
        ui.vertical(|ui| {
            ui.set_width(right_w);
            add_list(ui, &t, key, &mut menu, &mut view, (right_w, left_top));
        });
    });
    keys(ui, &mut menu, &mut view);
    if menu != menu_before {
        set_slots(&mut custom, key, &menu);
    }
    if custom != before {
        *draft = custom.to_value();
    }
    view.drawn = pass;
    ui.ctx().data_mut(|d| d.insert_temp(view_id, view));
}

/// Put `slot` below the picked row (at the end when none is picked) and pick it.
fn insert(menu: &mut Vec<Slot>, view: &mut View, slot: Slot) {
    let at = view.picked.map_or(menu.len(), |p| (p + 1).min(menu.len()));
    menu.insert(at, slot);
    view.picked = Some(at);
    view.reveal = true;
}

/// Move the row at `from` so it lands before the row that was at `to` (`to` may be the length: the end).
fn move_row(menu: &mut Vec<Slot>, view: &mut View, from: usize, to: usize) {
    if from >= menu.len() || to > menu.len() || to == from || to == from + 1 {
        return;
    }
    let slot = menu.remove(from);
    let at = if to > from { to - 1 } else { to };
    menu.insert(at, slot);
    view.picked = Some(at);
}

fn step(menu: &mut [Slot], view: &mut View, at: usize, up: bool) {
    let to = if up { at.checked_sub(1) } else { Some(at + 1) };
    if let Some(to) = to.filter(|to| *to < menu.len()) {
        menu.swap(at, to);
        view.picked = Some(to);
        view.reveal = true;
    }
}

fn remove(menu: &mut Vec<Slot>, view: &mut View, at: usize) {
    if at >= menu.len() {
        return;
    }
    menu.remove(at);
    view.picked = match view.picked {
        Some(p) if p == at => (!menu.is_empty()).then(|| at.min(menu.len() - 1)),
        Some(p) if p > at => Some(p - 1),
        p => p,
    };
}

/// The six dots of a drag handle.
fn grip(p: &egui::Painter, centre: egui::Pos2, color: Color32) {
    for dx in [-2.5, 2.5] {
        for dy in [-4.0, 0.0, 4.0] {
            p.circle_filled(centre + vec2(dx, dy), 1.2, color);
        }
    }
}

/// A small icon button in a row; true when clicked.
fn icon_button(ui: &mut egui::Ui, t: &Tokens, rect: Rect, id: egui::Id, icon: &str, tip: &str) -> bool {
    let r = ui.interact(rect, id, Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    if r.hovered() {
        ui.painter().rect_filled(rect, t.radius_sm, t.pressed);
    }
    crate::icons::paint(ui, rect, icon, 12.0, if r.hovered() { t.text } else { t.text_dim });
    r.on_hover_text(tip).clicked()
}

fn truncated(ui: &egui::Ui, text: String, font: egui::FontId, color: Color32, max: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text, egui::TextFormat::simple(font, color));
    job.wrap = egui::text::TextWrapping::truncate_at_width(max);
    ui.fonts_mut(|f| f.layout_job(job))
}

fn frame(t: &Tokens) -> egui::Frame {
    egui::Frame::new().fill(t.field).stroke(Stroke::new(1.0, t.card_border)).corner_radius(6.0).inner_margin(FRAME_MARGIN)
}

fn shown_list(ui: &mut egui::Ui, t: &Tokens, key: &str, menu: &mut Vec<Slot>, view: &mut View, width: f32) {
    let list = frame(t).show(ui, |ui| {
        ui.set_width(width - 10.0);
        egui::ScrollArea::vertical().id_salt("context-menu-rows").max_height(view.list_h).min_scrolled_height(view.list_h).auto_shrink([false, false]).show(
            ui,
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut rects = Vec::with_capacity(menu.len());
                // (row, what): 0 picked, 1 removed, 2 moved up, 3 moved down
                let mut action: Option<(usize, u8)> = None;
                for (i, slot) in menu.iter().enumerate() {
                    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
                    rects.push(rect);
                    let picked = view.picked == Some(i);
                    let dragging_this = matches!(egui::DragAndDrop::payload::<Drag>(ui.ctx()).as_deref(), Some(Drag::Row(d)) if *d == i);
                    let p = ui.painter();
                    if picked {
                        p.rect_filled(rect, 4.0, t.row_selected);
                    } else if r.hovered() && !egui::DragAndDrop::has_any_payload(ui.ctx()) {
                        p.rect_filled(rect, 4.0, t.hover);
                    }
                    let dim = if dragging_this { 0.35 } else { 1.0 };
                    grip(p, pos2(rect.left() + 9.0, rect.center().y), t.text_dim.gamma_multiply(dim));
                    let buttons = ui.rect_contains_pointer(rect) || picked;
                    let mut text_right = rect.right() - 6.0;
                    if buttons {
                        text_right -= 3.0 * 20.0;
                    }
                    if *slot == Slot::Separator {
                        let y = rect.center().y;
                        p.line_segment([pos2(rect.left() + 22.0, y), pos2(text_right, y)], Stroke::new(1.0, t.text_dim.gamma_multiply(0.6 * dim)));
                    } else {
                        if let Some(n) = note(key, slot) {
                            let g = p.layout_no_wrap(n, egui::FontId::proportional(10.5), t.text_dim.gamma_multiply(dim));
                            text_right -= g.size().x;
                            p.galley(pos2(text_right, rect.center().y - g.size().y / 2.0), g, t.text_dim);
                            text_right -= 6.0;
                        }
                        let color = match slot {
                            Slot::Cmd(id) if !known(key, id) => t.text_faint,
                            _ => t.text,
                        };
                        let g = truncated(
                            ui,
                            row_label(key, slot),
                            egui::FontId::proportional(12.5),
                            color.gamma_multiply(dim),
                            (text_right - rect.left() - 22.0).max(10.0),
                        );
                        ui.painter().galley(pos2(rect.left() + 22.0, rect.center().y - g.size().y / 2.0), g, color);
                    }
                    if buttons {
                        let at = |n: f32| Rect::from_center_size(pos2(rect.right() - 10.0 - n * 20.0, rect.center().y), vec2(18.0, 18.0));
                        let scope = ui.scope_id();
                        let id = |what: &str| scope.with((what, i));
                        if icon_button(ui, t, at(0.0), id("remove"), "x", tl!("Take out of the menu")) {
                            action = Some((i, 1));
                        }
                        if icon_button(ui, t, at(1.0), id("down"), "chevron-down", tl!("Move down")) {
                            action = Some((i, 3));
                        }
                        if icon_button(ui, t, at(2.0), id("up"), "chevron-up", tl!("Move up")) {
                            action = Some((i, 2));
                        }
                    }
                    let name = if *slot == Slot::Separator { tl!("Separator").to_string() } else { row_label(key, slot) };
                    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, picked, name.clone()));
                    if r.clicked() {
                        action = action.or(Some((i, 0)));
                    }
                    if r.drag_started() {
                        egui::DragAndDrop::set_payload(ui.ctx(), Drag::Row(i));
                    }
                    if picked && view.reveal {
                        r.scroll_to_me(None);
                        view.reveal = false;
                    }
                }
                edge_scroll(ui);
                match action {
                    Some((i, 1)) => remove(menu, view, i),
                    Some((i, 2)) => step(menu, view, i, true),
                    Some((i, 3)) => step(menu, view, i, false),
                    Some((i, _)) => view.picked = Some(i),
                    None => {}
                }
                rects
            },
        )
    });
    let rects = list.inner.inner;
    drop_into(ui, t, key, menu, view, list.response.rect, &rects);
}

/// While a row or an item is dragged near the top or bottom of the list, the list scrolls that
/// way, so a row can be taken anywhere in a menu taller than the list.
fn edge_scroll(ui: &egui::Ui) {
    const EDGE: f32 = 28.0;
    const STEP: f32 = 8.0;
    if !egui::DragAndDrop::has_payload_of_type::<Drag>(ui.ctx()) {
        return;
    }
    let Some(p) = ui.ctx().pointer_latest_pos() else { return };
    let clip = ui.clip_rect();
    if !clip.x_range().contains(p.x) {
        return;
    }
    let dy = if (clip.top() - EDGE..clip.top() + EDGE).contains(&p.y) {
        STEP
    } else if (clip.bottom() - EDGE..clip.bottom() + EDGE).contains(&p.y) {
        -STEP
    } else {
        return;
    };
    ui.scroll_with_delta(vec2(0.0, dy));
    ui.ctx().request_repaint();
}

/// While something is dragged over the list: a line where it would go, and the move or insert when
/// it is let go there. A drag let go anywhere else changes nothing.
fn drop_into(ui: &mut egui::Ui, t: &Tokens, key: &str, menu: &mut Vec<Slot>, view: &mut View, list_rect: Rect, rects: &[Rect]) {
    let ctx = ui.ctx().clone();
    let Some(drag) = egui::DragAndDrop::payload::<Drag>(&ctx) else { return };
    let Some(pointer) = ctx.pointer_latest_pos() else { return };
    // the thing dragged follows the pointer
    let label = match drag.as_ref() {
        Drag::Row(i) => menu.get(*i).cloned(),
        Drag::Add(k) => Slot::parse(k),
    }
    .map(|s| if s == Slot::Separator { tl!("Separator").to_string() } else { row_label(key, &s) });
    if let Some(label) = label {
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("context-menus-drag")));
        let g = p.layout_no_wrap(label, egui::FontId::proportional(12.5), t.text);
        let r = Rect::from_min_size(pointer + vec2(12.0, -ROW_H / 2.0), vec2(g.size().x + 20.0, ROW_H));
        p.rect(r, 4.0, t.chrome, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
        p.galley(pos2(r.left() + 10.0, r.center().y - g.size().y / 2.0), g, t.text);
    }
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    let released = ctx.input(|i| i.pointer.any_released());
    if !list_rect.contains(pointer) {
        if released {
            egui::DragAndDrop::clear_payload(&ctx);
        }
        return;
    }
    // the gap nearest the pointer: before the first row whose middle is below it
    let at = rects.iter().position(|r| pointer.y < r.center().y).unwrap_or(rects.len());
    let y = match rects.get(at) {
        Some(r) => r.top(),
        None => rects.last().map_or(list_rect.top() + 4.0, |r| r.bottom()),
    };
    let y = y.clamp(list_rect.top() + 2.0, list_rect.bottom() - 2.0);
    ui.painter().with_clip_rect(list_rect).line_segment([pos2(list_rect.left() + 6.0, y), pos2(list_rect.right() - 6.0, y)], Stroke::new(2.0, t.accent));
    if released {
        match egui::DragAndDrop::take_payload::<Drag>(&ctx).as_deref() {
            Some(Drag::Row(from)) => move_row(menu, view, *from, at),
            Some(Drag::Add(k)) => {
                if let Some(slot) = Slot::parse(k) {
                    let at = at.min(menu.len());
                    menu.insert(at, slot);
                    view.picked = Some(at);
                }
            }
            None => {}
        }
    }
}

/// The list to add from, `width` wide, ending where the shown list (which starts at `left_top`) ends.
fn add_list(ui: &mut egui::Ui, t: &Tokens, key: &'static str, menu: &mut Vec<Slot>, view: &mut View, (width, left_top): (f32, f32)) {
    crate::widgets::text_field(ui, egui::Id::unique("context-menus-search"), &mut view.query, tl!("Search commands"), width, false);
    // The offers are built again only when the menu, the search or the shown rows changed.
    let shown = shown_hash(menu);
    let offers = match view.offers.as_ref().filter(|c| c.key == key && c.shown == shown && c.query == view.query) {
        Some(c) => c.offers.clone(),
        None => {
            let offers = Arc::new(grouped(available(key, menu, &view.query)));
            view.offers = Some(OffersCache { key, query: view.query.clone(), shown, offers: offers.clone() });
            offers
        }
    };
    let mut add: Option<Slot> = None;
    let right_top = ui.cursor().top();
    frame(t).show(ui, |ui| {
        ui.set_width(width - 10.0);
        // (before the scroll area: show_rows spaces its rows by the spacing it is given)
        ui.spacing_mut().item_spacing.y = 0.0;
        let h = (view.list_h - (right_top - left_top)).max(60.0);
        egui::ScrollArea::vertical().id_salt("context-menu-add").max_height(h).min_scrolled_height(h).auto_shrink([false, false]).show_rows(
            ui,
            ROW_H,
            offers.len(),
            |ui, range| {
                if offers.is_empty() {
                    ui.label(egui::RichText::new(tl!("Nothing matches")).color(t.text_dim));
                }
                for offer in offers.get(range).unwrap_or_default() {
                    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
                    let a = match offer {
                        Offer::Header(group) => {
                            let g = truncated(ui, tl!(group).to_string(), egui::FontId::proportional(10.5), t.text_faint, rect.width() - 12.0);
                            ui.painter().galley(pos2(rect.left() + 6.0, rect.bottom() - g.size().y - 3.0), g, t.text_faint);
                            continue;
                        }
                        Offer::Item(a) => a,
                    };
                    let slot_key = Slot::Cmd(a.id.to_string()).key();
                    if r.hovered() && !egui::DragAndDrop::has_any_payload(ui.ctx()) {
                        ui.painter().rect_filled(rect, 4.0, t.hover);
                    }
                    let plus = Rect::from_center_size(pos2(rect.right() - 12.0, rect.center().y), vec2(18.0, 18.0));
                    let g = truncated(ui, tl!(a.label).to_string(), egui::FontId::proportional(12.5), t.text, (plus.left() - rect.left() - 18.0).max(10.0));
                    ui.painter().galley(pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, t.text);
                    let clicked = icon_button(ui, t, plus, ui.scope_id().with(("add", a.id)), "plus", tl!("Add to the menu"));
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, a.label));
                    // where the menu bar has it
                    let r = if a.path.is_empty() { r } else { r.on_hover_text(a.path.as_str()) };
                    if clicked || r.double_clicked() {
                        add = Some(Slot::Cmd(a.id.to_string()));
                    }
                    if r.drag_started() {
                        egui::DragAndDrop::set_payload(ui.ctx(), Drag::Add(slot_key));
                    }
                }
            },
        );
    });
    if let Some(slot) = add {
        insert(menu, view, slot);
    }
}

/// ↑ / ↓ pick a row, ⌥↑ / ⌥↓ move it, Delete / Backspace take it out: only while no text box has
/// the keyboard.
fn keys(ui: &egui::Ui, menu: &mut Vec<Slot>, view: &mut View) {
    if ui.ctx().memory(|m| m.focused().is_some()) || menu.is_empty() {
        return;
    }
    let Some(p) = view.picked.filter(|p| *p < menu.len()) else { return };
    ui.ctx().input_mut(|i| {
        if i.consume_key(Modifiers::ALT, Key::ArrowUp) {
            step(menu, view, p, true);
        } else if i.consume_key(Modifiers::ALT, Key::ArrowDown) {
            step(menu, view, p, false);
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            view.picked = Some(p.saturating_sub(1));
            view.reveal = true;
        } else if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            view.picked = Some((p + 1).min(menu.len() - 1));
            view.reveal = true;
        } else if i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) {
            remove(menu, view, p);
        }
    });
}
