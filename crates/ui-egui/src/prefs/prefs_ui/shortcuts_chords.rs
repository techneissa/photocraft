//! The fork's Chords and Conditions tab of Keyboard Shortcuts and Menus: a chord (two keys in a row, `Cmd+K C`)
//! and a When condition for each command, a clash note, and the JSON backup (export and import).
//!
//! The tab edits a draft the session holds ([`ChordDraft`]); OK writes it with the keys (`save_keys`). The keys
//! themselves are craft's dialog's; the fork's key layer (`keybinds`) acts on what this tab saves.
//! While a chord is recorded the layer hands the presses to the tab and runs nothing.

use std::collections::BTreeMap;

use craft::menu::shortcuts::DialogTab;
use egui::{Color32, Key, Modifiers, RichText, Sense, vec2};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::keybinds::{captured_id, chord, recording_id};

/// A command of the list: id, label as shown, menu path as shown, the keys it has now.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) path: String,
    pub(crate) keys: Vec<String>,
}

/// What the tab edits: the chords and conditions by command id, the list, the search and the recording.
#[derive(Clone, Debug, Default)]
pub(crate) struct ChordDraft {
    pub(crate) chords: BTreeMap<String, String>,
    pub(crate) whens: BTreeMap<String, String>,
    pub(crate) rows: Vec<Row>,
    pub(crate) filter: String,
    /// The command being recorded and the steps pressed so far.
    pub(crate) recording: Option<(String, Vec<String>)>,
    pub(crate) message: String,
    /// What the buttons asked this pass.
    pub(crate) ask: Ask,
}

/// A button of the tab, taken by `shortcuts::pass`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Ask {
    #[default]
    Nothing,
    Export,
    Import,
}

impl ChordDraft {
    /// The draft of the saved preferences, with the list of the commands the app has.
    pub(crate) fn of(app: &PhotocraftApp) -> Self {
        let prefs = app.session.prefs();
        let lang = crate::i18n::current();
        let rows = crate::shortcut_dispatch::command_rows(app)
            .into_iter()
            .map(|def| {
                let id = def.command.id.to_string();
                let label = crate::i18n::tr_id(lang, &id, &def.label).to_string();
                let parts = def.paths.first().cloned().unwrap_or_default();
                let path = parts.iter().take(parts.len().saturating_sub(1)).map(|p| crate::i18n::tr(lang, p).to_string()).collect::<Vec<_>>().join(" › ");
                let main = crate::shortcuts::effective_shortcut(app, &id, crate::shortcuts::default_shortcut(&id).as_deref());
                let keys = main.into_iter().chain(crate::shortcuts::effective_alternates(app, &id)).collect();
                Row { id, label, path, keys }
            })
            .collect();
        ChordDraft { chords: prefs.chords.clone(), whens: prefs.shortcut_when.clone(), rows, ..ChordDraft::default() }
    }

    /// Why command `id`'s condition text can't be used, if so.
    fn error(&self, id: &str) -> Option<String> {
        self.whens.get(id).and_then(|text| photocraft_engine::when::parse(text).err().map(|e| e.to_string()))
    }

    /// The note for command `id`: its chord's first key is another command's key, or two commands share the chord.
    fn clash(&self, id: &str) -> Option<String> {
        let chord_text = self.chords.get(id)?;
        let [first, _] = photocraft_engine::prefs::chord_steps(chord_text)?;
        let overlap = |other: &str| photocraft_engine::prefs::can_overlap(&self.whens, id, other);
        let key_of = |row: &&Row| row.id != id && row.keys.iter().any(|k| photocraft_engine::prefs::normalize_shortcut(k).as_deref() == Some(first.as_str()));
        let name = |other: &str| self.rows.iter().find(|r| r.id == other).map_or_else(|| other.to_string(), |r| r.label.clone());
        if let Some(other) = self.rows.iter().filter(key_of).find(|r| overlap(&r.id)) {
            let template = tl!("{key} is also the shortcut of {command}: it waits for the chord's second key.");
            return Some(crate::i18n::fmt(template, &[("key", &chord::text(&first)), ("command", &name(&other.id))]));
        }
        let other = self.chords.iter().find(|(other, c)| other.as_str() != id && *c == chord_text && overlap(other)).map(|(other, _)| other.clone())?;
        Some(crate::i18n::fmt(tl!("{key} is also used by {command} in the same context."), &[("key", &chord::text(chord_text)), ("command", &name(&other))]))
    }

