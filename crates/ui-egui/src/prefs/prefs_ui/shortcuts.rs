//! Edit › Keyboard Shortcuts and Menus: the framework's Keyboard Shortcuts dialog over a draft
//! keymap, with PhotoCraft's Menus tab beside it.
//!
//! The session holds everything the dialog edits: the draft keymap, the shortcut sets, the Menus
//! draft and the text of a set waiting to be reviewed. Cancel drops it and the runtime never saw
//! a change; OK writes the preferences once ([`save_keys`], then the menus), and the runtime's
//! keymap follows the new shortcut preferences (`shortcut_dispatch::overrides`).
//!
//! The Toolbar tab is not here: Edit › Toolbar opens the toolbar drawer.

use photocraft_engine::actions_cmds::ActionsExt;
use std::collections::{BTreeMap, BTreeSet};

use craft::keymap::import::kys;
use craft::keymap::{Builtins, CommandRef, ImportOptions, Keymap, NoneDefined, Overrides, Profiles, Scopes, reserved};
use craft::menu::Row;
use craft::menu::shortcuts::{Area, ConflictMode, DialogState, DialogTab, Format, Options, Outcome, ShortcutWords, ShortcutsDialog, kys_error_text};
use craft::ui::choice::Checkbox;
use craft::ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft::ui::display::Text;
use craft::ui::layout_components::Row as LayoutRow;
use craft::ui::runtime::env::{Env, with_env};
use craft::ui::runtime::windows;
use photocraft_engine::keymap_defs;
use photocraft_engine::prefs::{MenuCustomization, Preferences};
use photocraft_engine::shortcut_sets::DEFAULTS_NAME;
use serde_json::{Map, Value, json};

use crate::PhotocraftApp;

/// The id of the `.kys` format in the dialog's import and export lists.
const KYS_FORMAT: &str = "kys";

/// The open Keyboard Shortcuts and Menus dialog, at `app.prefs_rt.shortcuts`.
pub(crate) struct ShortcutsSession {
    /// The dialog's state between passes.
    pub(crate) state: DialogState,
    /// The shortcut sets as the dialog edits them.
    pub(crate) profiles: Profiles,
    /// The draft the dialog edits in place: the rows the runtime catalog binds, with the
    /// preferences' bindings. The runtime's own keymap is not this one.
    pub(crate) keymap: Keymap,
    /// The Menus tab's draft.
    pub(crate) menus: MenusDraft,
    /// The Chords and Conditions tab's draft (fork/shortcut-upgrade.md).
    pub(crate) chords: super::shortcuts_chords::ChordDraft,
    /// The text of a set to review on the next pass, taken by that pass alone: a second pass of
    /// the same frame finds nothing to review.
    pub(crate) pending_import: Option<String>,
    /// The user's answers to earlier import questions, for the next import of this session.
    pub(crate) remembered: ImportAnswers,
}

/// The user's answers to an import's questions: the command of each foreign entry and the chord text of each key or
/// modifier code, `None` for "No equivalent".
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ImportAnswers {
    pub(crate) targets: BTreeMap<String, Option<CommandRef>>,
    pub(crate) codes: BTreeMap<String, Option<String>>,
}

/// What the Menus tab edits: the hidden items and the colour of each item, by command id.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MenusDraft {
    pub(crate) hidden: BTreeSet<String>,
    pub(crate) colors: BTreeMap<String, String>,
}

impl MenusDraft {
    fn of(menus: &MenuCustomization) -> Self {
        MenusDraft { hidden: menus.hidden.iter().cloned().collect(), colors: menus.colors.clone() }
    }
}

