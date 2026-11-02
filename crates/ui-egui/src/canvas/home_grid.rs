//! Fork (`fork/home-recent-grid.md`): the Home screen with the title, buttons and drop hint at
//! the top and the recent files as a grid of image previews filling the rest of the canvas.
//! Clicking a preview opens the file; the hovered preview shows a × that takes it off Recent, and
//! right-click offers Remove from Recent and Clear Recent Files, as the official list did.

#[cfg(test)]
mod snapshots;
#[cfg(test)]
mod tests;
mod thumbs;

use super::start_screen::start_screen_drop_hint;
use super::tabs::{open_on_double_click, paint_dots};
use super::{PhotocraftApp, Rect, Sense};
use thumbs::Thumb;

/// Widest the title, text and buttons get.
const HEADER_WIDTH: f32 = 460.0;

/// Least width of each of the New document and Open buttons.
const BUTTON_WIDTH: f32 = 190.0;

/// Space between the two buttons.
const BUTTON_GAP: f32 = 12.0;

/// Height of the title, text, buttons and drop hint with the buttons side by side.
const HEADER_HEIGHT: f32 = 168.0;

/// Stacked buttons take one more row.
const STACKED_EXTRA: f32 = 44.0;

/// Space above the header when the grid is shown.
const TOP_SPACE: f32 = 48.0;

/// Space between the header and the grid's "Recent" heading.
const HEADER_GAP: f32 = 34.0;

/// Narrowest a preview gets before the grid drops a column.
const TILE_MIN_WIDTH: f32 = 190.0;

/// Most previews in a row.
const MAX_COLUMNS: usize = 6;

/// Space between previews, across and down.
const TILE_GAP: f32 = 18.0;

/// Space between the grid and the canvas's left and right edges.
const SIDE_SPACE: f32 = 40.0;

/// Widest the grid gets on a very wide window.
const MAX_GRID_WIDTH: f32 = 1600.0;

/// A preview's picture is this tall for its width (4:3).
const IMAGE_ASPECT: f32 = 0.75;

/// Height of the name and folder lines under a preview.
const CAPTION_HEIGHT: f32 = 44.0;

/// Columns, preview width and left edge of the grid in a canvas `width` points wide.
pub(super) fn grid_columns(width: f32) -> (usize, f32, f32) {
    let inner = (width - 2.0 * SIDE_SPACE).clamp(1.0, MAX_GRID_WIDTH);
    let fit = ((inner + TILE_GAP) / (TILE_MIN_WIDTH + TILE_GAP)).floor();
    let columns = if fit.is_finite() { (fit as usize).clamp(1, MAX_COLUMNS) } else { 1 };
    let tile = ((inner - TILE_GAP * (columns as f32 - 1.0)) / columns as f32).max(1.0);
    (columns, tile, (width - inner) / 2.0)
}

/// Draws the Home screen; always `true`, so the official one in `start_screen.rs` is skipped
/// (kept as is, so updates from upstream merge cleanly).
pub(super) fn home(app: &mut PhotocraftApp, ui: &mut egui::Ui) -> bool {
    let area = ui.available_rect_before_wrap();
    paint_dots(ui, area, app.session.prefs().interface.show_canvas_dots);
    open_on_double_click(app, ui, area);
    // File › Open Recent, newest first (on the web there are no paths to reopen).
    let show_recent = app.session.prefs().interface.show_home_recent_files;
    let recent: Vec<String> = if cfg!(target_arch = "wasm32") || !show_recent {
        Vec::new()
    } else {
        let limit = app.session.prefs().interface.home_recent_count.clamp(1, 100) as usize;
        app.ui.recent_files.iter().take(limit).cloned().collect()
    };
    let new_label = crate::shortcuts::command_label(app, "New document…", "file.new");
    let open_label = crate::shortcuts::command_label(app, "Open…", "file.open");
    let header_width = HEADER_WIDTH.min(area.width());
    let button_width = BUTTON_WIDTH.max(crate::widgets::button_width(ui, &[&new_label, &open_label]));
    let side_by_side = 2.0 * button_width + BUTTON_GAP <= header_width;
    let header_height = HEADER_HEIGHT + if side_by_side { 0.0 } else { STACKED_EXTRA };
    let size = egui::vec2(header_width, header_height);
    // With nothing to show below it, the header stays in the middle of the canvas.
    let header = if recent.is_empty() {
        Rect::from_center_size(area.center(), size)
    } else {
        let top = area.top() + TOP_SPACE.min(area.height() * 0.06).max(16.0);
        Rect::from_min_size(egui::pos2(area.center().x - header_width / 2.0, top), size)
    };
    ui.scope_builder(egui::UiBuilder::new().max_rect(header), |ui| {
        header_ui(app, ui, &new_label, &open_label, button_width, side_by_side);
    });
    if recent.is_empty() {
        return true;
    }
    let grid_area = Rect::from_min_max(egui::pos2(area.left(), header.bottom() + HEADER_GAP), area.right_bottom());
    if grid_area.height() < 40.0 {
        return true;
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(grid_area), |ui| {
        egui::ScrollArea::vertical().id_salt("home-recent-grid").auto_shrink([false, false]).show(ui, |ui| recent_grid(app, ui, &recent));
    });
    true
}