    /// The presses of a recording: a second press finishes the chord, Esc cancels, ⌫ clears the command's chord.
    pub(super) fn feed(&mut self, presses: Vec<(Key, Modifiers)>) {
        for (key, mods) in presses {
            let Some((id, steps)) = self.recording.as_mut() else { return };
            if key == Key::Escape && mods.is_none() {
                self.recording = None;
                return;
            }
            if matches!(key, Key::Backspace | Key::Delete) && mods.is_none() && steps.is_empty() {
                let id = id.clone();
                self.chords.remove(&id);
                self.recording = None;
                return;
            }
            let Some(text) = chord::shortcut_text(key, mods) else { continue };
            steps.push(text);
            if steps.len() == 2 {
                let (id, steps) = (id.clone(), steps.join(" "));
                match photocraft_engine::prefs::normalize_chord(&steps) {
                    Some(c) => _ = self.chords.insert(id, c),
                    None => self.message = tl!("That is not a chord: press two keys in a row.").to_string(),
                }
                self.recording = None;
                return;
            }
        }
    }

    fn matches(&self, row: &Row, needle: &str) -> bool {
        let hay = format!(
            "{} {} {} {} {}",
            row.path,
            row.label,
            row.id,
            self.chords.get(&row.id).map_or("", String::as_str),
            self.whens.get(&row.id).map_or("", String::as_str)
        );
        let keys = row.keys.iter().map(|k| crate::shortcuts::pretty(k)).collect::<Vec<_>>().join(" ");
        let hay = format!("{hay} {keys}").to_lowercase();
        needle.split_whitespace().all(|word| hay.contains(word))
    }
}

/// The tab, over the session's draft.
pub(super) fn tab(draft: &mut ChordDraft) -> DialogTab<'_> {
    DialogTab::new("chords", tl!("Chords and Conditions").to_string(), move |ui| body(ui, draft))
}