/// The built-in sets: PhotoCraft's defaults.
fn builtins() -> [(&'static str, Overrides); 1] {
    [(DEFAULTS_NAME, Overrides::new())]
}

/// The saved sets of `prefs` as the dialog's `Profiles`. The defaults are the active set when the
/// preferences name none.
pub(crate) fn profiles_of(prefs: &Preferences) -> Profiles {
    let sets: Vec<Value> = prefs.shortcut_sets.iter().map(|(name, set)| json!({"name": name, "overrides": keymap_defs::set_overrides(set)})).collect();
    serde_json::from_value(json!({"active": prefs.shortcut_set, "sets": sets})).unwrap_or_default()
}

/// The keymap the dialog and the offer edit: the rows the runtime catalog binds, the platform the
/// runtime's keymap is built for, and the preferences' bindings as overrides.
pub(crate) fn draft_keymap(app: &PhotocraftApp) -> Keymap {
    let platform = windows::os_of(app.ctx());
    // The runtime catalog's held rows while they are current; built here before the first frame holds them.
    let rows = match app.framework.catalog_rows_at(app.session.actions().list.revision()) {
        Some(rows) => rows.to_vec(),
        None => crate::shortcut_dispatch::command_rows(app),
    };
    Keymap::new(rows.into_iter().chain(crate::shortcut_dispatch::bridged_rows()).collect(), Scopes::new(), platform)
        .with_reserved(reserved::system(platform))
        .with_overrides(keymap_defs::prefs_overrides(app.session.prefs()))
}

/// How a `.kys` set is read for review and for the offer: rows without a key leave their command
/// alone, and the user's earlier answers come first.
pub(crate) fn import_options(answers: &ImportAnswers) -> ImportOptions {
    ImportOptions {
        remembered: answers.targets.clone(),
        codes: answers.codes.clone(),
        none_defined: NoneDefined::Keep,
        suggest: true,
        ..ImportOptions::default()
    }
}

/// The text of a set file's bytes, or why it is not one.
pub(crate) fn kys_text(name: &str, bytes: &[u8]) -> Result<String, String> {
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| format!("{name}: not a keyboard shortcut set (.kys): not UTF-8"))
}

/// Opens the dialog (when it is not open) on tab `tab`: 0 Keyboard Shortcuts, 1 Menus.
pub fn open(app: &mut PhotocraftApp, tab: u64) {
    if app.prefs_rt.shortcuts.is_some() {
        return;
    }
    let prefs = app.session.prefs();
    let session = ShortcutsSession {
        state: if tab == 1 { DialogState::new().with_tab("menus") } else { DialogState::new() },
        profiles: profiles_of(prefs),
        keymap: draft_keymap(app),
        menus: MenusDraft::of(&prefs.menus),
        chords: super::shortcuts_chords::ChordDraft::of(app),
        pending_import: None,
        remembered: ImportAnswers::default(),
    };
    app.prefs_rt.shortcuts = Some(session);
}

/// Opens the dialog (or uses the open one) on the Keyboard Shortcuts tab with command `id` selected and scrolled into
/// view: the entry point of Assign Shortcut… (fork/assign-shortcut-palette.md). False when the dialog does not list `id`.
pub fn open_shortcuts_for(app: &mut PhotocraftApp, id: &str) -> bool {
    open(app, 0);
    let Some(session) = app.prefs_rt.shortcuts.as_mut() else { return false };
    let command = CommandRef::new(id);
    let known = session.keymap.bindable(&command).is_some();
    if known {
        session.state.select(&command);
    }
    known
}

/// Whether the dialog is open: the runtime's keys step aside for its modal.
pub fn shortcuts_open(app: &PhotocraftApp) -> bool {
    app.prefs_rt.shortcuts.is_some()
}

/// The id the control channel knows the dialog by: Edit › Keyboard Shortcuts and Edit › Menus reply with it,
/// `ui.inspect` lists the dialog under it, and `ui.dialog.cancel` closes it. The dialog draws outside the framework's
/// dialog host, which has no id for it (#15544).
pub(crate) const CONTROL_ID: &str = "shortcuts";

/// The reply of a command that opened the dialog.
pub(crate) fn control_reply() -> Value {
    json!({"dialog": CONTROL_ID})
}

/// The dialog as `ui.inspect` lists it among the dialogs while it is open, in the host's entry shape.
pub(crate) fn inspect(app: &PhotocraftApp) -> Option<Value> {
    shortcuts_open(app).then(|| json!({"id": CONTROL_ID, "key": "edit.keyboardShortcuts", "parent": null, "fields": {}}))
}

/// Cancel for a control client, as the dialog's Cancel: the session goes and nothing it edited is saved. False with
/// the dialog closed.
pub(crate) fn cancel(app: &mut PhotocraftApp) -> bool {
    app.prefs_rt.shortcuts.take().is_some()
}

