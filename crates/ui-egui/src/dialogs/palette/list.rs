//! The palette's list: the rows for a query, ↑/↓/PageUp/PageDown, the scrolling list and its rows.
//! `palette.rs` calls these in place of its old fixed 12-row list.

use egui::{Align2, Sense, vec2};

use super::nav::{self, Hit, MAX_ROWS, PAGE, ROW_H};
use super::{exact_bonus, fuzzy_score, menu_hits, recent_hits, scroll_id};
use crate::PhotocraftApp;
use crate::theme::{self, Tokens};

/// The list keys pressed this frame.
#[derive(Clone, Copy, Default)]
pub(super) struct Keys {
    down: bool,
    up: bool,
    page_down: bool,
    page_up: bool,
}

/// Take the list keys before the search field sees them (it would move its cursor on them). While
/// the Assign Shortcut menu is open they are not the palette's.
pub(super) fn take_keys(ctx: &egui::Context) -> Keys {
    if crate::assign_shortcut::is_open(ctx) {
        return Keys::default();
    }
    ctx.input_mut(|i| Keys {
        down: i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
        up: i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
        page_down: i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
        page_up: i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
    })
}

/// Enter and Esc, unless the Assign Shortcut menu is open: that menu takes them first.
pub(super) fn enter_esc(ctx: &egui::Context) -> (bool, bool) {
    let open = crate::assign_shortcut::is_open(ctx);
    ctx.input(|i| (i.key_pressed(egui::Key::Enter) && !open, i.key_pressed(egui::Key::Escape) && !open))
}

/// Every row for the query `q`, best first. An empty query lists the recently run commands
/// (UI-217-7); with none (or a fresh session) it falls back to the full fuzzy list.
pub(super) fn all_hits(app: &PhotocraftApp, q: &str, recents: &[String], lang: crate::i18n::Lang) -> Vec<Hit> {
    let menus = crate::menus::menu_items(app);
    let empty = q.trim().is_empty();
    let mut hits: Vec<Hit> = if empty { recent_hits(recents, &menus, lang) } else { Vec::new() };
    if !empty || hits.is_empty() {
        let from = hits.len();
        hits.extend(menu_hits(menus, q, lang));
        for h in &mut hits[from..] {
            h.0 += nav::word_start_bonus(q, &h.2);
        }
        for tool in crate::state::Tool::ALL {
            if let Some(s) = fuzzy_score(q, &format!("{} {}", tl!(tool.label()), tool.label())) {
                let s = s + exact_bonus(q, &[tl!(tool.label()).as_ref(), tool.label()]) + nav::word_start_bonus(q, tl!(tool.label()));
                // The tool's key as the user set it, not Photoshop's default.
                let key = crate::tool_shortcuts::shortcut(app, tool).map(|k| format!("   {}", crate::shortcuts::pretty(&k))).unwrap_or_default();
                hits.push((s + 2, format!("tool:{tool:?}"), tl!(tool.label()).into(), Some(format!("{}{key}", crate::i18n::tr(lang, "Tool"))), true));
            }
        }
    }
    nav::rank(&mut hits);
    hits
}

fn reveal_id() -> egui::Id {
    egui::Id::unique("palette-reveal")
}

/// Move the highlight `sel` for this frame's keys (↑/↓ wrap, pages stop at the ends); a new query
/// `q` starts at the top again.
pub(super) fn navigate(ctx: &egui::Context, keys: Keys, q: &str, len: usize, sel: &mut usize) {
    let query_id = egui::Id::unique("palette-last-query");
    let last: Option<String> = ctx.data(|d| d.get_temp(query_id));
    if last.as_deref().is_some_and(|l| l != q) {
        *sel = 0;
        ctx.data_mut(|d| d.insert_temp(scroll_id(), 0.0f32));
    }
    ctx.data_mut(|d| d.insert_temp(query_id, q.to_string()));
    let before = *sel;
    if keys.down {
        *sel = nav::step(*sel, 1, len);
    }
    if keys.up {
        *sel = nav::step(*sel, -1, len);
    }
    if keys.page_down {
        *sel = nav::page(*sel, PAGE as isize, len);
    }
    if keys.page_up {
        *sel = nav::page(*sel, -(PAGE as isize), len);
    }
    *sel = (*sel).min(len.saturating_sub(1));
    let moved = keys.down || keys.up || keys.page_down || keys.page_up;
    ctx.data_mut(|d| d.insert_temp(reveal_id(), moved || before != *sel));
}

