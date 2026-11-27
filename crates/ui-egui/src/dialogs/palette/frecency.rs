//! Fork (palette-frecency, #28): the command palette learns from what is run through it, as
//! launchers such as Alfred and Raycast do. Each command keeps how often and when it was last run
//! (its frecency); each typed text keeps the command picked for it. The history lives in
//! `Preferences::palette_history`, so it survives a restart.
//!
//! - An empty search lists the commands run most often and most recently first.
//! - A typed search: text match first. Past use adds less than [`CLEARLY_BETTER`] scaled points,
//!   so among matches that are about as good, the one run more often goes first.
//! - A command picked for a typed text goes first when that text, or its start, is typed again.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::nav::Hit;

/// Commands remembered; past this the one with the lowest frecency is forgotten.
const MAX_COMMANDS: usize = 300;
/// Typed texts remembered; past this the oldest pick is forgotten.
const MAX_PICKS: usize = 200;
/// Longest typed text kept for a pick, in characters.
const MAX_TEXT: usize = 64;
/// A use counts half as much after a week.
const HALF_LIFE_SECS: f64 = 7.0 * 24.0 * 3600.0;
/// Text scores are multiplied by this before past use is added, so use can order matches that
/// score the same without changing the order of matches that don't.
const SCALE: i32 = 100;
/// A text match this many points better (about one more letter in a row, which scores 8) always
/// wins over past use: the use bonus stays below `CLEARLY_BETTER * SCALE`.
pub(super) const CLEARLY_BETTER: i32 = 7;
/// What a pick for the typed text adds: above every text score (an exact name scores 1000).
const PICK: i32 = 1_000_000 * SCALE;
/// The empty search's used commands, above every unused one.
const USED: i32 = 1_000_000;

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
    fn pick(&self, text: &str) -> Option<&str> {
        let k = key(text);
        if k.is_empty() {
            return None;
        }
        self.picks.range(k.clone()..).take_while(|(t, _)| t.starts_with(&k)).max_by_key(|(_, p)| p.at).map(|(_, p)| p.id.as_str())
    }

    /// What past use adds to a typed match of `id`: 0 for a command never run, growing with its
    /// frecency and never reaching [`CLEARLY_BETTER`] scaled points.
    fn use_bonus(&self, id: &str, now: i64) -> i32 {
        let f = self.frecency(id, now);
        let most = f64::from(CLEARLY_BETTER * SCALE - 1);
        (most * f / (f + 1.0)) as i32
    }

    /// Order `hits` for the search `text` at `now`; the caller then sorts them ([`super::nav::rank`]).
    pub(super) fn apply(&self, text: &str, now: i64, hits: &mut [Hit]) {
        if text.trim().is_empty() {
            let mut used: Vec<(f64, &str)> = self.uses.keys().map(|id| (self.frecency(id, now), id.as_str())).collect();
            used.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
            for h in hits.iter_mut() {
                if let Some(i) = used.iter().position(|(_, id)| *id == h.1) {
                    h.0 = USED - i as i32;
                }
            }
            return;
        }
        let picked = self.pick(text);
        for h in hits.iter_mut() {
            h.0 = h.0.saturating_mul(SCALE).saturating_add(self.use_bonus(&h.1, now));
            if picked == Some(h.1.as_str()) {
                h.0 = h.0.saturating_add(PICK);
            }
        }
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
