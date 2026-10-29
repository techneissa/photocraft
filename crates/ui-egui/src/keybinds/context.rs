//! The When context read from the app: what each context key holds right now.

use photocraft_doc::LayerContent;

use super::when::{Context, keys};
use crate::PhotocraftApp;
use crate::shortcut_dispatch::Focus;

/// The value of `tool` for the tool in use: the part of its shortcut id after `tools.select.`.
fn tool_value(app: &PhotocraftApp) -> usize {
    let id = crate::tool_shortcuts::id(app.ui.tool);
    id.strip_prefix("tools.select.").and_then(|n| keys::value_index(keys::TOOL, n)).unwrap_or(0)
}

fn layer_kind(content: &LayerContent) -> &'static str {
    match content {
        LayerContent::Raster(_) => "raster",
        LayerContent::Group(_) => "group",
        LayerContent::Adjustment(_) => "adjustment",
        LayerContent::Fill(_) => "fill",
        LayerContent::Text(_) => "text",
        LayerContent::Shape(_) => "shape",
        LayerContent::Smart(_) => "smart",
    }
}

/// The context now. `focus` says where the keyboard is (a text box, a widget, nowhere).
pub fn read(app: &PhotocraftApp, focus: Focus) -> Context {
    let mut c = Context::default();
    c.set(keys::TOOL, tool_value(app));
    c.set_flag(keys::TEXT_FOCUS, focus == Focus::Text);
    c.set_flag(keys::WIDGET_FOCUS, matches!(focus, Focus::Widget | Focus::DeletingWidget));
    c.set_flag(keys::DIALOG_OPEN, !app.ui.dialogs.is_empty() || app.discard.is_some());
    c.set_flag(keys::PALETTE_OPEN, app.ui.palette_open);
    c.set_flag(keys::EDITING_TEXT, app.ui.text_edit.is_some());
    c.set_flag(keys::TRANSFORM_ACTIVE, app.ui.transform.is_some() || app.distort.active());
    c.set_flag(keys::CROP_PENDING, crate::crop_ui::pending(app));
    c.set_flag(keys::FULLSCREEN, app.ui.view.screen_mode != "standard");
    if let Some(st) = app.session.active() {
        c.set_flag(keys::HAS_DOCUMENT, true);
        c.set_flag(keys::HAS_SELECTION, st.doc.selection.is_some());
        c.set_flag(keys::HAS_PATH, st.path.is_some());
        c.set_flag(keys::MULTI_LAYER, st.selected_layers().len() > 1);
        let kind = st.active_layer.and_then(|l| st.doc.layer(l)).map_or("none", |l| layer_kind(&l.content));
        c.set(keys::LAYER_KIND, keys::value_index(keys::LAYER_KIND, kind).unwrap_or(0));
    }
    c
}
