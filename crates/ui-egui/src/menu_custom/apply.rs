//! An arrangement laid over the rows a menu builds now: the pure part of [`super`].

use std::collections::{HashMap, HashSet};

use super::{MAX_ITEMS, catalog};
use crate::canvas_tool_menu::Row;
use serde_json::Value;

/// A separator's slot.
pub const SEPARATOR: &str = "---";
/// The prefix of a command's slot.
pub const CMD: &str = "cmd:";

/// One entry of an arranged menu.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Cmd(String),
    Separator,
}

impl Slot {
    /// The slot as it is saved.
    pub fn key(&self) -> String {
        match self {
            Slot::Cmd(id) => format!("{CMD}{id}"),
            Slot::Separator => SEPARATOR.to_string(),
        }
    }

    /// A saved slot, or None for one that is malformed.
    pub fn parse(s: &str) -> Option<Slot> {
        let s = s.trim();
        if s == SEPARATOR {
            return Some(Slot::Separator);
        }
        let id = s.strip_prefix(CMD)?.trim();
        (!id.is_empty() && id.len() <= 200).then(|| Slot::Cmd(id.to_string()))
    }
}

/// One menu's arrangement: the slots shown in order, and the built-in rows taken out (what the
/// menu builds and the list does not hold: a row an update adds later is in neither, so it shows).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arrangement {
    pub items: Vec<String>,
    pub hidden: Vec<String>,
}

impl Arrangement {
    /// Read leniently: text entries only, at most [`MAX_ITEMS`] of each list.
    pub fn from_value(v: &Value) -> Arrangement {
        let list = |name: &str| -> Vec<String> {
            v.get(name).and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).take(MAX_ITEMS).collect()).unwrap_or_default()
        };
        Arrangement { items: list("items"), hidden: list("hidden") }
    }

    pub fn to_value(&self) -> Value {
        serde_json::json!({"items": self.items, "hidden": self.hidden})
    }

    /// The slots, with those that are malformed left out.
    pub fn slots(&self) -> Vec<Slot> {
        self.items.iter().filter_map(|s| Slot::parse(s)).collect()
    }
}

/// Drop separators at the start and end and those following another.
pub fn tidy(rows: Vec<Row>) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::with_capacity(rows.len());
    for row in rows {
        if row.is_none() && out.last().is_none_or(Option::is_none) {
            continue;
        }
        out.push(row);
    }
    while out.last().is_some_and(Option::is_none) {
        out.pop();
    }
    out
}

/// `arrangement` over `default`, the rows the menu builds now (labels and ids as it builds them).
///
/// - A saved command the menu builds now is its row, where the arrangement puts it.
/// - One of the menu's own rows that is not built now (a mask row for a layer without a mask) is
///   skipped, and shows again when it is built.
/// - Any other command is added when `can_add` and the menu bar has it; the rest are skipped.
/// - A row the menu builds that the arrangement neither lists nor hides is new: it goes after the
///   row built before it.
/// - Separators that would start or end the menu, or follow one another, go.
pub fn apply(arrangement: &Arrangement, default: &[Row], universe: &[&str], can_add: bool) -> Vec<Row> {
    let mut built: HashMap<&str, (&'static str, &'static str)> = HashMap::new();
    for row in default.iter().flatten() {
        built.entry(row.1).or_insert(*row);
    }
    let known: HashSet<&str> = universe.iter().copied().collect();
    let hidden: HashSet<&str> = arrangement.hidden.iter().map(String::as_str).collect();
    let mut placed: HashSet<String> = HashSet::new();
    let mut out: Vec<Row> = Vec::new();
    for slot in arrangement.slots() {
        let Slot::Cmd(id) = slot else {
            out.push(None);
            continue;
        };
        // each command once
        if !placed.insert(id.clone()) {
            continue;
        }
        if let Some(row) = built.get(id.as_str()) {
            out.push(Some(*row));
        } else if !known.contains(id.as_str())
            && can_add
            && let Some(cmd) = catalog::find(&id)
        {
            out.push(Some((cmd.label, cmd.id)));
        }
    }
    let mut anchor: Option<usize> = None;
    for row in default.iter().flatten() {
        if placed.contains(row.1) {
            anchor = out.iter().position(|r| r.is_some_and(|r| r.1 == row.1)).or(anchor);
            continue;
        }
        if hidden.contains(row.1) {
            continue;
        }
        let at = anchor.map_or(out.len(), |a| a + 1);
        out.insert(at, Some(*row));
        anchor = Some(at);
        placed.insert(row.1.to_string());
    }
    tidy(out)
}
