//! A chord is two key steps written with a space between them (`Cmd+K C`); the engine keeps the
//! canonical text ([`photocraft_engine::prefs::normalize_chord`]), this turns it into key presses.

use egui::KeyboardShortcut;

use crate::shortcuts::{parse, pretty};

/// The two steps of `chord` as key presses, `None` when it isn't a chord or a step isn't a key
/// this build can press.
pub fn steps(chord: &str) -> Option<[KeyboardShortcut; 2]> {
    let [first, second] = photocraft_engine::prefs::chord_steps(chord)?;
    Some([parse(&first)?, parse(&second)?])
}

/// `Cmd+K C` as menus and tooltips show it: each step as [`pretty`] does (`⌘K C` on macOS).
pub fn text(chord: &str) -> String {
    chord.split_whitespace().map(pretty).collect::<Vec<_>>().join(" ")
}

/// One step as shown (`⌘K`).
pub fn step_text(step: &KeyboardShortcut) -> String {
    let mut s = String::new();
    if step.modifiers.command || step.modifiers.mac_cmd {
        s.push_str("Cmd+");
    }
    if step.modifiers.ctrl && !step.modifiers.command {
        s.push_str("Ctrl+");
    }
    if step.modifiers.alt {
        s.push_str("Alt+");
    }
    if step.modifiers.shift {
        s.push_str("Shift+");
    }
    let name = shortcut_text(step.logical_key, egui::Modifiers::NONE).unwrap_or_default();
    s.push_str(&name);
    pretty(&s)
}

/// Text of a key press for the shortcut editor (`Cmd+Shift+K`), `None` for bare modifiers.
pub fn shortcut_text(key: egui::Key, m: egui::Modifiers) -> Option<String> {
    if super::focus::is_modifier_key(key) {
        return None;
    }
    let mut parts = Vec::new();
    if m.command || m.mac_cmd {
        parts.push("Cmd".to_string());
    }
    if m.ctrl && !m.command {
        parts.push("Ctrl".into());
    }
    if m.alt {
        parts.push("Alt".into());
    }
    if m.shift {
        parts.push("Shift".into());
    }
    let name = match key {
        egui::Key::Equals => "=",
        egui::Key::Minus => "-",
        egui::Key::OpenBracket => "[",
        egui::Key::CloseBracket => "]",
        egui::Key::Semicolon => ";",
        egui::Key::Quote => "'",
        egui::Key::Plus => "",
        k => k.name(),
    };
    parts.push(name.to_string());
    photocraft_engine::prefs::normalize_shortcut(&parts.join("+"))
}
