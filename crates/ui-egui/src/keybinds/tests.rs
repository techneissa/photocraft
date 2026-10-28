use egui::{Event, Key, KeyboardShortcut, Modifiers};
use serde_json::json;

use super::*;
use crate::PhotocraftApp;
use crate::state::Tool;

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
}

fn press(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

/// One frame at `time` with these key presses, through the real key pass.
fn frame(app: &mut PhotocraftApp, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    ctx.begin_pass(egui::RawInput { events, time: Some(time), ..Default::default() });
    crate::shortcuts::handle(app, ctx);
    ctx.end_pass().textures_delta.clear();
}

fn with_chords(chords: &[(&str, &str)]) -> PhotocraftApp {
    let mut app = app();
    app.session.edit_prefs(|p| {
        for (id, c) in chords {
            p.chords.insert((*id).to_string(), (*c).to_string());
        }
    });
    app
}

fn set_when(app: &mut PhotocraftApp, id: &str, when: &str) {
    app.session.edit_prefs(|p| _ = p.shortcut_when.insert(id.to_string(), when.to_string()));
}

#[test]
fn chords_parse_and_show_like_shortcuts() {
    let [first, second] = chord::steps("Cmd+K C").unwrap();
    assert_eq!(first, KeyboardShortcut::new(Modifiers::COMMAND, Key::K));
    assert_eq!(second, KeyboardShortcut::new(Modifiers::NONE, Key::C));
    assert_eq!(chord::text("Cmd+K C"), if cfg!(target_os = "macos") { "⌘K C" } else { "Ctrl+K C" });
    for bad in ["", "Cmd+K", "Cmd+K C D", "Cmd+K Nope", "Hyper+K C"] {
        assert!(chord::steps(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn a_chord_runs_its_command_on_the_second_key() {
    let mut app = with_chords(&[("tools.select.brush", "Cmd+K B")]);
    app.ui.tool = Tool::Move;
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, 1.0, vec![press(Key::K, Modifiers::COMMAND)]);
    assert!(app.keybinds.pending.is_some(), "the leader waits for the next key");
    assert_eq!(app.ui.tool, Tool::Move);
    frame(&mut app, &ctx, 1.5, vec![press(Key::B, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_none());
    assert_eq!(app.ui.tool, Tool::Brush);
}

#[test]
fn the_second_key_works_with_the_leaders_modifier_still_held() {
    let mut app = with_chords(&[("tools.select.brush", "Cmd+K B")]);
    app.ui.tool = Tool::Move;
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, 1.0, vec![press(Key::K, Modifiers::COMMAND)]);
    frame(&mut app, &ctx, 1.2, vec![press(Key::B, Modifiers::COMMAND)]);
    assert_eq!(app.ui.tool, Tool::Brush);
}

#[test]
fn the_first_key_of_a_chord_does_not_fire_its_own_binding() {
    // E selects the Eraser by itself; as the leader of E B it must wait instead
    let mut app = with_chords(&[("tools.select.brush", "E B")]);
    app.ui.tool = Tool::Move;
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, 1.0, vec![press(Key::E, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_some());
    assert_eq!(app.ui.tool, Tool::Move, "E did not select the Eraser");
    frame(&mut app, &ctx, 1.3, vec![press(Key::B, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Brush);
    // with no chord, E is the Eraser again
    let mut plain = app_with_tool_move();
    let ctx = egui::Context::default();
    frame(&mut plain, &ctx, 2.0, vec![press(Key::E, Modifiers::NONE)]);
    assert_eq!(plain.ui.tool, Tool::Eraser);
}

fn app_with_tool_move() -> PhotocraftApp {
    let mut a = app();
    a.ui.tool = Tool::Move;
    a
}

#[test]
fn a_wrong_key_or_escape_cancels_and_is_swallowed() {
    let ctx = egui::Context::default();
    let mut app = with_chords(&[("tools.select.brush", "E B")]);
    app.ui.tool = Tool::Move;
    frame(&mut app, &ctx, 1.0, vec![press(Key::E, Modifiers::NONE)]);
    // Z would be the Zoom tool; here it only ends the chord
    frame(&mut app, &ctx, 1.2, vec![press(Key::Z, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_none());
    assert_eq!(app.ui.tool, Tool::Move);
    assert!(app.ui.status_error && app.ui.status.contains("no shortcut continues"), "{}", app.ui.status);
    app.ui.status.clear();
    frame(&mut app, &ctx, 2.0, vec![press(Key::E, Modifiers::NONE)]);
    frame(&mut app, &ctx, 2.2, vec![press(Key::Escape, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_none());
    assert!(app.ui.status.is_empty(), "Esc cancels quietly");
}

#[test]
fn a_chord_that_waits_too_long_is_forgotten() {
    let ctx = egui::Context::default();
    let mut app = with_chords(&[("tools.select.brush", "E B")]);
    app.ui.tool = Tool::Move;
    frame(&mut app, &ctx, 1.0, vec![press(Key::E, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_some());
    // past the timeout the key is an ordinary key again: Z is the Zoom tool
    frame(&mut app, &ctx, 1.0 + runtime::TIMEOUT + 0.5, vec![press(Key::Z, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_none());
    assert_eq!(app.ui.tool, Tool::Zoom);
}

#[test]
fn a_leader_does_not_arm_while_typing_or_in_a_dialog() {
    let ctx = egui::Context::default();
    let mut app = with_chords(&[("tools.select.brush", "E B")]);
    crate::menus::invoke(&mut app, &ctx, "edit.keyboardShortcuts", json!({})).unwrap();
    frame(&mut app, &ctx, 1.0, vec![press(Key::E, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_none(), "a dialog owns the keyboard");
}

#[test]
fn a_chord_follows_its_condition() {
    let ctx = egui::Context::default();
    let mut app = with_chords(&[("tools.select.brush", "E B")]);
    set_when(&mut app, "tools.select.brush", "hasDocument");
    app.ui.tool = Tool::Move;
    // no document: the chord does not exist, so E is the Eraser
    frame(&mut app, &ctx, 1.0, vec![press(Key::E, Modifiers::NONE)]);
    assert!(app.keybinds.pending.is_none());
    assert_eq!(app.ui.tool, Tool::Eraser);
}

#[test]
fn the_hint_shows_while_a_chord_waits() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    let mut h = Harness::builder().with_size(egui::vec2(900.0, 600.0)).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.session.edit_prefs(|p| _ = p.chords.insert("tools.select.brush".into(), "Cmd+K B".into()));
        app
    });
    h.run_steps(2);
    assert!(h.query_by_label("Cancel").is_none(), "no hint without a chord waiting");
    let [first, _] = chord::steps("Cmd+K B").unwrap();
    let now = h.ctx.input(|i| i.time);
    h.state_mut().keybinds.pending = Some(runtime::Pending { first, since: now });
    h.run_steps(2);
    h.get_by_label("Cancel");
    // and it goes away with the chord
    h.state_mut().keybinds.pending = None;
    h.run_steps(2);
    assert!(h.query_by_label("Cancel").is_none());
}

#[test]
fn a_binding_whose_condition_is_false_does_not_fire() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.ui.tool = Tool::Move;
    set_when(&mut app, "tools.select.brush", "hasDocument");
    // B is the Brush by default, but there is no document: the key goes to the next tool of its group
    frame(&mut app, &ctx, 1.0, vec![press(Key::B, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Pencil);
    // and a command with a key of its own (Copy, ⌘C) leaves the table while its condition fails
    set_when(&mut app, "edit.copy", "hasDocument");
    let cx = context::read(&app, crate::shortcut_dispatch::Focus::None);
    let c = KeyboardShortcut::new(Modifiers::COMMAND, Key::C);
    assert!(!crate::shortcut_dispatch::bindings_in(&app, Some(&cx)).iter().any(|(id, sc)| id == "edit.copy" && *sc == c));
    assert!(crate::shortcut_dispatch::bindings(&app).iter().any(|(id, sc)| id == "edit.copy" && *sc == c), "the plain table keeps it");
    crate::menus::invoke(&mut app, &ctx, "file.new", json!({"width": 8, "height": 8})).unwrap();
    let cx = context::read(&app, crate::shortcut_dispatch::Focus::None);
    assert!(crate::shortcut_dispatch::bindings_in(&app, Some(&cx)).iter().any(|(id, sc)| id == "edit.copy" && *sc == c));
}

#[test]
fn two_commands_share_a_key_while_their_conditions_exclude_each_other() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.session.edit_prefs(|p| {
        p.shortcuts.insert("tools.select.pencil".into(), "F9".into());
        p.shortcuts.insert("tools.select.eraser".into(), "F9".into());
    });
    set_when(&mut app, "tools.select.pencil", "tool == move");
    set_when(&mut app, "tools.select.eraser", "tool != move");
    app.ui.tool = Tool::Move;
    frame(&mut app, &ctx, 1.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Pencil);
    app.ui.tool = Tool::Lasso;
    frame(&mut app, &ctx, 2.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Eraser);
}

#[test]
fn a_condition_that_asks_for_a_dialog_works_inside_it() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.ui.tool = Tool::Move;
    app.session.edit_prefs(|p| _ = p.shortcuts.insert("tools.select.brush".into(), "F9".into()));
    set_when(&mut app, "tools.select.brush", "dialogOpen");
    // no dialog: the condition fails
    frame(&mut app, &ctx, 1.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Move);
    crate::menus::invoke(&mut app, &ctx, "edit.keyboardShortcuts", json!({})).unwrap();
    frame(&mut app, &ctx, 2.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Brush);
    // a plain binding stays out of dialogs
    app.ui.tool = Tool::Move;
    app.session.edit_prefs(|p| _ = p.shortcut_when.remove("tools.select.brush"));
    frame(&mut app, &ctx, 3.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Move);
}

#[test]
fn the_context_reads_the_app() {
    use super::when::keys::*;
    let mut app = app();
    app.ui.tool = Tool::Move;
    let cx = context::read(&app, crate::shortcut_dispatch::Focus::None);
    assert!(!cx.flag(HAS_DOCUMENT) && !cx.flag(TEXT_FOCUS) && cx.consistent());
    assert_eq!(cx.value_name(TOOL), "move");
    app.ui.tool = Tool::Brush;
    let cx = context::read(&app, crate::shortcut_dispatch::Focus::Text);
    assert_eq!(cx.value_name(TOOL), "brush");
    assert!(cx.flag(TEXT_FOCUS) && !cx.flag(WIDGET_FOCUS));
    let ctx = egui::Context::default();
    crate::menus::invoke(&mut app, &ctx, "file.new", json!({"width": 16, "height": 16})).unwrap();
    let cx = context::read(&app, crate::shortcut_dispatch::Focus::Widget);
    assert!(cx.flag(HAS_DOCUMENT) && cx.flag(WIDGET_FOCUS) && !cx.flag(HAS_SELECTION) && !cx.flag(HAS_PATH) && cx.consistent());
    assert_eq!(cx.value_name(LAYER_KIND), "raster");
}

#[test]
fn every_tool_has_a_value_in_the_tool_key() {
    let mut app = app();
    for t in Tool::ALL {
        app.ui.tool = t;
        let cx = context::read(&app, crate::shortcut_dispatch::Focus::None);
        let name = crate::tool_shortcuts::id(t);
        assert_eq!(format!("tools.select.{}", cx.value_name(super::when::keys::TOOL)), name, "{t:?}");
    }
}
