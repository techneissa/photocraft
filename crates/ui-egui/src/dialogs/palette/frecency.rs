//! Fork (palette-frecency, #28): the command palette learns from what is run through it, as
//! launchers such as Alfred and Raycast do. Each command keeps how often and when it was last run
//! (its frecency); each typed text keeps the command picked for it. The history lives in
//! `Preferences::palette_history`, so it survives a restart.
//!
//! craft's `CommandPalette` ranks by how well the text matches first and uses `uses` only to order
//! matches that are equally good; so a clearly better text match still wins over past use. The
//! other two parts ride on two hooks of the techneissa/craft-libs fork (fork/craft-libs.md):
//!
//! - An empty search lists the commands run most often and most recently first ([`History::recents`]).
//! - A typed search: among matches as good as each other, the one run more often goes first
//!   ([`History::uses`]).
//! - A command picked for a typed text goes first when that text, or its start, is typed again
//!   ([`History::learned`], with the text from `PaletteOutput::query`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use craft::ui::runtime::commands::CommandRef;

/// Commands remembered; past this the one with the lowest frecency is forgotten.
const MAX_COMMANDS: usize = 300;
/// Typed texts remembered; past this the oldest pick is forgotten.
const MAX_PICKS: usize = 200;
/// Longest typed text kept for a pick, in characters.
const MAX_TEXT: usize = 64;
/// A use counts half as much after a week.
const HALF_LIFE_SECS: f64 = 7.0 * 24.0 * 3600.0;
/// One command's use: how often and when last (Unix seconds).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Use {
    count: u32,
    last: i64,
}

/// The command picked for a typed text, and when (Unix seconds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Pick {
    id: String,
    at: i64,
}

/// What the palette learned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct History {
    #[serde(default)]
    uses: BTreeMap<String, Use>,
    #[serde(default)]
    picks: BTreeMap<String, Pick>,
}

/// Now, in Unix seconds.
pub(super) fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The typed text as a pick's key: trimmed, lower case, at most [`MAX_TEXT`] characters.
fn key(text: &str) -> String {
    text.trim().to_lowercase().chars().take(MAX_TEXT).collect()
}

impl History {
    /// The history saved in the preferences; an unreadable or missing one is empty.
    pub(super) fn of(app: &crate::PhotocraftApp) -> Self {
        Self::from_value(&app.session.prefs().palette_history)
    }

    fn from_value(v: &Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    /// `id`'s frecency at `now`: its use count, halved for every week since its last use.
    fn frecency(&self, id: &str, now: i64) -> f64 {
        self.uses.get(id).map_or(0.0, |u| {
            let age = (now - u.last).max(0) as f64;
            f64::from(u.count) * 0.5f64.powf(age / HALF_LIFE_SECS)
        })
    }

    /// `id` was run from the palette at `now`, after typing `text`.
    pub(super) fn record(&mut self, id: &str, text: &str, now: i64) {
        let u = self.uses.entry(id.to_string()).or_insert(Use { count: 0, last: now });
        u.count = u.count.saturating_add(1);
        u.last = now;
        let k = key(text);
        if !k.is_empty() {
            self.picks.insert(k, Pick { id: id.to_string(), at: now });
        }
        while self.uses.len() > MAX_COMMANDS {
            let Some(least) = self.uses.keys().filter(|k| k.as_str() != id).min_by(|a, b| self.frecency(a, now).total_cmp(&self.frecency(b, now))).cloned()
            else {
                break;
            };
            self.uses.remove(&least);
        }
        while self.picks.len() > MAX_PICKS {
            let Some(oldest) = self.picks.iter().min_by_key(|(_, p)| p.at).map(|(k, _)| k.clone()) else { break };
            self.picks.remove(&oldest);
        }
    }

    /// The command picked for `text` or for a text it starts (the newest such pick).
    pub(super) fn learned(&self, text: &str) -> Option<&str> {
        let k = key(text);
        if k.is_empty() {
            return None;
        }
        self.picks.range(k.clone()..).take_while(|(t, _)| t.starts_with(&k)).max_by_key(|(_, p)| p.at).map(|(_, p)| p.id.as_str())
    }

    /// How often `id` was run lately, for craft's ranking of equally good matches (it grades `ilog2(n + 1)`, so
    /// one use and ten uses differ, ten and eleven barely): its frecency rounded. 0 for a command never run or
    /// not run for months.
    pub(super) fn uses(&self, id: &str, now: i64) -> u32 {
        self.frecency(id, now).round() as u32
    }

    /// The commands for an empty search: those run before, most often and most recently first, then
    /// `session`'s (this session's, newest first) that are not among them.
    pub(super) fn recents(&self, session: &[CommandRef], now: i64) -> Vec<CommandRef> {
        let mut used: Vec<(f64, &str)> = self.uses.keys().map(|id| (self.frecency(id, now), id.as_str())).collect();
        used.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
        let mut list: Vec<CommandRef> = used.into_iter().map(|(_, id)| CommandRef::new(id)).collect();
        for command in session {
            if !list.contains(command) {
                list.push(command.clone());
            }
        }
        list
    }
}

/// `id` was run from the palette after typing `text`: remember it in the preferences.
pub(super) fn note(app: &mut crate::PhotocraftApp, id: &str, text: &str) {
    let mut history = History::of(app);
    history.record(id, text, now());
    if let Ok(v) = serde_json::to_value(&history) {
        app.session.prefs.edit(|p| p.palette_history = v);
    }
}

#[cfg(test)]
mod tests;
