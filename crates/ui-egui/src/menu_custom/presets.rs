//! Three presets of the whole set of arranged menus: Preferences › Context Menus keeps up to three
//! sets in slots (Preset 1, 2 and 3), saves the menus as they are now into one, and switches
//! between them at once.
//!
//! A slot holds every menu's arrangement. The slot last saved or switched to is `active`, and the
//! set a switch replaced is `undo`, so a switch can be taken back; the dropdown marks `active` as
//! changed when the menus no longer equal what that slot holds.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::menus;
use super::{Arrangement, Custom};

/// How many presets there are.
pub const COUNT: usize = 3;

/// Every menu's arrangement, by menu key.
pub type Set = BTreeMap<String, Arrangement>;

/// The set a switch replaced, and the slot that was active then. `after` is the set the switch
/// made: once the menus have changed since, the switch is no longer the last thing done and there
/// is nothing to take back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Undo {
    pub menus: Set,
    pub active: Option<usize>,
    pub after: Set,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Presets {
    /// The set in each slot (None = empty); [`COUNT`] long once anything is saved.
    pub slots: Vec<Option<Set>>,
    /// The slot last saved or switched to (counting from 0).
    pub active: Option<usize>,
    pub undo: Option<Undo>,
}

/// What one slot shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotState {
    pub saved: bool,
    pub active: bool,
    /// Active, but the menus have changed since (Save to Preset N keeps the change).
    pub edited: bool,
}

pub fn menus_from_value(v: &Value) -> Set {
    let Some(obj) = v.as_object() else { return Set::new() };
    obj.iter()
        .filter(|(key, _)| menus::is_key(key))
        .map(|(key, a)| (key.clone(), Arrangement::from_value(a)))
        .filter(|(_, a)| !a.items.is_empty() || !a.hidden.is_empty())
        .collect()
}

pub fn menus_to_value(set: &Set) -> Value {
    Value::Object(set.iter().map(|(key, a)| (key.clone(), a.to_value())).collect())
}

fn slot_index(v: Option<&Value>) -> Option<usize> {
    v.and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).filter(|n| *n < COUNT)
}

impl Presets {
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none) && self.active.is_none() && self.undo.is_none()
    }

    /// The set saved in `slot` (counting from 0).
    pub fn get(&self, slot: usize) -> Option<&Set> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    fn put(&mut self, slot: usize, set: Option<Set>) {
        if self.slots.len() < COUNT {
            self.slots.resize(COUNT, None);
        }
        if let Some(s) = self.slots.get_mut(slot) {
            *s = set;
        }
    }

    /// Read leniently: a value that is not what was saved loads as far as it can be read.
    pub fn from_value(v: &Value) -> Presets {
        let mut out = Presets::default();
        let Some(obj) = v.as_object() else { return out };
        out.slots =
            (0..COUNT).map(|i| obj.get("slots").and_then(Value::as_array).and_then(|a| a.get(i)).filter(|s| s.is_object()).map(menus_from_value)).collect();
        out.active = slot_index(obj.get("active")).filter(|i| out.get(*i).is_some());
        out.undo = obj.get("undo").and_then(Value::as_object).map(|u| Undo {
            menus: u.get("menus").map(menus_from_value).unwrap_or_default(),
            active: slot_index(u.get("active")).filter(|i| out.get(*i).is_some()),
            after: u.get("after").map(menus_from_value).unwrap_or_default(),
        });
        if out.slots.iter().all(Option::is_none) {
            out.slots.clear();
            out.active = None;
        }
        out
    }

    pub fn to_value(&self) -> Value {
        let mut out = json!({"slots": self.slots.iter().map(|s| s.as_ref().map_or(Value::Null, menus_to_value)).collect::<Vec<_>>()});
        if let Some(a) = self.active {
            out["active"] = json!(a);
        }
        if let Some(u) = &self.undo {
            out["undo"] = json!({"menus": menus_to_value(&u.menus), "after": menus_to_value(&u.after), "active": u.active});
        }
        out
    }
}

fn bad_slot() -> String {
    crate::i18n::fmt(tl!("`slot` must be a number from 1 to {n}"), &[("n", &COUNT.to_string())])
}

impl Custom {
    pub fn states(&self) -> [SlotState; COUNT] {
        std::array::from_fn(|i| {
            let saved = self.presets.get(i);
            let active = self.presets.active == Some(i) && saved.is_some();
            SlotState { saved: saved.is_some(), active, edited: active && saved != Some(&self.menus) }
        })
    }

    /// Whether the last switch can be taken back.
    pub fn can_undo(&self) -> bool {
        self.presets.undo.as_ref().is_some_and(|u| u.after == self.menus)
    }

    /// Save the menus as they are now into `slot` (counting from 0) and make it the active one.
    pub fn save_to(&mut self, slot: usize) -> Result<(), String> {
        if slot >= COUNT {
            return Err(bad_slot());
        }
        let now = self.menus.clone();
        self.presets.put(slot, Some(now));
        self.presets.active = Some(slot);
        Ok(())
    }

    /// Empty `slot`; the menus stay as they are. An empty slot is an error.
    pub fn clear(&mut self, slot: usize) -> Result<(), String> {
        if slot >= COUNT {
            return Err(bad_slot());
        }
        if self.presets.get(slot).is_none() {
            return Err(crate::i18n::fmt(tl!("Preset {n} is already empty"), &[("n", &(slot + 1).to_string())]));
        }
        self.presets.put(slot, None);
        if self.presets.active == Some(slot) {
            self.presets.active = None;
        }
        if let Some(u) = self.presets.undo.as_mut().filter(|u| u.active == Some(slot)) {
            u.active = None;
        }
        if self.presets.slots.iter().all(Option::is_none) {
            self.presets.slots.clear();
        }
        Ok(())
    }

    /// Switch the menus to what `slot` holds. The set it replaces can be brought back with [`Custom::undo`].
    pub fn switch_to(&mut self, slot: usize) -> Result<(), String> {
        if slot >= COUNT {
            return Err(bad_slot());
        }
        let Some(set) = self.presets.get(slot).cloned() else {
            return Err(crate::i18n::fmt(tl!("Preset {n} is empty: arrange the menus, then Save to Preset {n}"), &[("n", &(slot + 1).to_string())]));
        };
        let before = Undo { menus: self.menus.clone(), active: self.presets.active, after: set.clone() };
        let changes = before.menus != set;
        self.menus = set;
        self.presets.active = Some(slot);
        // switching to what is already there changes nothing, so there is nothing to take back
        if changes {
            self.presets.undo = Some(before);
        }
        Ok(())
    }

    /// Bring back the set the last switch replaced.
    pub fn undo(&mut self) -> Result<(), String> {
        if !self.can_undo() {
            return Err(tl!("Nothing to undo: the menus have not been switched to a preset, or have changed since").to_string());
        }
        let Some(undo) = self.presets.undo.take() else { return Ok(()) };
        self.menus = undo.menus;
        self.presets.active = undo.active;
        Ok(())
    }
}
