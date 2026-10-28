//! Type-to-search in the dropdown lists the wrappers in `dropdown.rs` build. With a list open, typing
//! narrows it to the options that match (a slim field with a magnifier shows the text above the
//! rows), ↑/↓ move a highlight over the matches, ↩ or a click picks one, and Esc clears the text
//! before it closes the list. Nothing shows until a letter is typed, so a list looks as it always did.
//!
//! The matching and the key handling are plain code on strings ([`matches`], [`Search`]); [`before`]
//! feeds them egui's events, and the [`Filter`] it returns draws the field and the rows. It replaces the older jump to the next option starting with the typed letter (#1481).

use egui::{Event, Id, Key, Ui};

use super::type_ahead;

/// The options `query` matches, best first: those whose label starts with it, then those with a word
/// starting with it ("li" finds Lighten and Linear Burn before Soft Light), then those that merely
/// contain it ("Multiply"); each group in list order. Case is ignored. `enabled[i] == false` keeps
/// option `i` out (an option without an entry is enabled). A blank query matches every enabled option.
pub(super) fn matches(query: &str, labels: &[&str], enabled: &[bool]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut tiers: [Vec<usize>; 3] = Default::default();
    for (i, label) in labels.iter().enumerate() {
        // An empty label is a separator or a gap, not something to pick.
        if label.is_empty() || !enabled.get(i).copied().unwrap_or(true) {
            continue;
        }
        let tier = tier(&label.to_lowercase(), &query);
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

/// The search of one open list, as [`before`] leaves it for the field and the rows.
#[derive(Default)]
pub struct Filter {
    /// The id of the dropdown's search, for [`type_ahead`] and the pick waiting to be taken.
    id: Option<Id>,
    query: String,
    /// The options shown, best match first; none when nothing is typed (every option shows).
    shown: Vec<usize>,
    active: Option<usize>,
    /// The highlight or the text changed this frame: the highlighted row scrolls into view.
    moved: bool,
}

impl Filter {
    /// Is a search typed?
    pub fn filtering(&self) -> bool {
        !self.query.is_empty()
    }

    /// The option ↩ picked, once: the list takes it when it draws, in this pass or a later one.
    pub fn picked(&self, ui: &Ui) -> Option<usize> {
        let pick = self.id?.with("pick");
        ui.data_mut(|d| d.remove_temp::<usize>(pick))
    }

    /// The option the highlight is on, while a search is typed.
    pub fn active_option(&self) -> Option<usize> {
        self.active
    }

    /// The top of the open list: the typed text in a slim field, nothing while none is typed.
    pub fn header(&self, ui: &mut Ui) {
        if let Some(id) = self.id {
            // Marks the list open for the shortcuts, and lets ↩ close a list with nothing typed.
            type_ahead::keys(ui, id, None, &[]);
        }
        if self.filtering() {
            field(ui, &self.query);
        }
    }

    /// `(index, item)` for the options to draw as rows, in the order to draw them: all of them in
    /// list order when nothing is typed, else the matches, best first.
    pub fn rows<I: Iterator>(&self, options: I) -> std::vec::IntoIter<(usize, I::Item)> {
        let all = options.enumerate();
        if !self.filtering() {
            return all.collect::<Vec<_>>().into_iter();
        }
        let mut slots: Vec<Option<(usize, I::Item)>> = all.map(Some).collect();
        self.shown.iter().filter_map(|&i| slots.get_mut(i)?.take()).collect::<Vec<_>>().into_iter()
    }

    /// Marks the row `item` of option `index`: the highlighted one is lit, and scrolled to when it moved.
    pub fn row(&self, ui: &Ui, index: usize, item: &egui::Response) {
        if self.active != Some(index) {
            return;
        }
        let v = ui.visuals();
        ui.painter().rect_filled(item.rect, v.widgets.hovered.corner_radius, v.selection.bg_fill.gamma_multiply(0.4));
        if self.moved {
            item.scroll_to_me(None);
        }
    }

    /// The dim line under the field when nothing matches.
    pub fn no_match(&self, ui: &mut Ui) {
        if self.filtering() && self.shown.is_empty() {
            ui.label(egui::RichText::new(tl!("No match")).weak());
        }
    }
}

/// Searches the open list of dropdown `salt`: takes the typing and the keys that belong to the search.
/// Call it before the combo box is shown, because a focused combo box button takes ↩ to toggle its
/// list before the list gets to see the key. `labels` are the options as displayed; `enabled`
/// says which can be picked (an option past its end can).
pub fn before(ui: &mut Ui, salt: &str, labels: &[&str], enabled: &[bool]) -> Filter {
    let id = ui.make_persistent_id(salt).with("search");
    let button = ui.make_persistent_id(egui::IdSalt::new(salt));
    // A list that is not open starts with nothing typed when it opens.
    if !craft_ui::ComboBox::is_open(ui.ctx(), button) {
        ui.data_mut(|d| d.remove::<Search>(id));
        return Filter::default();
    }
    let mut search: Search = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let before = search.clone();
    let mut picked = None;
    ui.input_mut(|i| {
        i.events.retain(|event| {
            let Some(input) = input_of(event) else { return true };
            let applied = search.apply(input, labels, enabled);
            picked = picked.or(applied.pick);
            !applied.consumed
        });
    });
    ui.data_mut(|d| d.insert_temp(id, search.clone()));
    if let Some(pick) = picked {
        ui.data_mut(|d| d.insert_temp(id.with("pick"), pick));
    }
    let mut filter = Filter { id: Some(id), ..Filter::default() };
    if search.is_active() {
        filter.shown = matches(&search.query, labels, enabled);
        filter.active = filter.shown.get(search.active).copied();
        filter.moved = search != before;
        filter.query = search.query;
    }
    filter
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

/// The slim field: a magnifier, the typed text and a caret.
fn field(ui: &mut Ui, query: &str) {
    let v = ui.visuals();
    let (fill, stroke, radius) = (v.extreme_bg_color, v.widgets.inactive.bg_stroke, v.widgets.inactive.corner_radius);
    let (text, faint) = (v.text_color(), v.weak_text_color());
    egui::Frame::new().fill(fill).stroke(stroke).corner_radius(radius).inner_margin(egui::Margin::symmetric(5, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (icon, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            crate::icons::paint(ui, icon, "search", 13.0, faint);
            let label = ui.add(egui::Label::new(egui::RichText::new(query).color(text)).truncate());
            let x = label.rect.right() + 1.0;
            ui.painter().line_segment([egui::pos2(x, label.rect.top() + 1.0), egui::pos2(x, label.rect.bottom() - 1.0)], egui::Stroke::new(1.0, text));
        });
    });
    ui.add_space(2.0);
}

#[cfg(test)]
mod tests;
