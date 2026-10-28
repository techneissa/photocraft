//! Type-to-search in the dropdown lists the wrappers in `dropdown.rs` build. With a list open, typing
//! narrows it to the options that match (a slim field with a magnifier shows the text above the
//! rows), ↑/↓ move a highlight over the matches, ↩ or a click picks one, and Esc clears the text
//! before it closes the list. Nothing shows until a letter is typed, so a list looks as it always did.
//!
//! The matching and the key handling are plain code on strings ([`matches`], [`Search`]); [`before`]
//! feeds them egui's events, and the [`Filter`] it returns draws the field and the rows. It replaces
//! the older jump to the next option starting with the typed letter (#1481).

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use egui::{Event, Id, Key, Ui};

use super::TEXT_FIELD_H;
use super::text_field;
use super::type_ahead;
use crate::theme::Tokens;

/// The options `query` matches, best first: those whose label starts with it, then those with a word
/// starting with it ("li" finds Lighten and Linear Burn before Soft Light), then those that merely
/// contain it ("Multiply"); each group in list order. Case is ignored. `enabled[i] == false` keeps
/// option `i` out (an option without an entry is enabled). A blank query matches every enabled option.
pub(super) fn matches(query: &str, labels: &[impl AsRef<str>], enabled: &[bool]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut tiers: [Vec<usize>; 3] = Default::default();
    for (i, label) in labels.iter().enumerate() {
        // An empty label is a separator or a gap, not something to pick.
        if label.as_ref().is_empty() || !enabled.get(i).copied().unwrap_or(true) {
            continue;
        }
        let tier = tier(&label.as_ref().to_lowercase(), &query);
        if let Some(list) = tier.and_then(|t| tiers.get_mut(t)) {
            list.push(i);
        }
    }
    tiers.concat()
}

/// 0 for a label (lower case) starting with `query`, 1 for one with a word starting with it, 2 for
/// one containing it, none for no match. `match_indices` gives character boundaries, so the slice
/// before a match never splits a character.
fn tier(label: &str, query: &str) -> Option<usize> {
    if label.starts_with(query) {
        return Some(0);
    }
    let mut found = None;
    for (at, _) in label.match_indices(query) {
        let word_start = label.get(..at).and_then(|before| before.chars().next_back()).is_none_or(|c| !c.is_alphanumeric());
        if word_start {
            return Some(1);
        }
        found = Some(2);
    }
    found
}

/// One thing the user did in an open list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Input<'a> {
    Text(&'a str),
    Backspace,
    /// ⌥ or ⌘ with Backspace: all the text.
    Clear,
    Up,
    Down,
    PageUp,
    PageDown,
    Enter,
    Esc,
}

/// How far PageUp and PageDown move the highlight.
const PAGE: usize = 5;

/// What the open list of a dropdown has been typed so far, and the match it highlights.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Search {
    pub query: String,
    /// The highlighted match, as a position in [`matches`].
    pub active: usize,
    /// An IME is composing text (its preedit is not empty): the field shows to take it.
    pub composing: bool,
}

/// What [`Search::apply`] did with an input.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Applied {
    /// The input belongs to the search: the list and the keyboard shortcuts don't get it.
    pub consumed: bool,
    /// ↩ picked this option (an index into the options).
    pub pick: Option<usize>,
}

impl Search {
    pub(super) fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Handles `input` for a list of `labels`. Keys that mean something with no text typed (↩ closes
    /// the list, Esc closes it, the arrows step the value) are left alone while there is none.
    pub(super) fn apply(&mut self, input: Input<'_>, labels: &[&str], enabled: &[bool]) -> Applied {
        let mut applied = Applied::default();
        let count = |query: &str| matches(query, labels, enabled).len();
        match input {
            Input::Text(text) => {
                let before = self.query.len();
                for c in text.chars().filter(|c| !c.is_control()) {
                    // A space only counts inside the text ("linear b"), never to start it.
                    if !(c.is_whitespace() && self.query.is_empty()) {
                        self.query.push(c);
                    }
                }
                if self.query.len() != before {
                    self.active = 0;
                }
                self.composing = false;
                applied.consumed = true;
            }
            Input::Backspace | Input::Clear if self.is_active() => {
                if input == Input::Clear {
                    self.query.clear();
                } else {
                    self.query.pop();
                }
                self.active = 0;
                applied.consumed = true;
            }
            Input::Esc if self.is_active() => {
                self.query.clear();
                self.active = 0;
                applied.consumed = true;
            }
            Input::Up | Input::Down | Input::PageUp | Input::PageDown if self.is_active() => {
                let last = count(&self.query).saturating_sub(1);
                self.active = match input {
                    Input::Up => self.active.saturating_sub(1),
                    Input::PageUp => self.active.saturating_sub(PAGE),
                    Input::Down => self.active.saturating_add(1),
                    _ => self.active.saturating_add(PAGE),
                }
                .min(last);
                applied.consumed = true;
            }
            Input::Enter if self.is_active() => {
                applied.pick = matches(&self.query, labels, enabled).get(self.active).copied();
                applied.consumed = true;
            }
            _ => {}
        }
        applied
    }
}

/// What a search has typed and found so far.
#[derive(Default)]
struct Found {
    query: String,
    /// The options shown, best match first.
    shown: Vec<usize>,
    /// The highlighted row, a position in `shown`.
    active: usize,
    /// The text or the highlight changed this frame: the highlighted row scrolls into view.
    moved: bool,
}

/// The search of one open list, made by [`before`] for the field and the rows.
#[derive(Default)]
pub struct Filter {
    /// The id of the dropdown's search; none for a list that is not open.
    id: Option<Id>,
    labels: Vec<String>,
    enabled: Vec<bool>,
    /// An IME is composing: the field shows even with no text yet.
    composing: bool,
    found: RefCell<Found>,
    /// The rows drawn so far this frame, to know which one is the highlighted.
    row: Cell<usize>,
}

impl Filter {
    /// Is a search typed?
    pub fn filtering(&self) -> bool {
        !self.found.borrow().query.is_empty()
    }

