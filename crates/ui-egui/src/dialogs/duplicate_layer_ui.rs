//! Layer › Duplicate Layer… dialog (#3348): the copy's name and its destination document, as in
//! Photoshop. Only the menu, palette and Layers panel menu open it; ⌘J, Alt-drag and dragging onto
//! the New Layer button still duplicate at once.
use crate::{PhotocraftApp, state::DialogKind};
use serde_json::{Map, Value, json};
const MARK: &str = "__duplicateLayer";

pub fn owns(fields: &Map<String, Value>) -> bool {
    fields.contains_key(MARK)
}

pub fn open(app: &mut PhotocraftApp) -> Result<Value, String> {
    let index = app.session.active_index().ok_or("no document")?;
    let st = app.session.active().ok_or("no document")?;
    let selected = st.selected_layers();
    let layer = st.active_layer.and_then(|id| st.doc.layer(id)).ok_or("no active layer")?;
    let mut fields = Map::new();
    fields.insert(MARK.into(), json!(true));
    fields.insert("__command".into(), json!("layer.duplicate"));
    fields.insert("destination".into(), json!(index));
    // Photoshop's document-`Name` starts empty; the engine defaults it when empty.
    fields.insert("documentName".into(), json!(""));
    fields.insert("document".into(), json!(st.doc.id.0));
    fields.insert("selected".into(), json!(selected.iter().map(|id| id.0).collect::<Vec<_>>()));
    // Several selected layers keep their names, as in Photoshop's Duplicate Layers.
    if selected.len() > 1 {
        fields.insert("__label".into(), json!("Duplicate Layers…"));
    } else if layer.is_group() {
        fields.insert("__label".into(), json!("Duplicate Group…"));
        fields.insert("name".into(), json!(st.doc.copy_name(&layer.name)));
    } else {
        fields.insert("__label".into(), json!("Duplicate Layer…"));
        fields.insert("name".into(), json!(st.doc.copy_name(&layer.name)));
    }
    let id = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({"dialog": id}))
}

pub fn body(app: &PhotocraftApp, ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let documents: Vec<(String, String)> = app.session.documents().iter().enumerate().map(|(i, d)| (i.to_string(), d.doc.name.clone())).collect();
    let mut options: Vec<(String, &str)> = documents.iter().map(|(k, n)| (k.clone(), n.as_str())).collect();
    options.push(("new".to_string(), tl!("New")));
    let mut destination = match fields.get("destination") {
        Some(Value::String(s)) if s == "new" => "new".to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    egui::Grid::new("duplicate-layer").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        if let Some(name) = fields.get("name").and_then(Value::as_str) {
            let mut name = name.to_string();
            let name_label = ui.label(tl!("Name:"));
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(260.0)).labelled_by(name_label.id);
            fields.insert("name".into(), json!(name));
            ui.end_row();
        }
        ui.label(egui::RichText::new(tl!("Destination")).font(crate::theme::semibold(13.0)));
        ui.end_row();
        ui.label(tl!("Document"));
        crate::widgets::dropdown(ui, "duplicate-layer-document", tl!("Document"), &mut destination, &options, 260.0);
        ui.end_row();
        // Photoshop's document-`Name` is only there when the destination is a new document.
        if destination == "new" {
            let mut doc_name = fields.get("documentName").and_then(Value::as_str).unwrap_or("").to_string();
            let name_label = ui.label(tl!("Name:"));
            ui.add(egui::TextEdit::singleline(&mut doc_name).desired_width(260.0)).labelled_by(name_label.id);
            fields.insert("documentName".into(), json!(doc_name));
            ui.end_row();
        }
    });
    if destination == "new" {
        fields.insert("destination".into(), json!("new"));
    } else if let Ok(i) = destination.parse::<u64>() {
        fields.insert("destination".into(), json!(i));
    }
}

