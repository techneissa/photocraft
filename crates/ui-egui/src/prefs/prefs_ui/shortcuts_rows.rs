//! The rows of Keyboard Shortcuts: a quiet selection, the reset icon of a changed row, the
//! warning icon, and removing a row's keys (right-click, or ⌫ / Delete on the selected row).

use std::collections::BTreeMap;

use egui::{Rect, Sense, pos2, vec2};

use super::shortcuts::dialog_keys;
use super::shortcuts_record::effective;
use super::{Tokens, prefs};

/// A row's staged bindings: main keys, further keys, chords, When conditions.
pub(super) struct Bindings<'a> {
    pub overrides: &'a mut BTreeMap<String, String>,
    pub alternates: &'a mut BTreeMap<String, Vec<String>>,
    pub chords: &'a mut BTreeMap<String, String>,
    pub whens: &'a mut BTreeMap<String, String>,
}

/// The four staged maps, read-only.
type Maps<'a> = (&'a BTreeMap<String, String>, &'a BTreeMap<String, Vec<String>>, &'a BTreeMap<String, String>, &'a BTreeMap<String, String>);

/// Does the row differ from the default: other keys, a chord or a condition?
pub(super) fn changed((overrides, alternates, chords, whens): Maps, id: &str, default: Option<&str>) -> bool {
    let none = (BTreeMap::new(), BTreeMap::new());
    dialog_keys(overrides, alternates, id, default) != dialog_keys(&none.0, &none.1, id, default) || chords.contains_key(id) || whens.contains_key(id)
}

/// The reset icon of a changed row; its tooltip says what it goes back to. True when clicked.
pub(super) fn reset_button(ui: &mut egui::Ui, default: Option<&str>) -> bool {
    let tip = match default.and_then(prefs::normalize_shortcut) {
        Some(key) => crate::i18n::fmt(tl!("Reset to {shortcut}"), &[("shortcut", &crate::shortcuts::pretty(&key))]),
        None => tl!("Reset to none").to_string(),
    };
    crate::icons::button(ui, "undo-2", 20.0, false, &tip).clicked()
}

/// Put the row back to its default keys, chord and condition.
pub(super) fn reset(b: &mut Bindings, id: &str) {
    b.overrides.remove(id);
    b.alternates.remove(id);
    b.chords.remove(id);
    b.whens.remove(id);
}

/// Take every key and the chord off the row. Its condition stays.
pub(super) fn remove(b: &mut Bindings, id: &str, default: Option<&str>) {
    if effective(b.overrides, id, &default.map(str::to_string)).is_some() {
        b.overrides.insert(id.to_string(), String::new());
    }
    if !dialog_keys(b.overrides, b.alternates, id, default).is_empty() {
        b.alternates.insert(id.to_string(), Vec::new());
    }
    b.chords.remove(id);
}

/// The small warning icon of a row, its sentence on hover.
pub(super) fn warning_icon(ui: &mut egui::Ui, t: &Tokens, sentence: &str) {
    let (rect, r) = ui.allocate_exact_size(vec2(18.0, 20.0), Sense::hover());
    crate::icons::paint(ui, rect, "triangle-alert", 14.0, t.warning);
    r.on_hover_text(sentence);
}

/// Reserve the selection's place under the row's cells (call before the first cell).
pub(super) fn selection_slot(ui: &egui::Ui) -> egui::layers::ShapeIdx {
    ui.painter().add(egui::Shape::Noop)
}

/// Fill the selected row with a quiet tint from its first cell to the right edge (call after the
/// last cell, before `end_row`).
pub(super) fn paint_selection(ui: &egui::Ui, t: &Tokens, slot: egui::layers::ShapeIdx, first_cell: Rect, selected: bool) {
    if !selected {
        return;
    }
    let rect = Rect::from_min_max(pos2(first_cell.left() - 4.0, first_cell.top() - 1.5), pos2(ui.clip_rect().right(), ui.min_rect().bottom() + 1.5));
    ui.painter().set(slot, egui::Shape::rect_filled(rect, 3.0, t.accent.gamma_multiply(0.16)));
}

#[cfg(test)]
mod tests {
    use super::*;

    type Owned = (BTreeMap<String, String>, BTreeMap<String, Vec<String>>, BTreeMap<String, String>, BTreeMap<String, String>);

    fn staged() -> Owned {
        Default::default()
    }

    #[test]
    fn a_row_is_changed_when_its_keys_chord_or_condition_differ_from_the_default() {
        let (mut o, a, mut c, mut w) = staged();
        let id = "file.new";
        assert!(!changed((&o, &a, &c, &w), id, Some("Cmd+N")));
        o.insert(id.into(), "Cmd+Alt+N".into());
        assert!(changed((&o, &a, &c, &w), id, Some("Cmd+N")));
        o.insert(id.into(), "Cmd+N".into());
        assert!(!changed((&o, &a, &c, &w), id, Some("Cmd+N")), "set back to the default");
        o.insert(id.into(), String::new());
        assert!(changed((&o, &a, &c, &w), id, Some("Cmd+N")), "its key taken off");
        o.clear();
        c.insert(id.into(), "Cmd+K C".into());
        assert!(changed((&o, &a, &c, &w), id, Some("Cmd+N")));
        c.clear();
        w.insert(id.into(), "hasDocument".into());
        assert!(changed((&o, &a, &c, &w), id, Some("Cmd+N")));
    }

    #[test]
    fn a_command_without_a_default_is_unchanged_when_its_key_is_taken_off_again() {
        let (mut o, a, c, w) = staged();
        o.insert("x.y".into(), String::new());
        assert!(!changed((&o, &a, &c, &w), "x.y", None));
    }

    #[test]
    fn removing_clears_the_keys_and_the_chord_and_reset_brings_everything_back() {
        let (mut o, mut a, mut c, mut w) = staged();
        let id = "file.new";
        a.insert(id.into(), vec!["F5".into()]);
        c.insert(id.into(), "Cmd+K C".into());
        w.insert(id.into(), "hasDocument".into());
        remove(&mut Bindings { overrides: &mut o, alternates: &mut a, chords: &mut c, whens: &mut w }, id, Some("Cmd+N"));
        assert_eq!(o.get(id).map(String::as_str), Some(""));
        assert_eq!(a.get(id), Some(&Vec::new()));
        assert!(c.is_empty());
        assert_eq!(w.len(), 1, "the condition stays");
        reset(&mut Bindings { overrides: &mut o, alternates: &mut a, chords: &mut c, whens: &mut w }, id);
        assert!(o.is_empty() && a.is_empty() && c.is_empty() && w.is_empty());
    }

    #[test]
    fn removing_a_command_without_keys_changes_nothing() {
        let (mut o, mut a, mut c, mut w) = staged();
        remove(&mut Bindings { overrides: &mut o, alternates: &mut a, chords: &mut c, whens: &mut w }, "x.y", None);
        assert!(o.is_empty() && a.is_empty() && c.is_empty());
    }
}
