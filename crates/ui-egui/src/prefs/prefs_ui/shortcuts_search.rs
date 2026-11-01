//! The search box of Keyboard Shortcuts and Menus understands modifier words: `cmd`, `shift`,
//! `ctrl` and `alt` (also `command`, `control`, `option`, `opt`) match the shortcuts that use ⌘,
//! ⇧, ⌃ and ⌥, words combine (`cmd shift` is a shortcut with both), and the symbols work too
//! (`⌘⇧`, `⌘n`, `cmd+shift+k`). Anything else still matches command names and menus.

const CMD: u8 = 1;
const CTRL: u8 = 2;
const ALT: u8 = 4;
const SHIFT: u8 = 8;

fn word_mask(word: &str) -> u8 {
    match word {
        "command" | "cmd" => CMD,
        "control" | "ctrl" => CTRL,
        "option" | "opt" | "alt" => ALT,
        "shift" => SHIFT,
        _ => 0,
    }
}

fn symbol_mask(c: char) -> u8 {
    match c {
        '⌘' => CMD,
        '⌃' => CTRL,
        '⌥' => ALT,
        '⇧' => SHIFT,
        _ => 0,
    }
}

/// A search, read once per frame.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Query {
    /// The whole search, for a search without modifiers.
    whole: String,
    /// The modifiers a shortcut must use (a set of the four bits above).
    mods: u8,
    /// The key it must end in (`⌘n`, `cmd+shift+k`).
    key: Option<String>,
    /// What is left of the search once the modifiers are taken out: matches names and menus.
    text: String,
}

/// Read `needle` (already lower case).
pub(super) fn parse(needle: &str) -> Query {
    let mut query = Query { whole: needle.to_string(), mods: 0, key: None, text: String::new() };
    let mut rest: Vec<&str> = Vec::new();
    for token in needle.split_whitespace() {
        let word = word_mask(token);
        let lead = token.chars().take_while(|c| symbol_mask(*c) != 0).count();
        if word != 0 {
            query.mods |= word;
        } else if lead > 0 {
            query.mods |= token.chars().take(lead).fold(0, |m, c| m | symbol_mask(c));
            let tail: String = token.chars().skip(lead).collect();
            if !tail.is_empty() {
                query.key = Some(tail);
            }
        } else if let Some((mods, key)) = plus_form(token) {
            query.mods |= mods;
            query.key = key.or(query.key.take());
        } else {
            rest.push(token);
        }
    }
    query.text = rest.join(" ");
    query
}

/// `cmd+shift+k` or `ctrl+alt`: modifier words joined by `+`, then a key or one more modifier.
fn plus_form(token: &str) -> Option<(u8, Option<String>)> {
    let parts: Vec<&str> = token.split('+').filter(|p| !p.is_empty()).collect();
    let (last, lead) = parts.split_last()?;
    if lead.is_empty() || lead.iter().any(|p| word_mask(p) == 0) {
        return None;
    }
    let mods = lead.iter().fold(0, |m, p| m | word_mask(p));
    match word_mask(last) {
        0 => Some((mods, Some((*last).to_string()))),
        m => Some((mods | m, None)),
    }
}

/// The modifiers and the key of a shortcut in the canonical spelling (`Cmd+Shift+K`).
fn split(binding: &str) -> (u8, &str) {
    let (head, key) = match binding.strip_suffix("++") {
        Some(head) => (head, "+"),
        None => binding.rsplit_once('+').unwrap_or(("", binding)),
    };
    (head.split('+').fold(0, |m, p| m | word_mask(&p.to_ascii_lowercase())), key)
}