    fn salt(&self, name: &str) -> Option<Id> {
        self.id.map(|id| id.with(name))
    }

    /// The option ↩ picked, once: the list takes it when it draws, in this pass or a later one.
    pub fn picked(&self, ui: &Ui) -> Option<usize> {
        let pick = self.salt("pick")?;
        ui.data_mut(|d| d.remove_temp::<usize>(pick))
    }

    /// The top of the open list: the typed text in a slim field with a magnifier, nothing while none
    /// is typed. The field is a text field, so it takes the keyboard (IME, selection, paste) while it
    /// shows, and the held-key tools stay out of the way.
    pub fn header(&self, ui: &mut Ui) {
        let (Some(id), Some(field_id)) = (self.id, self.salt("field")) else { return };
        if !(self.filtering() || self.composing) {
            // Marks the list open for the shortcuts, and lets ↩ close a list with nothing typed.
            type_ahead::keys(ui, id, None, &[]);
            return;
        }
        let width = ui.data(|d| d.get_temp::<f32>(id.with("width"))).unwrap_or(FIELD_WIDTH).max(FIELD_MIN);
        let mut query = self.found.borrow().query.clone();
        let t = Tokens::get(ui.ctx());
        ui.horizontal(|ui| {
            let (icon, _) = ui.allocate_exact_size(egui::vec2(16.0, TEXT_FIELD_H), egui::Sense::hover());
            crate::icons::paint(ui, icon, "search", 14.0, t.text_faint);
            ui.memory_mut(|m| m.request_focus(field_id));
            let field = text_field(ui, field_id, &mut query, "", width - 20.0, false);
            field.accessible_name(tl!("Search"));
        });
        ui.add_space(2.0);
        // The marking leaves the typed text to the field above.
        type_ahead::keys(ui, id, None, &[]);
        if query != self.found.borrow().query {
            let shown = matches(&query, &self.labels, &self.enabled);
            ui.data_mut(|d| d.insert_temp(id, Search { query: query.clone(), active: 0, composing: false }));
            *self.found.borrow_mut() = Found { query, shown, active: 0, moved: true };
        }
    }

    /// The options to draw as rows: all of `items` (one per option) when nothing is typed, else the
    /// matches among them, best first.
    pub fn options<'a, X: Clone>(&self, items: &'a [X]) -> Cow<'a, [X]> {
        self.row.set(0);
        if !self.filtering() {
            return Cow::Borrowed(items);
        }
        Cow::Owned(self.found.borrow().shown.iter().filter_map(|&i| items.get(i).cloned()).collect())
    }

    /// The item of the highlighted row, among the `shown` rows [`Self::options`] gave.
    pub fn active_in<'a, X>(&self, shown: &'a [X]) -> Option<&'a X> {
        let found = self.found.borrow();
        if found.query.is_empty() { None } else { shown.get(found.active) }
    }

    /// Called as each row `item` is drawn, in order: the highlighted one is lit, and scrolled to when it moved.
    pub fn row(&self, ui: &Ui, item: &egui::Response) {
        let at = self.row.get();
        self.row.set(at.saturating_add(1));
        if let Some(id) = self.salt("width") {
            ui.data_mut(|d| d.insert_temp(id, item.rect.width()));
        }
        let found = self.found.borrow();
        if found.query.is_empty() || found.active != at {
            return;
        }
        let t = Tokens::get(ui.ctx());
        // Translucent: the row is already drawn, its text shows through.
        ui.painter().rect_filled(item.rect, t.radius_sm, t.accent.gamma_multiply(0.35));
        if found.moved {
            item.scroll_to_me(None);
        }
    }

    /// The dim line under the field when nothing matches.
    pub fn no_match(&self, ui: &mut Ui) {
        let found = self.found.borrow();
        if !found.query.is_empty() && found.shown.is_empty() {
            ui.label(egui::RichText::new(tl!("No match")).color(Tokens::get(ui.ctx()).text_faint));
        }
    }
}