/// Whether the dialog is recording a chord: the key goes to the recorder and runs nothing, and the
/// native menu bar pauses its key equivalents.
pub fn recording(app: &PhotocraftApp) -> bool {
    app.prefs_rt.shortcuts.as_ref().is_some_and(|session| session.state.is_recording())
}

/// Queues the text of a set for review on the dialog's next pass.
pub(crate) fn queue_import(app: &mut PhotocraftApp, text: String) {
    open(app, 0);
    if let Some(session) = app.prefs_rt.shortcuts.as_mut() {
        session.pending_import = Some(text);
    }
}

/// The dialog's options: Photoshop's Shortcuts For areas, the conflict bar, `.kys` files in and
/// out, and the summary.
fn options() -> Options {
    let kys = Format::new(KYS_FORMAT, "Photoshop Keyboard Shortcuts (.kys)", &["kys", "psp"]);
    Options::default().with(|o| {
        o.title = Some(tl!("Keyboard Shortcuts and Menus").to_string());
        o.areas = Area::photoshop();
        o.conflicts = ConflictMode::Ask;
        o.import_formats = vec![kys];
        o.export_formats = vec![kys];
        o.summary = true;
    })
}

/// The dialog over the session's parts for one pass.
fn dialog<'a>(keymap: &'a mut Keymap, profiles: &'a mut Profiles, builtins: &'a Builtins<'a>, options: &'a Options) -> ShortcutsDialog<'a> {
    ShortcutsDialog::new(keymap).sets(profiles, builtins).options(options)
}

/// One pass of the dialog while it is open.
pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    pass(app, ui, false);
}

/// One pass; with `press_ok` the pass presses OK first. The session is out of `app.prefs_rt`
/// during the pass, so nothing in the pass reads it through [`recording`].
fn pass(app: &mut PhotocraftApp, ui: &mut egui::Ui, press_ok: bool) {
    let Some(mut session) = app.prefs_rt.shortcuts.take() else { return };
    let rows = crate::menus::base_rows(app);
    let options = options();
    let builtins = builtins();
    let ShortcutsSession { state, profiles, keymap, menus, chords, pending_import, remembered } = &mut session;
    if let Some(text) = pending_import.take() {
        review(ui, state, dialog(keymap, profiles, &builtins, &options), &text, remembered);
    }
    if press_ok {
        dialog(keymap, profiles, &builtins, &options).ok(ui, state);
    }
    let mut tabs = [super::shortcuts_chords::tab(chords), menus_tab(&rows, menus)];
    let output = dialog(keymap, profiles, &builtins, &options).tabs(&mut tabs).show(ui, state);
    drop(tabs);
    match output.outcome {
        Some(Outcome::Ok) => {
            if let Err(e) = save_keys_with(app, keymap, profiles, (&chords.chords, &chords.whens)).and_then(|_| save_menus(app, menus)) {
                app.ui.status = crate::i18n::fmt(tl!("The keyboard shortcuts were not saved: {error}"), &[("error", &e)]);
                app.ui.status_error = true;
            }
        }
        Some(Outcome::Import { .. }) => ask_import(app, state),
        Some(Outcome::Export { name, json, .. }) => save_text(app, state, &format!("{name}.json"), json),
        Some(Outcome::ExportAs { format, name, keymap: exported, .. }) if format == KYS_FORMAT => {
            save_text(app, state, &format!("{name}.kys"), kys::write(&name, &exported));
        }
        Some(Outcome::Summary { name, html, .. }) => save_text(app, state, &format!("{name}.html"), html),
        Some(Outcome::Remembered { targets, codes, .. }) => {
            remembered.targets.extend(targets);
            remembered.codes.extend(codes);
        }
        _ => {}
    }
    super::shortcuts_chords::run_ask(app, chords);
    // An import reopens the dialog on what it read: that session stays.
    if state.is_open() && app.prefs_rt.shortcuts.is_none() {
        app.prefs_rt.shortcuts = Some(session);
    }
}

/// Opens the review of the set `text` holds (a `.kys` file, or craft-keymap JSON), or shows why it
/// cannot be read.
fn review(ui: &egui::Ui, state: &mut DialogState, dialog: ShortcutsDialog<'_>, text: &str, remembered: &ImportAnswers) {
    if text.trim_start().starts_with(['{', '[']) {
        if let Err(e) = dialog.import_json(ui, state, text) {
            state.show_error(e.to_string());
        }
        return;
    }
    match kys::read(text) {
        Ok(set) => dialog.review_import(ui, state, set, import_options(remembered)),
        Err(e) => state.show_error(kys_error_text(&e, &Env::of(ui).words::<ShortcutWords>())),
    }
}