/// Title, one line of text, New document and Open, and the drop hint, centred in `ui`.
fn header_ui(app: &mut PhotocraftApp, ui: &mut egui::Ui, new_label: &str, open_label: &str, button_width: f32, side_by_side: bool) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let width = ui.max_rect().width();
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let title = ui.painter().layout_no_wrap(tl!("PhotoCraft").into(), crate::theme::semibold(38.0), t.text);
            let by = ui.painter().layout_no_wrap(tl!("open source").into(), egui::FontId::proportional(13.0), t.text_faint);
            let total = title.size().x + by.size().x + 10.0;
            ui.add_space(((width - total) / 2.0).max(0.0));
            let (r, _) = ui.allocate_exact_size(title.size(), Sense::hover());
            ui.painter().galley(r.min, title, t.text);
            let (r2, _) = ui.allocate_exact_size(egui::vec2(by.size().x + 10.0, r.height()), Sense::hover());
            ui.painter().galley(egui::pos2(r2.left() + 10.0, r.bottom() - by.size().y - 8.0), by, t.text_faint);
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tl!("Create a new document or open an existing file.")).color(t.text_dim).size(14.0));
        ui.add_space(20.0);
        let (mut new_clicked, mut open_clicked) = (false, false);
        if side_by_side {
            ui.horizontal(|ui| {
                ui.add_space(((width - 2.0 * button_width - BUTTON_GAP) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing.x = BUTTON_GAP;
                new_clicked = crate::widgets::primary_button(ui, new_label, button_width).clicked();
                open_clicked = crate::widgets::secondary_button(ui, open_label, button_width).clicked();
            });
        } else {
            new_clicked = crate::widgets::primary_button(ui, new_label, button_width).clicked();
            ui.add_space(BUTTON_GAP);
            open_clicked = crate::widgets::secondary_button(ui, open_label, button_width).clicked();
        }
        if new_clicked {
            let fields = app.new_document_fields();
            app.ui.open_dialog(crate::state::DialogKind::NewDocument, fields);
        }
        if open_clicked {
            let _ = app.open_dialog_file();
        }
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            let msg = start_screen_drop_hint(app);
            let g = ui.painter().layout_no_wrap(msg.to_string(), egui::FontId::proportional(12.5), t.text_faint);
            ui.add_space(((width - g.size().x - 24.0) / 2.0).max(0.0));
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
            crate::icons::paint(ui, r, "image", 15.0, t.text_faint);
            ui.label(egui::RichText::new(msg).color(t.text_faint));
        });
    });
}

/// What a click on the grid asked for, applied after the grid is drawn.
#[derive(Default)]
struct GridAction {
    open: Option<String>,
    remove: Option<String>,
    clear: bool,
}

/// The "Recent" heading and one preview per recent file, as many columns as fit (up to six).
fn recent_grid(app: &mut PhotocraftApp, ui: &mut egui::Ui, recent: &[String]) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let full = ui.max_rect();
    let (columns, tile_w, left) = grid_columns(full.width());
    let image_h = (tile_w * IMAGE_ASPECT).round();
    let tile_h = image_h + CAPTION_HEIGHT;
    let rows = recent.len().div_ceil(columns);
    let heading_h = 30.0;
    let height = heading_h + rows as f32 * tile_h + rows.saturating_sub(1) as f32 * TILE_GAP + 24.0;
    let (space, _) = ui.allocate_exact_size(egui::vec2(full.width(), height), Sense::hover());
    let x0 = space.left() + left;
    let heading = ui.painter().layout_no_wrap(tl!("Recent").into(), crate::theme::semibold(13.0), t.text_dim);
    ui.painter().galley(egui::pos2(x0 + 2.0, space.top()), heading, t.text_dim);
    let thumbs = thumbs::cache(ui.ctx());
    let mut action = GridAction::default();
    {
        let mut cache = thumbs.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.receive(app, ui.ctx());
        cache.keep_only(recent);
        for (i, path) in recent.iter().enumerate() {
            let (row, col) = (i / columns, i % columns);
            let min = egui::pos2(x0 + col as f32 * (tile_w + TILE_GAP), space.top() + heading_h + row as f32 * (tile_h + TILE_GAP));
            let tile = Rect::from_min_size(min, egui::vec2(tile_w, tile_h));
            // Only previews on screen are decoded; the rest wait until they are scrolled to.
            let thumb = if ui.is_rect_visible(tile) { cache.thumb(ui.ctx(), path) } else { Thumb::Loading };
            recent_tile(ui, path, tile, image_h, thumb, &mut action);
        }
    }
    if action.clear {
        app.clear_recent();
    } else if let Some(path) = action.remove {
        app.remove_recent(&path);
    }
    if let Some(path) = action.open
        && let Err(e) = app.open_path(&path)
    {
        app.open_failed(&crate::file_open::display_name(&path), &e);
    }
}

