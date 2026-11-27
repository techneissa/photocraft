use egui::{Event, Key, KeyboardShortcut, Modifiers};
use serde_json::json;

use super::*;
use crate::PhotocraftApp;
use crate::state::Tool;

fn app() -> Box<PhotocraftApp> {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    PhotocraftApp::setup_context(app.ctx(), Default::default());
    app
}

fn press(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

/// One frame at `time` with these key presses, through the real flow: the app's `raw_input_hook` (which holds the
/// fork's key layer), then craft's key stage and the run of the commands the keys fired.
fn frame(app: &mut PhotocraftApp, _unused: &egui::Context, time: f64, events: Vec<Event>) {
    let ctx = &app.ctx().clone();
    let mut raw = egui::RawInput { events, time: Some(time), ..Default::default() };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    crate::app::framework::keys_pass(app, ctx, raw, |_| {});
}

fn with_chords(chords: &[(&str, &str)]) -> Box<PhotocraftApp> {
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

/// The whole path: the app's `raw_input_hook` hands the key layer the frame's presses, the layer fires a chord's command
/// or a command whose When holds, and the commands run through the app's own dispatch. Without the hook in
/// `raw_input_hook`, craft's keymap knows neither: the chord does nothing and the When-gated key is unbound there.
#[test]
fn a_chord_and_a_when_gated_binding_run_their_command_through_the_real_key_dispatch() {
    let ctx = egui::Context::default();
    let mut app = with_chords(&[("tools.select.brush", "Cmd+K B")]);
    app.session.edit_prefs(|p| _ = p.shortcuts.insert("tools.select.pencil".into(), "F9".into()));
    set_when(&mut app, "tools.select.pencil", "hasDocument");
    app.ui.tool = Tool::Move;
    frame(&mut app, &ctx, 1.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Move, "no document: F9 does nothing");
    crate::menus::invoke(&mut app, &ctx, "file.new", json!({"width": 8, "height": 8})).unwrap();
    frame(&mut app, &ctx, 2.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Pencil, "with a document the When holds and F9 selects the Pencil");
    frame(&mut app, &ctx, 3.0, vec![press(Key::K, Modifiers::COMMAND)]);
    assert!(app.keybinds.pending.is_some() && app.ui.tool == Tool::Pencil, "⌘K waits for the second key");
    frame(&mut app, &ctx, 3.3, vec![press(Key::B, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Brush, "⌘K B selects the Brush");
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

fn app_with_tool_move() -> Box<PhotocraftApp> {
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
fn a_binding_whose_condition_is_false_goes_to_the_next_command_with_the_key() {
    let ctx = egui::Context::default();
    let mut app = app();
    app.ui.tool = Tool::Move;
    set_when(&mut app, "tools.select.brush", "hasDocument");
    // B is the Brush by default, but there is no document: the key goes on to craft, whose B group has lost the Brush
    frame(&mut app, &ctx, 1.0, vec![press(Key::B, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Pencil);
    // a command with a key of its own (Copy, ⌘C) runs when its condition holds, through the layer
    app.ui.tool = Tool::Move;
    crate::menus::invoke(&mut app, &ctx, "file.new", json!({"width": 8, "height": 8})).unwrap();
    app.session.edit_prefs(|p| _ = p.shortcuts.insert("tools.select.brush".into(), "F9".into()));
    frame(&mut app, &ctx, 2.0, vec![press(Key::F9, Modifiers::NONE)]);
    assert_eq!(app.ui.tool, Tool::Brush, "the condition holds with a document");
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
    crate::menus::invoke(&mut app, &ctx, "file.new", json!({})).unwrap();
    frame(&mut app, &ctx, 1.5, vec![]);
    assert!(app.dialog_open(), "File > New is open");
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
    let cx = context::read(&app, super::focus::Focus::None);
    assert!(!cx.flag(HAS_DOCUMENT) && !cx.flag(TEXT_FOCUS) && cx.consistent());
    assert_eq!(cx.value_name(TOOL), "move");
    app.ui.tool = Tool::Brush;
    let cx = context::read(&app, super::focus::Focus::Text);
    assert_eq!(cx.value_name(TOOL), "brush");
    assert!(cx.flag(TEXT_FOCUS) && !cx.flag(WIDGET_FOCUS));
    let ctx = egui::Context::default();
    crate::menus::invoke(&mut app, &ctx, "file.new", json!({"width": 16, "height": 16})).unwrap();
    let cx = context::read(&app, super::focus::Focus::Widget);
    assert!(cx.flag(HAS_DOCUMENT) && cx.flag(WIDGET_FOCUS) && !cx.flag(HAS_SELECTION) && !cx.flag(HAS_PATH) && cx.consistent());
    assert_eq!(cx.value_name(LAYER_KIND), "raster");
}

#[test]
fn every_tool_has_a_value_in_the_tool_key() {
    let mut app = app();
    for t in Tool::ALL {
        app.ui.tool = t;
        let cx = context::read(&app, super::focus::Focus::None);
        let name = crate::tool_shortcuts::id(t);
        assert_eq!(format!("tools.select.{}", cx.value_name(super::when::keys::TOOL)), name, "{t:?}");
    }
}

/// One frame with a text box drawn (and focused from the first), then the keys through the key layer and the key pass.
fn typing_frame(app: &mut PhotocraftApp, time: f64, events: Vec<Event>, text: &mut String) -> egui::RawInput {
    let ctx = app.ctx().clone();
    let mut raw = egui::RawInput { events, time: Some(time), ..Default::default() };
    eframe::App::raw_input_hook(app, &ctx, &mut raw);
    let seen = raw.clone();
    crate::app::framework::keys_pass(app, &ctx, raw, |ui| {
        let r = ui.add(egui::TextEdit::singleline(text).id(egui::Id::new("pc-test-text")));
        r.request_focus();
    });
    seen
}

#[test]
fn a_leader_does_not_eat_typing_in_a_text_box() {
    let mut app = with_chords(&[("tools.select.brush", "Cmd+K C")]);
    let mut text = String::new();
    typing_frame(&mut app, 1.0, vec![], &mut text);
    typing_frame(&mut app, 1.1, vec![], &mut text);
    assert!(app.ctx().text_edit_focused(), "the box has the keyboard");
    typing_frame(&mut app, 1.2, vec![press(Key::K, Modifiers::COMMAND)], &mut text);
    assert!(app.keybinds.pending.is_none(), "⌘K did not arm the chord inside a text box");
    // the letter after it still reaches the box: the layer leaves it in the frame's events
    let seen = typing_frame(&mut app, 1.3, vec![press(Key::C, Modifiers::NONE), Event::Text("c".into())], &mut text);
    assert!(seen.events.iter().any(|e| matches!(e, Event::Text(t) if t == "c")), "the c is left for the field");
}

#[test]
fn a_chord_that_asks_for_text_focus_may_start_in_a_text_box() {
    let mut app = with_chords(&[("tools.select.brush", "Cmd+K C")]);
    set_when(&mut app, "tools.select.brush", "textFocus");
    let mut text = String::new();
    typing_frame(&mut app, 1.0, vec![], &mut text);
    typing_frame(&mut app, 1.1, vec![], &mut text);
    typing_frame(&mut app, 1.2, vec![press(Key::K, Modifiers::COMMAND)], &mut text);
    assert!(app.keybinds.pending.is_some());
}