/// Import…: ask for the file; the text is reviewed on the pass after it arrives.
fn ask_import(app: &mut PhotocraftApp, state: &mut DialogState) {
    let picked = app.pick_file_bytes_filtered(&["kys", "psp", "json"], |app, name, bytes| {
        let text = kys_text(&name, &bytes)?;
        queue_import(app, text);
        Ok(control_reply())
    });
    if let Err(e) = picked {
        state.show_error(e);
    }
}

/// Saves `text` through the save dialog under the suggested file name.
fn save_text(app: &mut PhotocraftApp, state: &mut DialogState, suggested: &str, text: String) {
    // A set name can hold characters a file name can't.
    let name: String = suggested.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    let saved = app.pick_save(&name, move |app, path| {
        let write = app.services.write.as_mut().ok_or("no writer configured")?;
        write(&path, text.as_bytes())?;
        Ok(json!({"path": path}))
    });
    if let Err(e) = saved {
        state.show_error(e);
    }
}

/// Writes the bindings and sets of the dialog (or of an import) to the preferences in one
/// `edit.keyboardShortcuts` call: the one writer of the shortcut preferences. An action's function
/// key belongs to the action, not to a set, so the preferences' own stay whatever set was chosen.
pub(crate) fn save_keys(app: &mut PhotocraftApp, keymap: &Keymap, profiles: &Profiles) -> Result<Value, String> {
    // The chords and conditions stay as they are: a reset clears them, so they are written back.
    let (chords, whens) = (app.session.prefs().chords.clone(), app.session.prefs().shortcut_when.clone());
    save_keys_with(app, keymap, profiles, (&chords, &whens))
}

/// [`save_keys`] with the chords and When conditions of the Chords and Conditions tab (command id → text).
pub(crate) fn save_keys_with(
    app: &mut PhotocraftApp,
    keymap: &Keymap,
    profiles: &Profiles,
    (chords, whens): (&BTreeMap<String, String>, &BTreeMap<String, String>),
) -> Result<Value, String> {
    let (mut set, mut alternates) = keymap_defs::overrides_to_prefs(keymap.overrides());
    let prefs = app.session.prefs();
    let is_action = |id: &String| id.starts_with(photocraft_engine::actions_cmds::PLAY_SHORTCUT_PREFIX);
    set.retain(|id, _| !is_action(id));
    alternates.retain(|id, _| !is_action(id));
    set.extend(prefs.shortcuts.iter().filter(|(id, _)| is_action(id)).map(|(id, key)| (id.clone(), key.clone())));
    alternates.extend(prefs.alternate_shortcuts.iter().filter(|(id, _)| is_action(id)).map(|(id, keys)| (id.clone(), keys.clone())));
    let builtins = builtins();
    let active = profiles.active_name(&builtins);
    // OK saves the dialog's bindings into the set in use.
    let sets: Map<String, Value> = profiles
        .sets()
        .iter()
        .map(|(name, overrides)| {
            let overrides = if name.as_str() == active { keymap.overrides() } else { overrides };
            let (mut shortcuts, mut alternate_shortcuts) = keymap_defs::overrides_to_prefs(overrides);
            shortcuts.retain(|id, _| !is_action(id));
            alternate_shortcuts.retain(|id, _| !is_action(id));
            (name.clone(), json!({"shortcuts": shortcuts, "alternateShortcuts": alternate_shortcuts}))
        })
        .collect();
    let active = if active == DEFAULTS_NAME { "" } else { active };
    let call = json!({"reset": true, "set": set, "alternates": alternates, "sets": sets, "activeSet": active, "allowUnknown": true, "removeConflicts": false, "chords": chords, "when": whens});
    app.run("edit.keyboardShortcuts", call).map_err(|e| e.to_string())
}

/// Writes the Menus tab's draft to the preferences.
fn save_menus(app: &mut PhotocraftApp, menus: &MenusDraft) -> Result<Value, String> {
    let call = json!({"values": {"menus": {"hidden": menus.hidden, "colors": menus.colors}}});
    app.run("prefs.set", call).map_err(|e| e.to_string())
}

