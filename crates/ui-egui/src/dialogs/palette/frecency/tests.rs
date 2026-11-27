use serde_json::json;

use super::super::nav::{Hit, rank};
use super::{CLEARLY_BETTER, History, MAX_COMMANDS};

const DAY: i64 = 24 * 3600;
const NOW: i64 = 1_800_000_000;

fn hit(score: i32, id: &str) -> Hit {
    (score, id.to_string(), id.to_string(), None, true)
}

/// The ids of `hits` for `text`, ordered as the palette lists them.
fn order(history: &History, text: &str, mut hits: Vec<Hit>) -> Vec<String> {
    history.apply(text, NOW, &mut hits);
    rank(&mut hits);
    hits.into_iter().map(|h| h.1).collect()
}

#[test]
fn among_equal_matches_the_command_run_more_often_goes_first() {
    let mut h = History::default();
    h.record("b.often", "", NOW - DAY);
    h.record("b.often", "", NOW - DAY);
    h.record("c.once", "", NOW - DAY);
    let hits = vec![hit(20, "a.never"), hit(20, "c.once"), hit(20, "b.often")];
    assert_eq!(order(&h, "x", hits), ["b.often", "c.once", "a.never"]);
}

#[test]
fn a_clearly_better_text_match_still_wins_over_past_use() {
    let mut h = History::default();
    for _ in 0..500 {
        h.record("used", "", NOW);
    }
    let hits = vec![hit(20, "used"), hit(20 + CLEARLY_BETTER, "better")];
    assert_eq!(order(&h, "x", hits), ["better", "used"]);
}

#[test]
fn a_recent_use_counts_more_than_an_old_one() {
    let mut h = History::default();
    h.record("old", "", NOW - 60 * DAY);
    h.record("old", "", NOW - 60 * DAY);
    h.record("recent", "", NOW - DAY);
    assert_eq!(order(&h, "x", vec![hit(10, "old"), hit(10, "recent")]), ["recent", "old"]);
}

#[test]
fn the_command_picked_for_a_text_comes_first_for_that_text_and_its_start() {
    let mut h = History::default();
    h.record("file.export.exportAs", "Exp", NOW - DAY);
    let hits = || vec![hit(400, "file.export.quickExport"), hit(30, "file.export.exportAs"), hit(1000, "layer.exportAs")];
    for text in ["exp", "ex", "EXP ", "e"] {
        assert_eq!(order(&h, text, hits())[0], "file.export.exportAs", "{text:?}");
    }
    // A longer text than the one picked for, or another text, has no pick.
    assert_eq!(order(&h, "expo", hits())[0], "layer.exportAs");
    assert_eq!(order(&h, "q", hits())[0], "layer.exportAs");
    // The newest pick for the text wins.
    h.record("file.export.quickExport", "ex", NOW);
    assert_eq!(order(&h, "ex", hits())[0], "file.export.quickExport");
    // A pick that doesn't match the text isn't added.
    assert_eq!(order(&h, "ex", vec![hit(5, "other")]), ["other"]);
}

#[test]
fn an_empty_search_lists_the_most_used_and_most_recent_first() {
    let mut h = History::default();
    h.record("rare", "", NOW - 30 * DAY);
    for _ in 0..3 {
        h.record("often", "", NOW - 2 * DAY);
    }
    h.record("today", "", NOW);
    // This session's recent command (score 1000) and the rest (0) follow the learned ones.
    let hits = vec![hit(0, "a.unused"), hit(1000, "session"), hit(0, "rare"), hit(0, "today"), hit(0, "often")];
    assert_eq!(order(&h, "  ", hits), ["often", "today", "rare", "session", "a.unused"]);
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
    let h = History::of(&app);
    assert_eq!(order(&h, "si", vec![hit(30, "image.canvasSize"), hit(30, "image.imageSize")])[0], "image.imageSize");
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