/// The width of the field before a row has told how wide the list is, and the narrowest it gets.
const FIELD_WIDTH: f32 = 140.0;
const FIELD_MIN: f32 = 90.0;

/// A combo box with a search: [`Self::search`] is `show_ui` with the typed text in a field above the
/// rows.
pub trait Searchable {
    fn search<R>(self, ui: &mut Ui, filter: &Filter, rows: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<Option<R>>;
}

impl Searchable for craft_ui::ComboBox<'_> {
    fn search<R>(self, ui: &mut Ui, filter: &Filter, rows: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<Option<R>> {
        self.show_ui_with_header(ui, |ui| filter.header(ui), |ui, ()| rows(ui))
    }
}

/// Searches the open list of dropdown `salt`: takes the typing and the keys that belong to the search.
/// Call it before the combo box is shown, because a focused combo box button takes ↩ to toggle its
/// list before the list gets to see the key. `labels` are the options as displayed.
pub fn before(ui: &mut Ui, salt: &str, labels: &[impl AsRef<str>]) -> Filter {
    before_enabled(ui, salt, labels, &[])
}

/// [`before`], where `enabled[i] == false` keeps option `i` out of the matches (an option past the end
/// of `enabled` is enabled).
pub fn before_enabled(ui: &mut Ui, salt: &str, labels: &[impl AsRef<str>], enabled: &[bool]) -> Filter {
    let id = ui.make_persistent_id(salt).with("search");
    let button = ui.make_persistent_id(egui::IdSalt::new(salt));
    // A list that is not open starts with nothing typed when it opens.
    if !craft_ui::ComboBox::is_open(ui.ctx(), button) {
        ui.data_mut(|d| d.remove::<Search>(id));
        return Filter::default();
    }
    let labels: Vec<String> = labels.iter().map(|l| l.as_ref().to_owned()).collect();
    let texts: Vec<&str> = labels.iter().map(String::as_str).collect();
    let mut search: Search = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let before = search.clone();
    let mut picked = None;
    // With text typed the field has the keyboard: it edits the text, and only the keys that move the
    // highlight, pick and clear are the search's.
    let editing = search.is_active() || search.composing;
    ui.input_mut(|i| {
        i.events.retain(|event| {
            // An IME composing: the field shows from the next frame on, to show the preedit.
            if let Event::Ime(egui::ImeEvent::Preedit { text, .. }) = event {
                search.composing = !text.is_empty();
                return true;
            }
            let Some(input) = input_of(event) else { return true };
            if editing && matches!(input, Input::Text(_) | Input::Backspace | Input::Clear) {
                return true;
            }
            let applied = search.apply(input, &texts, enabled);
            picked = picked.or(applied.pick);
            !applied.consumed
        });
    });
    ui.data_mut(|d| d.insert_temp(id, search.clone()));
    if let Some(pick) = picked {
        ui.data_mut(|d| d.insert_temp(id.with("pick"), pick));
    }
    let mut found = Found::default();
    if search.is_active() {
        found = Found { shown: matches(&search.query, &texts, enabled), active: search.active, moved: search != before, query: search.query };
    }
    Filter { id: Some(id), enabled: enabled.to_vec(), labels, composing: search.composing, found: RefCell::new(found), row: Cell::new(0) }
}

/// The egui event as a search input, if it is one.
fn input_of(event: &Event) -> Option<Input<'_>> {
    match event {
        Event::Text(t) | Event::Paste(t) | Event::Ime(egui::ImeEvent::Commit(t)) => Some(Input::Text(t)),
        Event::Key { key, pressed: true, modifiers, .. } => {
            let plain = !(modifiers.command || modifiers.ctrl || modifiers.alt);
            match key {
                Key::Backspace if plain => Some(Input::Backspace),
                Key::Backspace => Some(Input::Clear),
                _ if !plain => None,
                Key::ArrowUp => Some(Input::Up),
                Key::ArrowDown => Some(Input::Down),
                Key::PageUp => Some(Input::PageUp),
                Key::PageDown => Some(Input::PageDown),
                Key::Enter => Some(Input::Enter),
                Key::Escape => Some(Input::Esc),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