/// The Menus tab: a row per menu item with a Visible check and a colour.
fn menus_tab<'a>(rows: &'a [Row], draft: &'a mut MenusDraft) -> DialogTab<'a> {
    DialogTab::new("menus", tl!("Menus").to_string(), move |ui| menus_body(ui, rows, &mut *draft))
}

/// The Menus tab's body; true when the draft changed. `rows` are the menu items before the
/// customization, so a hidden item is listed here to be shown again.
fn menus_body(ui: &mut egui::Ui, rows: &[Row], draft: &mut MenusDraft) -> bool {
    let before = draft.clone();
    if ui.button(tl!("Show All Menu Items")).clicked() {
        draft.hidden.clear();
    }
    let items: Vec<&Row> = rows.iter().filter(|row| row.label != "---").collect();
    let t = crate::theme::Tokens::get(ui.ctx());
    let height = ui.spacing().interact_size.y;
    // PLAN: the framework `Table` draws a value in each cell and cannot hold a Checkbox or a Select, so the rows are laid out by hand.
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, height, items.len(), |ui, range| {
        for row in &items[range] {
            let id = row.command.id.as_str();
            with_env(
                ui,
                |set| set.id_salt = Some(egui::Id::unique(id)),
                |ui| {
                    LayoutRow::new().show(ui, |ui, _| {
                        // The name has a column of its own, so the controls line up from row to row.
                        ui.allocate_ui(egui::vec2(540.0, height), |ui| {
                            ui.set_min_width(540.0);
                            Text::new(format!("{} › {}", row.path.join(" › "), row.label)).show(ui);
                        });
                        // Items that can't be hidden (File › Close, Save…, Edit › Menus…) have no visibility
                        // control at all, as in the reference application (#5096, #14335).
                        if MenuCustomization::hideable(id) {
                            let visible = !draft.hidden.contains(id);
                            let shown = Checkbox::new(Bind::given(visible)).label(tl!("Visible").to_string()).show(ui);
                            match shown.events.value() {
                                Some(&true) => {
                                    draft.hidden.remove(id);
                                }
                                Some(&false) => {
                                    draft.hidden.insert(id.to_owned());
                                }
                                None => {}
                            }
                        } else {
                            ui.add_space(62.0);
                        }
                        let mut color = draft.colors.get(id).map_or("none", String::as_str).to_owned();
                        if super::shortcuts_menus::color_dot(ui, &t, &mut color) {
                            if color == "none" {
                                draft.colors.remove(id);
                            } else {
                                draft.colors.insert(id.to_owned(), color);
                            }
                        }
                    });
                },
            );
        }
    });
    *draft != before
}

/// Test hook: adds `binding` to `command` in the open dialog's draft keymap, as the dialog's own edit does.
#[cfg(test)]
pub(crate) fn assign(
    app: &mut PhotocraftApp,
    command: &CommandRef,
    binding: craft::keymap::Binding,
    policy: craft::keymap::Policy,
) -> Result<craft::keymap::Assigned, craft::keymap::AssignError> {
    let session = app.prefs_rt.shortcuts.as_mut().ok_or(craft::keymap::AssignError::UnknownCommand)?;
    session.keymap.assign(command, binding, policy)
}

/// Test hook: makes `binding` the only binding of `command` in the open dialog's draft keymap, as recording over its
/// key in the dialog does.
#[cfg(test)]
pub(crate) fn rebind(
    app: &mut PhotocraftApp,
    command: &CommandRef,
    binding: craft::keymap::Binding,
    policy: craft::keymap::Policy,
) -> Result<craft::keymap::Assigned, craft::keymap::AssignError> {
    let session = app.prefs_rt.shortcuts.as_mut().ok_or(craft::keymap::AssignError::UnknownCommand)?;
    session.keymap.set_bindings(command, vec![binding], policy)
}

/// Test hook: presses OK in the open dialog and handles the outcome, in one pass on `ctx`.
#[cfg(test)]
pub(crate) fn press_ok(app: &mut PhotocraftApp, ctx: &egui::Context) {
    crate::testing::run_ui(ctx, Default::default(), |ui| pass(app, ui, true));
}