/// Has the pointer moved since the last frame the palette drew? The first frame of a session (the
/// palette just opened under a resting pointer) has no earlier position: not moved.
fn pointer_moved(ctx: &egui::Context) -> bool {
    let id = egui::Id::unique("palette-pointer");
    let now = ctx.cumulative_frame_nr();
    let pos = ctx.input(|i| i.pointer.hover_pos());
    let last: Option<(Option<egui::Pos2>, u64)> = ctx.data(|d| d.get_temp(id));
    ctx.data_mut(|d| d.insert_temp(id, (pos, now)));
    matches!(last, Some((prev, at)) if now.saturating_sub(at) <= 1 && pos.is_some() && pos != prev)
}

/// The scrolling list of `hits` (twelve rows on show, never past the window's bottom), the
/// highlight `sel` following the pointer, a click or right-click on a row acting on it.
pub(super) fn rows(ui: &mut egui::Ui, t: &Tokens, hits: &[Hit], sel: &mut usize, width: f32, run: &mut Option<String>) {
    let len = hits.len();
    if len == 0 {
        return;
    }
    let ctx = ui.ctx().clone();
    // The list gets its own height: the palette's Area keeps the size it had.
    let room = (ctx.content_rect().bottom() - ui.cursor().top() - 80.0).max(ROW_H * 3.0);
    let height = (ROW_H * len.min(MAX_ROWS) as f32).min(room);
    let mut scroll: f32 = ctx.data(|d| d.get_temp(scroll_id()).unwrap_or(0.0));
    let mut list = egui::ScrollArea::vertical().id_salt("palette-list").max_height(height).auto_shrink([false, false]);
    if ctx.data_mut(|d| d.remove_temp::<bool>(reveal_id())).unwrap_or(false) {
        scroll = nav::reveal(scroll, *sel, height);
        list = list.vertical_scroll_offset(scroll);
    }
    // The pointer moving onto a row highlights it; a resting pointer doesn't take the highlight
    // back from the keys when the list scrolls under it.
    let moved = pointer_moved(&ctx);
    let shown = *sel;
    let out = ui.allocate_ui(vec2(width, height), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        list.show_rows(ui, ROW_H, len, |ui, range| {
            for i in range {
                let Some((_, id, label, detail, enabled)) = hits.get(i) else { continue };
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, *enabled, label.as_str()));
                if i == shown {
                    ui.painter().rect_filled(rect, t.radius_sm, t.accent_soft);
                }
                if resp.hovered() && moved {
                    *sel = i;
                    ctx.request_repaint();
                }
                let color = if *enabled { t.text } else { t.text_faint };
                ui.painter().text(rect.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, label, theme::medium(13.0), color);
                if let Some(d) = detail {
                    ui.painter().text(rect.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, d, egui::FontId::proportional(11.5), t.text_faint);
                }
                if resp.clicked() && *enabled {
                    *run = Some(id.clone());
                }
                let command = id.strip_prefix("tool:").and_then(crate::state::Tool::from_name).map_or_else(|| id.clone(), crate::tool_shortcuts::id);
                crate::assign_shortcut::on_palette(ui, &resp, &command);
            }
        })
    });
    scroll = out.inner.state.clamped_offset().y;
    ctx.data_mut(|d| d.insert_temp(scroll_id(), scroll));
}

#[cfg(test)]
mod assign_tests;
#[cfg(test)]
mod upgrade_tests;
