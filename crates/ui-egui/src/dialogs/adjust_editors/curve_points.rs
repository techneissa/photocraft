//! The Curves dialog's black point and white point sliders under the graph, and its Show Clipping
//! checkbox. The sliders move the curve's first and last point along the input axis, as
//! Photoshop's do; Alt-dragging one (or dragging one with Show Clipping on) shows what the curve
//! clips on the canvas (`levels_clipping.rs`, `curves_clipping.rs`).
use super::{Color32, Edit, EditorCx, Rect, Sense, Stroke, Tokens, pos2, widgets};

/// Which end slider is held in the Curves editor rooted at `mem` (0 black point, 1 white point).
fn drag_id(mem: egui::Id) -> egui::Id {
    mem.with("curves-point-drag")
}

fn show_clipping_id(mem: egui::Id) -> egui::Id {
    mem.with("curves-show-clipping")
}

/// The end slider being dragged in the editor rooted at `mem`, as of its last frame.
pub(crate) fn curves_drag(ctx: &egui::Context, mem: egui::Id) -> Option<u8> {
    ctx.data(|d| d.get_temp::<Option<u8>>(drag_id(mem))).flatten()
}

/// Whether the Show Clipping checkbox of the editor rooted at `mem` is on.
pub(crate) fn show_clipping(ctx: &egui::Context, mem: egui::Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(show_clipping_id(mem))).unwrap_or(false)
}

/// Mirror the Show Clipping checkbox into the dialog's `__showClipping` field and back, so the
/// control channel can set it: a value that differs from the last one mirrored wins.
pub(crate) fn sync_show_clipping(ctx: &egui::Context, mem: egui::Id, fields: &mut serde_json::Map<String, serde_json::Value>) {
    let last_id = mem.with("curves-show-clipping-last");
    let last: Option<bool> = ctx.data(|d| d.get_temp(last_id));
    let field = fields.get("__showClipping").and_then(serde_json::Value::as_bool);
    let on = match field {
        Some(f) if Some(f) != last => {
            ctx.data_mut(|d| d.insert_temp(show_clipping_id(mem), f));
            f
        }
        _ => show_clipping(ctx, mem),
    };
    if field != Some(on) {
        fields.insert("__showClipping".into(), serde_json::Value::Bool(on));
    }
    ctx.data_mut(|d| d.insert_temp(last_id, on));
}

/// Where the slider strip sits under a graph (the gradient bar is above it).
pub(crate) fn strip(graph: Rect) -> Rect {
    Rect::from_min_max(pos2(graph.left() - 6.0, graph.bottom() + 12.0), pos2(graph.right() + 6.0, graph.bottom() + 24.0))
}

/// A triangular handle pointing up at the gradient bar.
fn handle(p: &egui::Painter, x: f32, y: f32, fill: Color32, stroke: Color32) {
    p.add(egui::Shape::convex_polygon(vec![pos2(x, y), pos2(x + 5.5, y + 9.0), pos2(x - 5.5, y + 9.0)], fill, Stroke::new(1.0, stroke)));
}

/// The black point and white point sliders under the graph: they sit at the first and last point
/// of `pts` and move its input. Returns what the drag changed.
pub(super) fn point_sliders(ui: &mut egui::Ui, cx: &EditorCx, graph: Rect, pts: &mut [[f32; 2]]) -> (bool, Edit) {
    let t = Tokens::get(ui.ctx());
    let mut e = Edit::default();
    let mut changed = false;
    let Some((first, last)) = pts.first().copied().zip(pts.last().copied()) else { return (false, e) };
    if !graph.is_finite() || graph.width() <= 0.0 {
        return (false, e);
    }
    let bar = strip(graph);
    let resp = ui.interact(bar, cx.mem.with("curves-point-strip"), Sense::click_and_drag());
    let xpos = |val: f32| graph.left() + val / 255.0 * graph.width();
    let xval = |x: f32| ((x - graph.left()) / graph.width() * 255.0).clamp(0.0, 255.0);
    let id = drag_id(cx.mem);
    let mut which: Option<u8> = ui.data(|d| d.get_temp(id)).flatten();
    if resp.is_pointer_button_down_on()
        && ui.input(|i| i.pointer.primary_pressed())
        && let Some(pos) = ui.input(|i| i.pointer.press_origin())
    {
        which = super::levels::nearest(&[xpos(first[0]), xpos(last[0])], pos.x);
    }
    if resp.is_pointer_button_down_on()
        && let (Some(k), Some(pos)) = (which, ui.input(|i| i.pointer.interact_pos()))
    {
        let (i, old) = if k == 0 { (0, first) } else { (pts.len() - 1, last) };
        changed = crate::point_curve::move_to(pts, i, [xval(pos.x), old[1]]);
    }
    if which.is_some() && !resp.is_pointer_button_down_on() {
        which = None;
        e.commit = true;
    }
    ui.data_mut(|d| d.insert_temp(id, which));
    let p = ui.painter_at(bar.expand(2.0));
    let (first, last) = (pts.first().copied().unwrap_or(first), pts.last().copied().unwrap_or(last));
    handle(&p, xpos(first[0]), bar.top() - 1.0, Color32::BLACK, t.text_dim);
    handle(&p, xpos(last[0]), bar.top() - 1.0, Color32::WHITE, t.text_dim);
    (changed, e)
}

/// The Show Clipping checkbox under the graph: with it on, dragging a point slider shows the
/// clipping display without Alt.
pub(super) fn show_clipping_row(ui: &mut egui::Ui, cx: &EditorCx) {
    let id = show_clipping_id(cx.mem);
    let mut on = show_clipping(ui.ctx(), cx.mem);
    ui.add_space(2.0);
    if widgets::checkbox(ui, &mut on, tl!("Show Clipping")).changed() {
        ui.data_mut(|d| d.insert_temp(id, on));
    }
}
