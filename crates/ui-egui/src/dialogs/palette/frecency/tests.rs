use craft::ui::runtime::commands::CommandRef;
use photocraft_engine::prefs::SavedPrefsExt as _;
use serde_json::json;

use super::{History, MAX_COMMANDS};

const DAY: i64 = 24 * 3600;
const NOW: i64 = 1_800_000_000;

#[test]
fn a_command_run_more_often_or_more_lately_has_more_use() {
    let mut h = History::default();
    h.record("b.often", "", NOW - DAY);
    h.record("b.often", "", NOW - DAY);
    h.record("c.once", "", NOW - DAY);
    h.record("old", "", NOW - 60 * DAY);
    h.record("old", "", NOW - 60 * DAY);
    let uses = |id| h.uses(id, NOW);
    assert!(uses("b.often") > uses("c.once"), "more often");
    assert!(uses("c.once") > uses("old"), "a use of yesterday counts more than two of two months ago");
    assert_eq!((uses("a.never"), uses("old")), (0, 0), "never run, and run too long ago, count nothing");
}

#[test]
fn the_command_picked_for_a_text_comes_first_for_that_text_and_its_start() {
    let mut h = History::default();
    h.record("file.export.exportAs", "Exp", NOW - DAY);
    for text in ["exp", "ex", "EXP ", "e"] {
        assert_eq!(h.learned(text), Some("file.export.exportAs"), "{text:?}");
    }
    // A longer text than the one picked for, another text, or none has no pick.
    for text in ["expo", "q", "  "] {
        assert_eq!(h.learned(text), None, "{text:?}");
    }
    // The newest pick for the text wins.
    h.record("file.export.quickExport", "ex", NOW);
    assert_eq!(h.learned("ex"), Some("file.export.quickExport"));
    assert_eq!(h.learned("exp"), Some("file.export.exportAs"), "its own text keeps its pick");
}

#[test]
fn an_empty_search_lists_the_most_used_and_most_recent_first() {
    let mut h = History::default();
    h.record("rare", "", NOW - 30 * DAY);
    for _ in 0..3 {
        h.record("often", "", NOW - 2 * DAY);
    }
    h.record("today", "", NOW);
    // This session's commands follow the learned ones, once.
    let session = [CommandRef::new("session"), CommandRef::new("today")];
    let ids: Vec<String> = h.recents(&session, NOW).into_iter().map(|c| c.id).collect();
    assert_eq!(ids, ["often", "today", "rare", "session"]);
}

#[test]
fn the_history_survives_a_restart_through_the_preferences() {
    let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    super::note(&mut app, "image.imageSize", "size");
    super::note(&mut app, "image.imageSize", "");
    let saved = app.session.prefs().to_json();
    assert!(saved["paletteHistory"]["uses"]["image.imageSize"]["count"] == json!(2), "{}", saved["paletteHistory"]);
    let mut next = photocraft_engine::Session::new();
    next.load_prefs_json(&saved.to_string()).unwrap();
    let app = crate::PhotocraftApp::new(next, crate::Services::default());
    assert_eq!(History::of(&app).learned("si"), Some("image.imageSize"));
    // An unreadable value is an empty history, never an error.
    assert_eq!(History::from_value(&json!({"uses": 5})), History::default());
}

#[test]
fn the_history_keeps_a_bounded_number_of_commands_and_texts() {
    let mut h = History::default();
    for i in 0..MAX_COMMANDS + 50 {
        h.record(&format!("cmd.{i}"), &format!("t{i}"), NOW + i as i64);
    }
    assert_eq!(h.uses.len(), MAX_COMMANDS);
    assert!(h.uses.contains_key(&format!("cmd.{}", MAX_COMMANDS + 49)), "the newest stays");
    assert!(h.picks.len() <= super::MAX_PICKS);
    assert!(h.picks.contains_key(&format!("t{}", MAX_COMMANDS + 49)));
}

mod through_the_palette {
    use egui::accesskit::Role;
    use egui::{Event, Key, Modifiers, vec2};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};

    use crate::PhotocraftApp;

    fn harness() -> Harness<'static, Box<PhotocraftApp>> {
        let mut h = Harness::builder().with_size(vec2(1440.0, 900.0)).with_max_steps(64).build_eframe(|cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            PhotocraftApp::with_context(cc.egui_ctx.clone(), photocraft_engine::Session::new(), crate::Services::default())
        });
        h.run_steps(4);
        h
    }

    /// Opens the palette, types `text` and returns the labels of the listed rows, best first.
    fn open_and_type(h: &mut Harness<'_, Box<PhotocraftApp>>, text: &str) -> Vec<String> {
        h.state_mut().ui.palette.open = true;
        h.run_steps(6);
        h.event(Event::Text(text.to_owned()));
        h.run_steps(6);
        h.query_all_by_role(Role::ListBoxOption).filter_map(|n| n.accesskit_node().label()).collect()
    }

    #[test]
    fn a_command_picked_for_a_typed_text_comes_first_for_that_text_and_its_start() {
        let mut h = harness();
        let before = open_and_type(&mut h, "exp");
        assert!(before.len() > 2, "{before:?}");
        // Pick the last of them, a command that never ranks first for this text.
        let picked = before.last().cloned().unwrap_or_default();
        for _ in 1..before.len() {
            h.key_press(Key::ArrowDown);
            h.run_steps(2);
        }
        h.key_press_modifiers(Modifiers::NONE, Key::Enter);
        h.run_steps(6);
        assert!(!h.state().ui.palette.open, "the pick closed the palette");
        for text in ["exp", "ex"] {
            let rows = open_and_type(&mut h, text);
            assert_eq!(rows.first(), Some(&picked), "{text:?}: {rows:?}");
            h.key_press(Key::Escape);
            h.key_press(Key::Escape);
            h.run_steps(4);
        }
    }
}
