//! The leader key: pressing the first step of a chord arms it, the next key finishes it. Esc, a
//! key that completes no chord, or [`TIMEOUT`] seconds without a key cancel it.

use egui::{Event, Key, KeyboardShortcut, Modifiers};

use super::{chord, context, gate};
use crate::PhotocraftApp;
use crate::shortcut_dispatch::Focus;
use crate::shortcuts::key_matches;

/// Seconds the leader key waits for the next key.
pub const TIMEOUT: f64 = 4.0;

/// A chord whose first step was pressed.
#[derive(Clone, Copy, Debug)]
pub struct Pending {
    pub first: KeyboardShortcut,
    /// `egui` time of the press.
    pub since: f64,
}

/// What the key pass does after [`poll`].
#[derive(Debug, PartialEq, Eq)]
pub enum Poll {
    /// Not a chord key: the normal shortcuts go on.
    Pass,
    /// The keys were used up (a leader pressed, a chord cancelled): nothing else fires this frame.
    Swallow,
    /// A chord finished: run this command.
    Fire(String),
}

/// The keys freshly pressed this frame (not the repeats of a key held down, not lone modifiers).
fn fresh_presses(i: &egui::InputState) -> Vec<(Key, Modifiers)> {
    i.events
        .iter()
        .filter_map(|e| match e {
            Event::Key { key, pressed: true, repeat: false, modifiers, .. } if !crate::prefs_ui::is_modifier_key(*key) => Some((*key, *modifiers)),
            _ => None,
        })
        .collect()
}

/// Remove this frame's key presses, so nothing else sees the ones the chord took.
fn consume_keys(ctx: &egui::Context) {
    ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { pressed: true, .. } | Event::Text(_) | Event::Copy | Event::Cut | Event::Paste(_))));
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

/// Advance the chord state with this frame's keys (call once per frame, before the single-step
/// shortcuts, while nothing else owns the keyboard).
pub fn poll(app: &mut PhotocraftApp, ctx: &egui::Context, focus: Focus) -> Poll {
    if app.keybinds.pending.is_none() && app.session.prefs().chords.is_empty() {
        return Poll::Pass;
    }
    let now = ctx.input(|i| i.time);
    let presses = ctx.input(fresh_presses);
    if let Some(p) = app.keybinds.pending {
        if now - p.since > TIMEOUT || !leader_may_wait(app) {
            app.keybinds.pending = None;
        } else {
            return follow(app, ctx, focus, p, &presses, now);
        }
    }
    if presses.is_empty() || !leader_may_wait(app) {
        return Poll::Pass;
    }
    let focus = leader_focus(app, focus);
    let cx = context::read(app, focus);
    let chords = gate::chords_in(app, &cx);
    let leader =
        presses.iter().find_map(|(key, mods)| chords.iter().map(|(_, [first, _])| *first).find(|first| focus.allows(first) && key_matches(first, *key, *mods)));
    let Some(first) = leader else { return Poll::Pass };
    consume_keys(ctx);
    app.keybinds.pending = Some(Pending { first, since: now });
    ctx.request_repaint_after(std::time::Duration::try_from_secs_f64(TIMEOUT + 0.05).unwrap_or_default());
    Poll::Swallow
}

/// A chord may wait while nothing modal owns the keyboard: no dialog, prompt or warp.
fn leader_may_wait(app: &PhotocraftApp) -> bool {
    app.ui.dialogs.is_empty() && app.discard.is_none() && !app.distort.active() && app.distort.gallery.is_none() && !app.ui.palette_open
}

/// The frame after a leader: the next key finishes the chord, or cancels it.
fn follow(app: &mut PhotocraftApp, ctx: &egui::Context, focus: Focus, p: Pending, presses: &[(Key, Modifiers)], now: f64) -> Poll {
    if presses.is_empty() {
        ctx.request_repaint_after(std::time::Duration::try_from_secs_f64((TIMEOUT - (now - p.since)).max(0.0) + 0.05).unwrap_or_default());
        return Poll::Swallow;
    }
    let cx = context::read(app, leader_focus(app, focus));
    let next: Vec<(String, KeyboardShortcut)> =
        gate::chords_in(app, &cx).into_iter().filter(|(_, [first, _])| *first == p.first).map(|(id, [_, second])| (id, second)).collect();
    // the key as pressed first; failing that, with the modifiers of the leader still held taken
    // off, so ⌘K then C works with ⌘ kept down
    let hit = presses.iter().find_map(|(key, mods)| {
        next.iter()
            .find(|(_, second)| key_matches(second, *key, *mods))
            .or_else(|| next.iter().find(|(_, second)| key_matches(second, *key, without(*mods, p.first.modifiers))))
            .map(|(id, _)| id.clone())
    });
    let esc = presses.iter().any(|(key, mods)| *key == Key::Escape && mods.is_none());
    consume_keys(ctx);
    app.keybinds.pending = None;
    match hit {
        Some(id) => Poll::Fire(id),
        None => {
            if !esc {
                let template = "{key} was pressed, but no shortcut continues with that key";
                app.ui.status = crate::i18n::fmt(tl!(template), &[("key", &chord::step_text(&p.first))]);
                app.ui.status_error = true;
            }
            Poll::Swallow
        }
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

/// The keys of one frame as the shell hands them to [`poll`]: a chord used up, a command to run,
/// or nothing for the normal shortcuts. Returns true when the frame's keys are used up.
pub fn handle(app: &mut PhotocraftApp, ctx: &egui::Context, focus: Focus) -> bool {
    match poll(app, ctx, focus) {
        Poll::Pass => false,
        Poll::Swallow => true,
        Poll::Fire(id) => {
            crate::shortcut_dispatch::dispatch(app, ctx, &id);
            true
        }
    }
}
