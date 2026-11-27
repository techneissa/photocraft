//! The fork's key layer in front of craft's keymap: it reads the frame's key presses before the
//! runtime's key stage does and takes only the ones it owns. Everything else reaches craft unchanged.
//!
//! It owns three kinds of key:
//! - the first step of a chord (the leader): pressing it arms the chord, the next key finishes it;
//!   Esc, a key that completes no chord, or [`TIMEOUT`] seconds without a key cancel it;
//! - the keys of a command whose When condition holds: the command runs as the key's own;
//! - the keys of a command whose When condition does not hold go on to craft, which does not bind a command
//!   with a condition (`unbind_gated`): the next command with that key answers, or none.
//!
//! The commands it fires wait in [`super::State::fired`] and run in `shortcuts::handle`, as the
//! commands craft's keys fire do.

use egui::{Event, Key, KeyboardShortcut, Modifiers};

use super::focus::{Focus, is_modifier_key, key_matches};
use super::{chord, context, gate};
use crate::PhotocraftApp;

/// Seconds the leader key waits for the next key.
pub const TIMEOUT: f64 = 4.0;

/// A chord whose first step was pressed.
#[derive(Clone, Copy, Debug)]
pub struct Pending {
    pub first: KeyboardShortcut,
    /// `egui` time of the press.
    pub since: f64,
}

/// `m` without the modifiers `held` (the leader's, still down when the second key comes).
fn without(m: Modifiers, held: Modifiers) -> Modifiers {
    Modifiers {
        alt: m.alt && !held.alt,
        ctrl: m.ctrl && !held.ctrl,
        shift: m.shift && !held.shift,
        mac_cmd: m.mac_cmd && !held.mac_cmd,
        command: m.command && !held.command,
    }
}

/// Where the keyboard is for a leader: inline type editing keeps plain keys like a text box.
fn leader_focus(app: &PhotocraftApp, focus: Focus) -> Focus {
    if app.ui.text_edit.is_some() { Focus::Text } else { focus }
}

/// Whether the chord of `id` may start with the keyboard where `focus` says. A text field keeps its
/// keys (⌘K then `c` types the `c` there) unless the chord's condition asks for `textFocus`.
fn arms(app: &PhotocraftApp, focus: Focus, id: &str) -> bool {
    focus != Focus::Text || photocraft_engine::prefs::when_of(&app.session.prefs().shortcut_when, id).is_some_and(|e| e.mentions(super::when::keys::TEXT_FOCUS))
}

/// A chord may wait while nothing modal owns the keyboard: no dialog, prompt, warp or palette.
fn leader_may_wait(app: &PhotocraftApp) -> bool {
    !app.dialog_open() && app.discard.is_none() && !app.distort.active() && !app.ui.palette.open && !crate::prefs_ui::shortcuts_open(app)
}

/// What one key press did to the layer.
#[derive(Debug, PartialEq, Eq)]
enum Taken {
    /// Not ours: it goes on to craft.
    Pass,
    /// Used up (a leader pressed, a chord cancelled).
    Swallow,
    /// Used up, and this command runs.
    Fire(String),
}

/// Takes the key presses of this frame's raw input that the layer owns (call from `raw_input_hook`, after the
/// clipboard keys became presses). The commands they fire wait in `app.keybinds.fired`.
pub fn filter(app: &mut PhotocraftApp, ctx: &egui::Context, raw: &mut egui::RawInput) {
    if crate::prefs_ui::shortcuts_open(app) {
        // The shortcut recorders take every key; the Chords and Conditions tab's gets the presses and nothing runs.
        app.keybinds.pending = None;
        if ctx.data(|d| d.get_temp::<bool>(super::recording_id())).unwrap_or(false) {
            let mut taken: Vec<(Key, Modifiers)> = Vec::new();
            raw.events.retain(|e| match e {
                Event::Key { key, pressed: true, repeat: false, modifiers, .. } if !is_modifier_key(*key) => {
                    taken.push((*key, *modifiers));
                    false
                }
                Event::Key { pressed: true, .. } | Event::Text(_) => false,
                _ => true,
            });
            if !taken.is_empty() {
                ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(Key, Modifiers)>>(super::captured_id()).extend(taken));
                ctx.request_repaint();
            }
        }
        return;
    }
    let prefs = app.session.prefs();
    let idle = app.keybinds.pending.is_none() && prefs.chords.is_empty() && prefs.shortcut_when.is_empty();
    let pressed = |e: &Event| matches!(e, Event::Key { pressed: true, repeat: false, key, .. } if !is_modifier_key(*key));
    if idle || (app.keybinds.pending.is_none() && !raw.events.iter().any(pressed)) {
        return;
    }
    let now = raw.time.unwrap_or_else(|| ctx.input(|i| i.time));
    if let Some(p) = app.keybinds.pending
        && (now - p.since > TIMEOUT || !leader_may_wait(app))
    {
        app.keybinds.pending = None;
    }
    let focus = Focus::of(ctx);
    let mut kept = Vec::with_capacity(raw.events.len());
    let mut took = false;
    for event in std::mem::take(&mut raw.events) {
        let Event::Key { key, pressed: true, repeat: false, modifiers, .. } = &event else {
            kept.push(event);
            continue;
        };
        if is_modifier_key(*key) {
            kept.push(event);
            continue;
        }
        match take(app, focus, *key, *modifiers, now) {
            Taken::Pass => kept.push(event),
            Taken::Swallow => took = true,
            Taken::Fire(id) => {
                took = true;
                app.keybinds.fired.push(id);
            }
        }
    }
    if took {
        // The characters of the keys taken must not type anywhere.
        kept.retain(|e| !matches!(e, Event::Text(_)));
    }
    raw.events = kept;
    if app.keybinds.pending.is_some() {
        ctx.request_repaint_after(std::time::Duration::try_from_secs_f64(TIMEOUT + 0.05).unwrap_or_default());
    }
}