/// One preview: the picture cropped to fill a 4:3 frame, the file name and its folder under it.
fn recent_tile(ui: &mut egui::Ui, path: &str, tile: Rect, image_h: f32, thumb: Thumb, action: &mut GridAction) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let name = crate::file_open::display_name(path);
    let resp = ui.interact(tile, ui.scope_id().with(("home-recent", path)), Sense::click());
    // The × sits on top of the tile, so the tile counts as hovered while the pointer is on it.
    let hovered = ui.rect_contains_pointer(tile);
    let image = Rect::from_min_size(tile.min, egui::vec2(tile.width(), image_h));
    let radius = t.radius_lg;
    ui.painter().rect_filled(image, radius, t.field);
    match thumb {
        Thumb::Ready(id, size) => {
            egui::Image::new(egui::load::SizedTexture::new(id, size)).uv(fill_uv(size, image.size())).corner_radius(radius).paint_at(ui, image);
        }
        Thumb::Loading => {
            crate::icons::paint(ui, Rect::from_center_size(image.center(), egui::vec2(28.0, 28.0)), "image", 24.0, t.text_faint);
        }
        Thumb::Unavailable => {
            crate::icons::paint(ui, Rect::from_center_size(image.center() - egui::vec2(0.0, 10.0), egui::vec2(28.0, 28.0)), "file", 24.0, t.text_faint);
            let ext = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
            if !ext.is_empty() {
                let g = ui.painter().layout_no_wrap(ext, egui::FontId::proportional(11.0), t.text_faint);
                ui.painter().galley(egui::pos2(image.center().x - g.size().x / 2.0, image.center().y + 12.0), g, t.text_faint);
            }
        }
    }
    let border = if hovered { egui::Stroke::new(2.0, t.accent) } else { egui::Stroke::new(1.0, t.card_border) };
    ui.painter().rect_stroke(image, radius, border, egui::StrokeKind::Inside);
    let name_g = crate::tab_strip::elided(ui, &name, egui::FontId::proportional(12.5), t.text, tile.width() - 4.0);
    ui.painter().galley(egui::pos2(tile.left() + 2.0, image.bottom() + 8.0), name_g, t.text);
    let folder = std::path::Path::new(path).parent().and_then(|d| d.file_name()).map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    if !folder.is_empty() {
        let g = crate::tab_strip::elided(ui, &folder, egui::FontId::proportional(11.5), t.text_faint, tile.width() - 4.0);
        ui.painter().galley(egui::pos2(tile.left() + 2.0, image.bottom() + 25.0), g, t.text_faint);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    let resp = resp.on_hover_text(path).on_hover_cursor(egui::CursorIcon::PointingHand);
    let mut removed = false;
    if hovered {
        let close = Rect::from_center_size(egui::pos2(image.right() - 18.0, image.top() + 18.0), egui::vec2(24.0, 24.0));
        let x = ui.interact(close, resp.id.with("remove"), Sense::click());
        let label = crate::i18n::fmt(tl!("Remove {name} from Recent"), &[("name", &name)]);
        x.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
        // A dark disc keeps the × readable on any picture.
        ui.painter().circle_filled(close.center(), 11.0, if x.hovered() { t.danger } else { t.scrim });
        crate::icons::paint(ui, close, "x", 12.0, t.primary_text);
        if x.on_hover_text(tl!("Remove from Recent")).clicked() {
            action.remove = Some(path.to_string());
            removed = true;
        }
    }
    if resp.clicked() && !removed {
        action.open = Some(path.to_string());
    }
    resp.context_menu(|ui| {
        if ui.button(tl!("Remove from Recent")).clicked() {
            action.remove = Some(path.to_string());
            ui.close();
        }
        if ui.button(tl!("Clear Recent Files")).clicked() {
            action.clear = true;
            ui.close();
        }
    });
}

/// The part of a `texture`-sized picture that fills a `frame` of another shape, centred.
pub(super) fn fill_uv(texture: egui::Vec2, frame: egui::Vec2) -> Rect {
    let full = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    if texture.x <= 0.0 || texture.y <= 0.0 || frame.x <= 0.0 || frame.y <= 0.0 {
        return full;
    }
    let (picture, box_) = (texture.x / texture.y, frame.x / frame.y);
    if !picture.is_finite() || !box_.is_finite() {
        full
    } else if picture > box_ {
        let w = box_ / picture;
        Rect::from_min_max(egui::pos2((1.0 - w) / 2.0, 0.0), egui::pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = picture / box_;
        Rect::from_min_max(egui::pos2(0.0, (1.0 - h) / 2.0), egui::pos2(1.0, (1.0 + h) / 2.0))
    }
}