/// The tab's body; true when the draft changed.
fn body(ui: &mut egui::Ui, d: &mut ChordDraft) -> bool {
    let before = (d.chords.clone(), d.whens.clone());
    let ctx = ui.ctx().clone();
    ctx.data_mut(|data| data.insert_temp(recording_id(), d.recording.is_some()));
    let presses: Vec<(Key, Modifiers)> = ctx.data_mut(|data| data.remove_temp(captured_id()).unwrap_or_default());
    if d.recording.is_some() {
        d.feed(presses);
        ctx.data_mut(|data| data.insert_temp(recording_id(), d.recording.is_some()));
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut d.filter)
                .desired_width((ui.available_width() - 150.0).max(120.0))
                .hint_text(tl!("Search commands, chords, conditions")),
        );
        if ui.button(tl!("Export")).on_hover_text(tl!("Export Shortcuts as JSON…")).clicked() {
            d.ask = Ask::Export;
        }
        if ui.button(tl!("Import")).on_hover_text(tl!("Import Shortcuts from JSON…")).clicked() {
            d.ask = Ask::Import;
        }
    });
    ui.label(
        RichText::new(tl!(
            "A chord is two keys in a row, like ⌘K then C. A condition decides where a command's keys work, like tool == brush && hasSelection."
        ))
        .size(11.5)
        .weak(),
    );
    if !d.message.is_empty() {
        ui.colored_label(Color32::from_rgb(230, 120, 80), d.message.clone());
    }
    let needle = d.filter.trim().to_lowercase();
    let shown: Vec<usize> = (0..d.rows.len()).filter(|i| d.matches(&d.rows[*i], &needle)).collect();
    let height = ui.spacing().interact_size.y + 4.0;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, height, shown.len(), |ui, range| {
        for &at in &shown[range] {
            let row = d.rows[at].clone();
            ui.horizontal(|ui| {
                let name_width = (ui.available_width() * 0.38).max(160.0);
                ui.allocate_ui(vec2(name_width, height), |ui| {
                    ui.set_min_width(name_width);
                    let text = if row.path.is_empty() { row.label.clone() } else { format!("{} › {}", row.path, row.label) };
                    ui.add(egui::Label::new(text).wrap());
                });
                let recording = d.recording.as_ref().is_some_and(|(id, _)| *id == row.id);
                let text = match (&d.recording, d.chords.get(&row.id)) {
                    (Some((id, steps)), _) if *id == row.id => {
                        if steps.is_empty() {
                            tl!("Press two keys…").to_string()
                        } else {
                            format!("{} …", chord::text(&steps.join(" ")))
                        }
                    }
                    (_, Some(c)) => chord::text(c),
                    _ => tl!("None").to_string(),
                };
                let button = egui::Button::new(RichText::new(text).size(12.0)).min_size(vec2(130.0, 20.0)).selected(recording);
                if ui.add(button).on_hover_text(tl!("Click, then press two keys in a row. Esc cancels, ⌫ removes the chord.")).clicked() {
                    d.recording = if recording { None } else { Some((row.id.clone(), Vec::new())) };
                    d.message.clear();
                }
                let note = d.clash(&row.id);
                let (rect, response) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                if let Some(note) = note {
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "⚠", egui::FontId::proportional(14.0), Color32::from_rgb(235, 170, 60));
                    response.on_hover_text(note);
                }
                let mut when = d.whens.get(&row.id).cloned().unwrap_or_default();
                let error = d.error(&row.id);
                let edit = egui::TextEdit::singleline(&mut when)
                    .id_salt(("when", &row.id))
                    .desired_width(ui.available_width())
                    .hint_text(tl!("Always"))
                    .text_color_opt(error.as_ref().map(|_| Color32::from_rgb(235, 100, 90)));
                let response = ui.add(edit);
                let response = match &error {
                    Some(e) => response.on_hover_text(e.clone()),
                    None => response,
                };
                if response.changed() {
                    if when.trim().is_empty() {
                        d.whens.remove(&row.id);
                    } else {
                        d.whens.insert(row.id.clone(), when);
                    }
                }
            });
        }
    });
    (d.chords.clone(), d.whens.clone()) != before
}

/// Does what Export and Import asked, once; a failure is the tab's message.
pub(super) fn run_ask(app: &mut PhotocraftApp, draft: &mut ChordDraft) {
    let ask = std::mem::take(&mut draft.ask);
    let result = match ask {
        Ask::Nothing => return,
        Ask::Export => export(app, draft),
        Ask::Import => app.pick_file_bytes_filtered(&["json"], |app, name, bytes| import(app, &name, &bytes)),
    };
    if let Err(e) = result {
        draft.message = e;
    }
}

/// Export…: the saved keys with the draft's chords and conditions, through the save panel.
fn export(app: &mut PhotocraftApp, d: &ChordDraft) -> Result<Value, String> {
    let mut prefs = app.session.prefs().clone();
    prefs.chords = d.chords.clone();
    prefs.shortcut_when = d.whens.clone();
    let mut text = serde_json::to_string_pretty(&photocraft_engine::shortcut_backup::to_json(&prefs)).map_err(|e| e.to_string())?;
    text.push('\n');
    app.pick_save("PhotoCraft Shortcuts.json", move |app, path| {
        let write = app.services.write.as_mut().ok_or("no writer configured")?;
        write(&path, text.as_bytes())?;
        Ok(json!({"path": path}))
    })
}

/// A shortcuts file's bytes: refused with the engine's reason, or applied at once and the dialog reopened on it.
fn import(app: &mut PhotocraftApp, file_name: &str, bytes: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "This isn't a PhotoCraft shortcuts file: it isn't text".to_string())?;
    let reply = app.run("edit.keyboardShortcuts", json!({"importJson": text})).map_err(|e| e.to_string())?;
    app.prefs_rt.shortcuts = None;
    super::shortcuts::open(app, 0);
    if let Some(session) = app.prefs_rt.shortcuts.as_mut() {
        session.chords.message = crate::i18n::fmt(tl!("Imported {file}. The shortcuts are saved."), &[("file", file_name)]);
    }
    Ok(reply)
}