/// One fresh key press, in order: finishes a pending chord, arms one, or is a gated key.
fn take(app: &mut PhotocraftApp, focus: Focus, key: Key, mods: Modifiers, now: f64) -> Taken {
    if let Some(p) = app.keybinds.pending {
        return follow(app, focus, p, key, mods);
    }
    let modal = !leader_may_wait(app);
    let cx = context::read(app, leader_focus(app, focus));
    if !modal {
        let leader = gate::chords_in(app, &cx)
            .into_iter()
            .find(|(id, [first, _])| arms(app, focus, id) && leader_focus(app, focus).allows(first) && key_matches(first, key, mods))
            .map(|(_, [first, _])| first);
        if let Some(first) = leader {
            app.keybinds.pending = Some(Pending { first, since: now });
            return Taken::Swallow;
        }
    }
    gated(app, &cx, focus, modal, key, mods)
}

/// The key after a leader: finishes the chord, or cancels it.
fn follow(app: &mut PhotocraftApp, focus: Focus, p: Pending, key: Key, mods: Modifiers) -> Taken {
    let cx = context::read(app, leader_focus(app, focus));
    let next: Vec<(String, KeyboardShortcut)> =
        gate::chords_in(app, &cx).into_iter().filter(|(_, [first, _])| *first == p.first).map(|(id, [_, second])| (id, second)).collect();
    // the key as pressed first; failing that, with the modifiers of the leader still held taken off,
    // so ⌘K then C works with ⌘ kept down
    let hit = next
        .iter()
        .find(|(_, second)| key_matches(second, key, mods))
        .or_else(|| next.iter().find(|(_, second)| key_matches(second, key, without(mods, p.first.modifiers))))
        .map(|(id, _)| id.clone());
    app.keybinds.pending = None;
    match hit {
        Some(id) => Taken::Fire(id),
        None => {
            if !(key == Key::Escape && mods.is_none()) {
                let template = "{key} was pressed, but no shortcut continues with that key";
                app.ui.status = crate::i18n::fmt(tl!(template), &[("key", &chord::step_text(&p.first))]);
                app.ui.status_error = true;
            }
            Taken::Swallow
        }
    }
}

/// The keys of the commands that have a When condition, as pressable shortcuts: the main key and the further ones.
fn gated_keys(app: &PhotocraftApp) -> Vec<(String, KeyboardShortcut)> {
    app.session
        .prefs()
        .shortcut_when
        .keys()
        .flat_map(|id| {
            let main = crate::shortcuts::effective_shortcut(app, id, crate::shortcuts::default_shortcut(id).as_deref());
            main.into_iter().chain(crate::shortcuts::effective_alternates(app, id)).filter_map(|text| crate::shortcuts::parse(&text)).map(|sc| (id.clone(), sc))
        })
        .collect()
}

/// A key of a command with a When condition: it runs the command when the condition holds. When it does not, the key
/// is not taken: craft's keymap does not bind a command with a condition (`unbind_gated`), so the key goes to whatever
/// else has it. Under a modal only the commands whose condition asks for it work.
fn gated(app: &PhotocraftApp, cx: &super::when::Context, focus: Focus, modal: bool, key: Key, mods: Modifiers) -> Taken {
    let focus = leader_focus(app, focus);
    let works = |id: &str, sc: &KeyboardShortcut| {
        gate::holds(app, cx, id) && if modal { super::blocked::wants(app, cx, id) } else { focus.allows(sc) || super::blocked::wants(app, cx, id) }
    };
    match gated_keys(app).into_iter().find(|(id, sc)| key_matches(sc, key, mods) && works(id, sc)) {
        Some((id, _)) => Taken::Fire(id),
        None => Taken::Pass,
    }
}

/// The chords that continue the pending leader: the next step and its command, by command id.
pub fn continuations(app: &PhotocraftApp, focus: Focus, p: &Pending) -> Vec<(KeyboardShortcut, String)> {
    let cx = context::read(app, leader_focus(app, focus));
    let mut v: Vec<(KeyboardShortcut, String)> =
        gate::chords_in(app, &cx).into_iter().filter(|(_, [first, _])| *first == p.first).map(|(id, [_, second])| (second, id)).collect();
    v.sort_by(|a, b| a.1.cmp(&b.1));
    v
}

/// Runs the commands the layer fired (call from `shortcuts::handle`).
pub fn run_fired(app: &mut PhotocraftApp, ctx: &egui::Context) {
    for id in std::mem::take(&mut app.keybinds.fired) {
        crate::shortcut_dispatch::dispatch(app, ctx, &id);
    }
}
