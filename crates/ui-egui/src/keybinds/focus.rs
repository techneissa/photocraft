//! Where the keyboard is, and which keys the focused thing keeps for itself.

use egui::{Key, KeyboardShortcut, Modifiers};

/// Where keyboard focus is: nowhere (the canvas), on a widget (a slider, a button reached with
/// Tab), or in a text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    None,
    Widget,
    /// A focused widget that edits with ⌫ / Delete itself.
    DeletingWidget,
    Text,
}

impl Focus {
    /// The focus as of the frame before (`ctx` has not read this frame's input yet).
    pub fn of(ctx: &egui::Context) -> Focus {
        if ctx.text_edit_focused() {
            Focus::Text
        } else if ctx.egui_wants_keyboard_input() {
            Focus::Widget
        } else {
            Focus::None
        }
    }

    /// May `sc` fire with this focus? A focused widget keeps its navigation keys (arrows, ↩,
    /// Space, Esc…); a text field keeps everything but ⌘ shortcuts (minus its own editing ones:
    /// select all, clipboard, undo) and the function keys. Ctrl+Tab switches documents whatever has focus.
    pub fn allows(self, sc: &KeyboardShortcut) -> bool {
        let k = sc.logical_key;
        if k == Key::Tab && sc.modifiers.ctrl {
            return true;
        }
        let function = matches!(
            k,
            Key::F1
                | Key::F2
                | Key::F3
                | Key::F4
                | Key::F5
                | Key::F6
                | Key::F7
                | Key::F8
                | Key::F9
                | Key::F10
                | Key::F11
                | Key::F12
                | Key::F13
                | Key::F14
                | Key::F15
        );
        let navigation = matches!(
            k,
            Key::ArrowLeft
                | Key::ArrowRight
                | Key::ArrowUp
                | Key::ArrowDown
                | Key::Enter
                | Key::Space
                | Key::Tab
                | Key::Escape
                | Key::Home
                | Key::End
                | Key::PageUp
                | Key::PageDown
        );
        match self {
            Focus::None => true,
            Focus::Widget => sc.modifiers.command || !navigation,
            Focus::DeletingWidget => sc.modifiers.command || !(navigation || matches!(k, Key::Backspace | Key::Delete)),
            Focus::Text => {
                let text_edit = !sc.modifiers.alt && matches!(k, Key::A | Key::C | Key::X | Key::V | Key::Z | Key::Y)
                    || navigation
                    || matches!(k, Key::Backspace | Key::Delete);
                function || (sc.modifiers.command && !text_edit)
            }
        }
    }
}

/// With ⇧ held the OS reports the shifted character as the logical key (US layout): ⇧; is `:`.
fn shifted(k: Key) -> Option<Key> {
    Some(match k {
        Key::Equals => Key::Plus,
        Key::Semicolon => Key::Colon,
        Key::OpenBracket => Key::OpenCurlyBracket,
        Key::CloseBracket => Key::CloseCurlyBracket,
        Key::Slash => Key::Questionmark,
        Key::Backslash => Key::Pipe,
        Key::Num1 => Key::Exclamationmark,
        _ => return None,
    })
}

/// Does a key press match a shortcut, the way Photoshop matches them?
///
/// - Modifiers match exactly (⌘⌥I is not ⌘I), with ⌘ = Ctrl off the Mac.
/// - `=` also matches `+`; Delete also matches Backspace, and the other way round.
/// - A ⇧ shortcut on a punctuation key matches the shifted character (⌘⇧; arrives as ⌘`:`).
pub fn key_matches(sc: &KeyboardShortcut, key: Key, mods: Modifiers) -> bool {
    let k = sc.logical_key;
    let direct = key == k || (k == Key::Delete && key == Key::Backspace) || (k == Key::Backspace && key == Key::Delete);
    let plus = k == Key::Equals && key == Key::Plus;
    let via_shift = sc.modifiers.shift && shifted(k) == Some(key);
    if !(direct || plus || via_shift) {
        return false;
    }
    // Typing `+` (or another shifted symbol) may need ⇧ on this layout: don't require it absent.
    let shifted_symbol =
        matches!(key, Key::Plus | Key::Colon | Key::OpenCurlyBracket | Key::CloseCurlyBracket | Key::Questionmark | Key::Pipe | Key::Exclamationmark);
    let shift_ok = mods.shift == sc.modifiers.shift || (shifted_symbol && !sc.modifiers.shift);
    mods.alt == sc.modifiers.alt && shift_ok && mods.cmd_ctrl_matches(sc.modifiers)
}

/// A modifier key on its own: egui reports Ctrl, Shift, Alt and ⌘ presses as key events too.
pub fn is_modifier_key(key: Key) -> bool {
    use Key::*;
    matches!(key, ShiftLeft | ShiftRight | ControlLeft | ControlRight | AltLeft | AltRight | SuperLeft | SuperRight)
}