pub fn confirm(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let index = app.session.active_index().ok_or("The document is closed")?;
    let st = app.session.active().ok_or("The document is closed")?;
    if fields.get("document").and_then(Value::as_u64) != Some(st.doc.id.0)
        || fields.get("selected") != Some(&json!(st.selected_layers().iter().map(|id| id.0).collect::<Vec<_>>()))
    {
        return Err("The document or selected layers changed. Choose Duplicate Layer again.".into());
    }
    let mut params = Map::new();
    if let Some(name) = fields.get("name").and_then(Value::as_str) {
        params.insert("name".into(), json!(name));
    }
    let (result, destination) = match fields.get("destination") {
        Some(Value::String(s)) if s == "new" => {
            if let Some(doc_name) = fields.get("documentName").and_then(Value::as_str).filter(|n| !n.is_empty()) {
                params.insert("documentName".into(), json!(doc_name));
            }
            params.insert("document".into(), json!("new"));
            (app.run_command("layer.duplicate", Value::Object(params))?, None)
        }
        Some(Value::Number(n)) => {
            let destination =
                n.as_u64().and_then(|i| usize::try_from(i).ok()).filter(|i| *i < app.session.documents().len()).ok_or("no destination document")?;
            if destination == index {
                (app.run_command("layer.duplicate", Value::Object(params))?, Some(destination))
            } else {
                params.insert("document".into(), json!(destination));
                (app.run_command("layer.copyToDocument", Value::Object(params))?, Some(destination))
            }
        }
        _ => return Err("no destination document".into()),
    };
    // Say where the copy went, also when it is the document it came from: a copy that landed in the
    // current document instead of the other one was easy to miss.
    let name = match destination {
        Some(i) => app.session.documents().get(i).map(|d| d.doc.name.clone()),
        None => app.session.active().map(|st| st.doc.name.clone()),
    };
    if let Some(name) = name {
        app.ui.status = crate::i18n::fmt(tl!("Duplicated to {document}"), &[("document", &name)]);
        app.ui.status_error = false;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_app() -> PhotocraftApp {
        let mut s = photocraft_engine::Session::new();
        s.execute("file.new", json!({"width":8,"height":8,"name":"Poster"})).unwrap();
        s.execute("layer.new.layer", json!({"name":"Ink"})).unwrap();
        PhotocraftApp::new(s, crate::Services::default())
    }
    fn invoke(app: &mut PhotocraftApp) -> u64 {
        let ctx = egui::Context::default();
        crate::menus::invoke(app, &ctx, "layer.duplicate", json!({})).unwrap()["dialog"].as_u64().unwrap()
    }
    fn names(app: &PhotocraftApp, i: usize) -> Vec<String> {
        app.session.documents()[i].doc.walk().into_iter().map(|(_, _, l)| l.name.clone()).collect()
    }

    #[test]
    fn the_menu_opens_the_dialog_and_ok_duplicates_under_the_typed_name() {
        let mut app = make_app();
        let dialog = invoke(&mut app);
        assert_eq!(app.session.active().unwrap().doc.layer_count(), 2, "nothing is copied until OK");
        assert_eq!(app.ui.dialogs[0].fields["name"], "Ink copy");
        app.ui.dialog_mut(dialog).unwrap().fields.insert("name".into(), json!("Shadow"));
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert!(names(&app, 0).contains(&"Shadow".to_string()));
        assert_eq!(app.session.active().unwrap().doc.layer_count(), 3);
    }

    #[test]
    fn another_destination_document_receives_the_copy() {
        let mut app = make_app();
        app.session.execute("file.new", json!({"width":8,"height":8,"name":"Card"})).unwrap();
        app.session.set_active(0);
        let dialog = invoke(&mut app);
        app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!(1));
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert_eq!(app.session.documents()[0].doc.layer_count(), 2);
        assert!(names(&app, 1).contains(&"Ink copy".to_string()));
    }

    /// techneissa's report: two copies from one document into another, with edits between them,
    /// leave both copies (and the edits) in the destination, and each Undo takes one step of it.
    #[test]
    fn two_copies_into_another_document_with_edits_between_keep_every_layer() {
        let mut app = make_app();
        app.session.execute("file.new", json!({"width":8,"height":8,"name":"Card"})).unwrap();
        for name in ["One", "Two"] {
            app.session.execute("layer.new.layer", json!({"name": name})).unwrap();
            app.session.execute("select.rect", json!({"x":1,"y":1,"width":3,"height":3})).unwrap();
            app.session.execute("edit.fill", json!({"color": if name == "One" { "#ff0000" } else { "#0000ff" }})).unwrap();
            app.session.execute("select.deselect", json!({})).unwrap();
        }
        let card_before = app.session.documents()[1].doc.clone();
        let copy_from_card = |app: &mut PhotocraftApp, name: &str| {
            app.session.set_active(1);
            app.session.execute("layer.select", json!({"name": name})).unwrap();
            let dialog = invoke(app);
            app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!(0));
            crate::dialogs::confirm(app, dialog).unwrap();
            assert_eq!(app.session.active_index(), Some(0), "the destination is shown after the copy");
        };
        copy_from_card(&mut app, "One");
        // Edits in the destination.
        app.run("select.rect", json!({"x":0,"y":0,"width":2,"height":2})).unwrap();
        app.run("edit.fill", json!({"color": "#00ff00"})).unwrap();
        app.run("select.deselect", json!({})).unwrap();
        app.run("layer.new.layer", json!({"name": "Notes"})).unwrap();
        let before_second = app.session.documents()[0].doc.clone();
        assert_eq!(names(&app, 0), ["Background", "Ink", "One copy", "Notes"]);
        copy_from_card(&mut app, "Two");
        let after = names(&app, 0);
        assert_eq!(after.len(), 5, "Background, Ink, One copy, Notes and Two copy: {after:?}");
        assert!(after.contains(&"Two copy".to_string()) && after.contains(&"One copy".to_string()) && after.contains(&"Notes".to_string()), "{after:?}");
        for (_, _, l) in before_second.walk() {
            assert_eq!(app.session.documents()[0].doc.layer(l.id), Some(l), "{} is unchanged", l.name);
        }
        assert_eq!(app.session.documents()[1].doc, card_before, "the source document is untouched");
        // One Undo takes the second copy only.
        assert!(app.session.undo());
        assert_eq!(app.session.documents()[0].doc, before_second);
        assert_eq!(app.session.documents()[1].doc, card_before);
    }

    /// A PNG file in `dir`: `w`×`h`, opaque white (a Background layer on opening) or, with `alpha`,
    /// a transparent canvas with a red square (an RGBA PNG).
    fn png_file(dir: &std::path::Path, name: &str, w: u32, alpha: bool) -> String {
        let mut s = photocraft_engine::Session::new();
        if alpha {
            s.execute("file.new", json!({"width": w, "height": w, "background": "transparent"})).unwrap();
            s.execute("select.rect", json!({"x": 1, "y": 1, "width": 3, "height": 3})).unwrap();
            s.execute("edit.fill", json!({"color": "#ff0000"})).unwrap();
        } else {
            s.execute("file.new", json!({"width": w, "height": w, "background": "white"})).unwrap();
        }
        let path = dir.join(name);
        std::fs::write(&path, photocraft_io::export(&s.active().unwrap().doc, "p.png", &Default::default()).unwrap().bytes).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// The Layers-row right-click menu's Duplicate: select the row, then invoke the command (which
    /// opens the dialog), pick the destination, OK.
    fn row_menu_duplicate_to(app: &mut PhotocraftApp, source: usize, layer: photocraft_doc::LayerId, destination: usize) {
        app.session.set_active(source);
        app.run("layer.select", json!({"layer": layer.0})).unwrap();
        let ctx = egui::Context::default();
        let dialog = crate::menus::invoke(app, &ctx, "layer.duplicate", json!({})).unwrap()["dialog"].as_u64().unwrap();
        app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!(destination));
        crate::dialogs::confirm(app, dialog).unwrap();
        assert_eq!(app.session.active_index(), Some(destination));
    }

    /// techneissa's report, with PNGs opened from files and the Layers-row menu: two Duplicates of
    /// layers of image 2 into image 1, with an edit in image 1 between them, leave image 1 with
    /// all three layers and image 2 untouched; each Undo takes exactly one step.
    #[test]
    fn png_documents_take_two_row_menu_duplicates_from_another_png_with_an_edit_between() {
        let dir = std::env::temp_dir().join(format!("photocraft-dup-png-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (a_alpha, b_alpha, extra) in
            [(false, false, 0), (false, true, 0), (true, false, 0), (true, true, 0), (false, false, 1), (true, false, 1), (false, true, 1)]
        {
            let services = crate::Services {
                import: Some(Box::new(|name, bytes, opts| {
                    photocraft_io::import_with_options(name, bytes, opts).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string())
                })),
                ..Default::default()
            };
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
            app.open_path(&png_file(&dir, "one.png", 8, a_alpha)).unwrap();
            app.open_path(&png_file(&dir, "two.png", 8, b_alpha)).unwrap();
            assert_eq!(app.session.documents().len(), 2);
            // `extra` more layers, painted, in each image (the layer rows techneissa right-clicked).
            for doc in 0..2 {
                for n in 0..extra {
                    app.session.set_active(doc);
                    app.run("layer.new.layer", json!({"name": format!("Paint {doc}.{n}")})).unwrap();
                    app.run("select.rect", json!({"x": 2, "y": 2, "width": 3, "height": 3})).unwrap();
                    app.run("edit.fill", json!({"color": "#0000ff"})).unwrap();
                    app.run("select.deselect", json!({})).unwrap();
                }
            }
            let tag = format!("A alpha {a_alpha}, B alpha {b_alpha}, {extra} extra");
            let a_start = names(&app, 0);
            let b_layers: Vec<_> = app.session.documents()[1].doc.walk().into_iter().map(|(_, _, l)| l.id).collect();
            let b_before = app.session.documents()[1].doc.clone();
            eprintln!("{tag}: A {a_start:?}, B {:?}", names(&app, 1));
            // The panel lists the top layer first: the first row, then the second.
            let first = *b_layers.last().unwrap();
            let second = b_layers.iter().rev().nth(1).copied().unwrap_or(first);
            row_menu_duplicate_to(&mut app, 1, first, 0);
            assert_eq!(names(&app, 0).len(), a_start.len() + 1, "{tag}: {:?}", names(&app, 0));
            // An edit in image 1: paint on the active layer.
            app.run("select.rect", json!({"x":0,"y":0,"width":2,"height":2})).unwrap();
            app.run("edit.fill", json!({"color": "#00ff00"})).unwrap();
            app.run("select.deselect", json!({})).unwrap();
            let a_before_second = app.session.documents()[0].doc.clone();
            row_menu_duplicate_to(&mut app, 1, second, 0);
            let a = &app.session.documents()[0].doc;
            assert_eq!(a.walk().len(), a_before_second.walk().len() + 1, "{tag}: {:?}", names(&app, 0));
            assert_eq!(a.walk().len(), a_start.len() + 2, "{tag}: {:?}", names(&app, 0));
            for (_, _, l) in a_before_second.walk() {
                assert!(a.layer(l.id) == Some(l), "{tag}: {} is unchanged", l.name);
            }
            assert!(app.session.documents()[1].doc == b_before, "{tag}: image 2 is untouched");
            assert!(app.session.undo());
            assert!(app.session.documents()[0].doc == a_before_second, "{tag}: one Undo takes the second copy");
            assert!(app.session.documents()[1].doc == b_before, "{tag}: image 2 is untouched by Undo");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_destination_makes_a_named_document_holding_just_the_copy() {
        let mut app = make_app();
        let dialog = invoke(&mut app);
        let fields = &mut app.ui.dialog_mut(dialog).unwrap().fields;
        fields.insert("destination".into(), json!("new"));
        fields.insert("documentName".into(), json!("Card"));
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert_eq!(app.session.documents().len(), 2);
        assert_eq!(app.session.active_index(), Some(1));
        assert_eq!(app.session.documents()[1].doc.name, "Card");
        assert_eq!(app.session.documents()[0].doc.layer_count(), 2, "the source is untouched");
        assert!(names(&app, 1).contains(&"Ink copy".to_string()));
    }

    #[test]
    fn a_new_destination_without_a_name_defaults_it_and_refuses_a_dead_index() {
        let mut app = make_app();
        let dialog = invoke(&mut app);
        app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!("new"));
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert_eq!(app.session.documents()[1].doc.name, "Poster copy");
        let dialog = invoke(&mut app);
        app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!(7));
        assert!(crate::dialogs::confirm(&mut app, dialog).is_err());
        assert_eq!(app.session.documents().len(), 2);
    }

    #[test]
    fn a_single_group_gets_the_group_title() {
        let mut app = make_app();
        app.session.execute("layer.new.group", json!({"name": "Pack"})).unwrap();
        let _dialog = invoke(&mut app);
        assert_eq!(app.ui.dialogs[0].fields["__label"], "Duplicate Group…");
    }

    #[test]
    fn a_changed_document_refuses_and_direct_runs_stay_dialog_free() {
        let mut app = make_app();
        let dialog = invoke(&mut app);
        app.session.execute("file.new", json!({"width":8,"height":8})).unwrap();
        assert!(crate::dialogs::confirm(&mut app, dialog).is_err());
        app.session.set_active(0);
        app.run("layer.duplicate", json!({})).unwrap();
        app.run("layer.new.layerViaCopy", json!({})).unwrap();
        assert!(app.ui.dialogs.is_empty());
        assert_eq!(app.session.active().unwrap().doc.layer_count(), 4);
    }

    /// techneissa lost a copy that went into the current document instead of the other one: the
    /// status bar names the document that received it, in every case.
    #[test]
    fn ok_says_which_document_received_the_copy() {
        let mut app = make_app();
        app.session.execute("file.new", json!({"width":8,"height":8,"name":"Card"})).unwrap();
        app.session.set_active(0);
        // The current document (the dialog's default).
        let dialog = invoke(&mut app);
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert_eq!(app.ui.status, "Duplicated to Poster");
        assert!(!app.ui.status_error);
        // Another document.
        app.session.set_active(0);
        let dialog = invoke(&mut app);
        app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!(1));
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert_eq!(app.ui.status, "Duplicated to Card");
        // A new document, under the name typed.
        app.session.set_active(0);
        let dialog = invoke(&mut app);
        {
            let fields = &mut app.ui.dialog_mut(dialog).unwrap().fields;
            fields.insert("destination".into(), json!("new"));
            fields.insert("documentName".into(), json!("Proof"));
        }
        crate::dialogs::confirm(&mut app, dialog).unwrap();
        assert_eq!(app.ui.status, "Duplicated to Proof");
        // A refused copy keeps its error and says nothing about a destination.
        app.ui.status.clear();
        let dialog = invoke(&mut app);
        app.ui.dialog_mut(dialog).unwrap().fields.insert("destination".into(), json!(9));
        assert!(crate::dialogs::confirm(&mut app, dialog).is_err());
        assert!(app.ui.status.is_empty());
    }
}