impl Query {
    /// Does a command match? `bindings` are its keys and the steps of its chord, canonical;
    /// `text_hit` says whether a piece of text is in its name, menu or keys.
    pub(super) fn matches(&self, bindings: &[String], text_hit: &dyn Fn(&str) -> bool) -> bool {
        if self.whole.is_empty() {
            return true;
        }
        if self.mods == 0 && self.key.is_none() {
            return text_hit(&self.whole);
        }
        (self.text.is_empty() || text_hit(&self.text))
            && bindings.iter().any(|b| {
                let (mods, key) = split(b);
                mods & self.mods == self.mods && self.key.as_deref().is_none_or(|k| key.eq_ignore_ascii_case(k))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(needle: &str, bindings: &[&str], name: &str) -> bool {
        let bindings: Vec<String> = bindings.iter().map(|b| b.to_string()).collect();
        let name = name.to_lowercase();
        parse(&needle.to_lowercase()).matches(&bindings, &|t| name.contains(t))
    }

    #[test]
    fn modifier_words_match_the_shortcuts_that_use_them() {
        for (word, yes, no) in [
            ("command", "Cmd+N", "Alt+N"),
            ("cmd", "Cmd+Shift+N", "Shift+F5"),
            ("shift", "Shift+F5", "Cmd+N"),
            ("control", "Ctrl+Alt+K", "Cmd+K"),
            ("ctrl", "Ctrl+K", "Alt+K"),
            ("option", "Cmd+Alt+K", "Cmd+K"),
            ("opt", "Alt+Backspace", "Backspace"),
            ("alt", "Alt+K", "Shift+K"),
        ] {
            assert!(hit(word, &[yes], "Some Command"), "{word} finds {yes}");
            assert!(!hit(word, &[no], "Some Command"), "{word} leaves {no}");
        }
    }

    #[test]
    fn words_combine_on_one_shortcut() {
        assert!(hit("cmd shift", &["Cmd+Shift+N"], "x"));
        assert!(hit("shift cmd", &["Cmd+Alt+Shift+N"], "x"));
        assert!(!hit("cmd shift", &["Cmd+N"], "x"));
        assert!(!hit("cmd shift", &["Cmd+N", "Shift+F5"], "x"), "both on the same key, not spread over two");
        assert!(hit("cmd shift", &["Cmd+N", "Cmd+Shift+F5"], "x"), "any one of a command's keys will do");
        assert!(hit("cmd shift", &["Cmd+K", "Cmd+Shift+S"], "x"), "a chord's steps count one by one");
    }

    #[test]
    fn the_symbols_work_too() {
        assert!(hit("⌘", &["Cmd+N"], "x"));
        assert!(hit("⌘⇧", &["Cmd+Shift+N"], "x"));
        assert!(!hit("⌘⇧", &["Cmd+N"], "x"));
        assert!(hit("⌃⌥", &["Cmd+Ctrl+Alt+N"], "x"));
        assert!(hit("⌘n", &["Cmd+N"], "x"));
        assert!(!hit("⌘n", &["Cmd+M"], "x"));
        assert!(hit("cmd+shift+k", &["Cmd+Shift+K"], "x"));
        assert!(!hit("cmd+shift+k", &["Cmd+Shift+J"], "x"));
        assert!(hit("cmd+alt", &["Cmd+Alt+J"], "x"));
    }

    #[test]
    fn plain_words_still_match_names() {
        assert!(hit("new", &[], "New Layer"));
        assert!(hit("new layer", &[], "New Layer"), "a phrase is one piece of text");
        assert!(!hit("zzz", &["Cmd+N"], "New Layer"));
        assert!(hit("", &[], "New Layer"), "an empty search shows everything");
    }

    #[test]
    fn a_modifier_and_a_name_must_both_match() {
        assert!(hit("cmd layer", &["Cmd+N"], "New Layer"));
        assert!(!hit("cmd layer", &["Cmd+N"], "New Document"));
        assert!(!hit("cmd layer", &["Alt+N"], "New Layer"));
    }

    #[test]
    fn a_plus_key_is_read() {
        assert!(hit("cmd+=", &["Cmd+="], "x"));
        assert_eq!(split("Cmd++"), (CMD, "+"));
        assert_eq!(split("F5"), (0, "F5"));
    }
}
