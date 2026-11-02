//! The list of commands Keyboard Shortcuts shows, kept between frames. [`super::shortcuts::shortcut_items`]
//! builds the whole menu catalogue and looks every item up in three tables; the window used to do
//! that on every frame it was open. [`cached`] builds it once and builds it again only when something
//! it reads changes.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use super::PhotocraftApp;
use super::shortcuts_record::Item;

thread_local! {
    /// The fingerprint of what the list was built from, and the list.
    static LIST: RefCell<Option<(u64, Rc<Vec<Item>>)>> = const { RefCell::new(None) };
}

/// [`super::shortcuts::shortcut_items`], built again only when what it reads changed: the keys the
/// user set, the hidden menu items, the language, the plug-ins, and the few menu labels that follow
/// the selected layer.
pub(super) fn cached(app: &PhotocraftApp) -> Rc<Vec<Item>> {
    let print = fingerprint(app);
    LIST.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some((kept, items)) = slot.as_ref()
            && *kept == print
        {
            return Rc::clone(items);
        }
        let items = Rc::new(super::shortcuts::shortcut_items(app));
        *slot = Some((print, Rc::clone(&items)));
        items
    })
}

/// What [`super::shortcuts::shortcut_items`] reads, in one number: `menu_items` and the tables behind it.
/// Menu items that are checked or enabled differently don't change the list, so neither is read here.
fn fingerprint(app: &PhotocraftApp) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let prefs = app.session.prefs();
    // The keys the user set (a default shows only without one), the items Edit › Menus hid, and the
    // quick export format that names its menu item.
    prefs.shortcuts.hash(&mut h);
    prefs.menus.hidden.hash(&mut h);
    std::mem::discriminant(&prefs.export.quick_export_format).hash(&mut h);
    crate::i18n::current().code().hash(&mut h);
    photocraft_plugins::registry::revision().hash(&mut h);
    // The menu words that follow the selected layers: Disable / Enable Layer Mask, Merge Group, and
    // Create / Release Clipping Mask.
    let layer = app.session.active().and_then(|s| s.active_layer.and_then(|id| s.doc.layer(id)));
    layer.and_then(|l| l.mask.as_ref()).map(|m| m.enabled).hash(&mut h);
    let group_only = app
        .session
        .active()
        .is_some_and(|s| s.selected_layers().len() <= 1 && layer.is_some_and(|l| matches!(l.content, photocraft_doc::LayerContent::Group(_))));
    group_only.hash(&mut h);
    crate::shortcut_dispatch::clip_key_releases(app).hash(&mut h);
    h.finish()
}
