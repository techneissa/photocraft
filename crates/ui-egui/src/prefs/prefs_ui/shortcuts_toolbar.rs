//! The Toolbar tab of Keyboard Shortcuts and Menus: a check per tool (a tool left unchecked leaves the toolbar for
//! Extra Tools) with its key, in as many columns as the width holds, and Restore Defaults. The tab edits the list of
//! hidden tools; OK writes it through `edit.toolbar` (`shortcuts::pass`), as the fork's old window did.

use craft::menu::shortcuts::DialogTab;
use craft::ui::choice::Checkbox;
use craft::ui::contract::{Bind, Props as _, Show as _};
use craft::ui::runtime::env::with_env;

use crate::state::Tool;

/// The width one tool column (checkbox, name, key) takes.
const COLUMN_WIDTH: f32 = 240.0;

/// How many tool columns fit `width`: one to four.
fn columns(width: f32) -> usize {
    ((width / COLUMN_WIDTH) as usize).clamp(1, 4)
}

/// The tab, over the draft's hidden tools (`Tool` names).
pub(super) fn tab(hidden: &mut Vec<String>) -> DialogTab<'_> {
    DialogTab::new("toolbar", tl!("Toolbar").to_string(), move |ui| body(ui, hidden))
}

/// The tab's body; true when the draft changed.
fn body(ui: &mut egui::Ui, hidden: &mut Vec<String>) -> bool {
    let before = hidden.clone();
    if ui.button(tl!("Restore Defaults")).clicked() {
        hidden.clear();
    }
    let columns = columns(ui.available_width());
    let rows = Tool::ALL.len().div_ceil(columns);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("toolbar-grid").num_columns(columns * 2).spacing([16.0, 4.0]).show(ui, |ui| {
            for row in 0..rows {
                for column in 0..columns {
                    let Some(tool) = Tool::ALL.get(column * rows + row) else {
                        ui.label("");
                        ui.label("");
                        continue;
                    };
                    let name = format!("{tool:?}");
                    let on = !hidden.contains(&name);
                    let shown = with_env(
                        ui,
                        |set| set.id_salt = Some(egui::Id::unique(("toolbar-tool", &name))),
                        |ui| Checkbox::new(Bind::given(on)).label(tl!(tool.label()).to_string()).show(ui),
                    );
                    match shown.events.value() {
                        Some(&true) => hidden.retain(|h| *h != name),
                        Some(&false) if !hidden.contains(&name) => hidden.push(name),
                        _ => {}
                    }
                    let key = tool.key();
                    ui.label(if key == '\0' { String::new() } else { key.to_string() });
                }
                ui.end_row();
            }
        });
    });
    *hidden != before
}
