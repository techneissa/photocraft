//! The commands that can be added to a menu: every live item of the menu bar, with its label and
//! the path to it (`Layer › Layer Style › Blending Options…`).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// One command of the menu bar.
#[derive(Debug)]
pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    /// Where the menu bar has it, as `Layer › Layer Style`.
    pub path: String,
    /// Label and path in lower case, for the editor's search.
    pub hay: String,
}

struct Catalog {
    all: Vec<Cmd>,
    by_id: HashMap<&'static str, usize>,
}

fn build() -> Catalog {
    let mut all: Vec<Cmd> = Vec::new();
    let mut seen: HashSet<&'static str> = HashSet::new();
    let mut add = |id: &'static str, label: &'static str, path: String| {
        if id == "---" || label == "---" || label.trim().is_empty() || !crate::menus::is_live(id) || !seen.insert(id) {
            return;
        }
        let hay = format!("{label} {path}").to_lowercase();
        all.push(Cmd { id, label, path, hay });
    };
    for &(path, label, _, id) in crate::menu_catalog::CATALOG {
        add(id, label, path.join(" › "));
    }
    for &(id, label, path, _) in crate::menus::UI_COMMANDS {
        add(id, label, path.join(" › "));
    }
    for c in photocraft_engine::command_specs().iter().filter(|c| !c.menu.is_empty()) {
        add(c.id, c.label, c.menu.join(" › "));
    }
    let by_id = all.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
    Catalog { all, by_id }
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(build)
}

/// Every command that can be added, in the menu bar's order.
pub fn all() -> &'static [Cmd] {
    &catalog().all
}

/// The command `id`, when the menu bar has it.
pub fn find(id: &str) -> Option<&'static Cmd> {
    let c = catalog();
    c.by_id.get(id).and_then(|i| c.all.get(*i))
}
