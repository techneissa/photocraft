//! Tests with canned GitHub answers: nothing here reaches the network or runs the real update.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use super::github::{Fetch, Reply, friendly};
use super::release::{self, Compare, Notes, Outcome, decide, is_commit, outcome_of, parse_compare, parse_notes, same_commit};
use super::route::{self, Route, choose_commit, find_route};
use super::{Origin, Parts, Runtime, State};
use crate::{PhotocraftApp, Services};

const OWN: &str = "1111111111111111111111111111111111111111";
const BUILT: &str = "2222222222222222222222222222222222222222";

fn release_json(body: &str) -> String {
    json!({"tag_name": "custom-build", "prerelease": true, "body": body}).to_string()
}

fn compare_json(status: &str) -> String {
    json!({"status": status, "ahead_by": 3, "behind_by": 0, "total_commits": 3, "files": []}).to_string()
}

/// GitHub with canned answers; remembers every address asked.
struct Fake {
    release: Result<Reply, String>,
    compare: Result<Reply, String>,
    seen: Mutex<Vec<String>>,
}

impl Fake {
    fn new(release: Result<Reply, String>, compare: Result<Reply, String>) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { release, compare, seen: Mutex::new(Vec::new()) })
    }
    fn built(status: &str) -> std::sync::Arc<Self> {
        Self::new(Ok(Reply::Body(release_json(&format!("commit: {BUILT}")))), Ok(Reply::Body(compare_json(status))))
    }
    fn asked(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

impl Fetch for Fake {
    fn get(&self, url: &str) -> Result<Reply, String> {
        self.seen.lock().unwrap().push(url.to_string());
        if url.contains("/compare/") { self.compare.clone() } else { self.release.clone() }
    }
}

// ------------------------------------------------------------------ parsing

#[test]
fn release_notes_name_a_commit_or_say_updating() {
    assert_eq!(parse_notes(&release_json(&format!("commit: {BUILT}"))), Ok(Notes::Commit(BUILT.into())));
    assert_eq!(parse_notes(&release_json("commit: updating")), Ok(Notes::Updating));
    // Other lines, spaces and capitals do not matter.
    assert_eq!(parse_notes(&release_json(&format!("Built by CI\n  commit:   {}  \n", BUILT.to_uppercase()))), Ok(Notes::Commit(BUILT.into())));
    assert_eq!(parse_notes(&release_json("commit: Updating")), Ok(Notes::Updating));
}

#[test]
fn bad_release_notes_are_short_messages_not_panics() {
    for body in ["commit: none", "commit:", "no commit here", "", "commit: not-a-sha", "commit: 12345", "commit: abcdefg; rm -rf /"] {
        let error = parse_notes(&release_json(body)).unwrap_err();
        assert!(!error.is_empty() && error.len() < 80, "{body:?}: {error}");
    }
    for json in ["", "not json", "[]", "null", "{}", r#"{"body": null}"#, r#"{"body": 42}"#] {
        assert!(parse_notes(json).is_err(), "{json:?}");
    }
}

#[test]
fn commit_ids_are_hex_of_seven_to_forty_digits() {
    assert!(is_commit(OWN) && is_commit("abcdef1") && is_commit("ABCDEF1234"));
    for bad in ["", "abcdef", "xyz1234", &"a".repeat(41), "abc def1", "updating", "../../etc"] {
        assert!(!is_commit(bad), "{bad:?}");
    }
    assert!(same_commit(OWN, OWN) && same_commit(OWN, "1111111") && same_commit("ABCDEF1", "abcdef1234"));
    assert!(!same_commit(OWN, BUILT) && !same_commit(OWN, "111") && !same_commit("zzzzzzz", "zzzzzzz"));
}

#[test]
fn compare_status_is_read_and_unknown_answers_are_errors() {
    for (status, want) in [("identical", Compare::Identical), ("ahead", Compare::Ahead), ("behind", Compare::Behind), ("diverged", Compare::Diverged)] {
        assert_eq!(parse_compare(&compare_json(status)), Ok(want));
    }
    for json in ["", "nope", "{}", r#"{"status": 3}"#, r#"{"status": "sideways"}"#, "[]"] {
        assert!(parse_compare(json).is_err(), "{json:?}");
    }
}

#[test]
fn the_published_build_is_offered_only_when_it_has_something_the_app_lacks() {
    assert_eq!(outcome_of(Compare::Identical, BUILT), Outcome::UpToDate);
    assert_eq!(outcome_of(Compare::Behind, BUILT), Outcome::UpToDate, "an app newer than the build has nothing to install");
    assert_eq!(outcome_of(Compare::Ahead, BUILT), Outcome::Ready(BUILT.into()));
    assert_eq!(outcome_of(Compare::Diverged, BUILT), Outcome::Ready(BUILT.into()), "the update script merges what each side has");
}

// ------------------------------------------------------------------ the decision

#[test]
fn a_build_ahead_of_the_app_is_ready() {
    let fake = Fake::built("ahead");
    assert_eq!(decide(&*fake, OWN), Ok(Outcome::Ready(BUILT.into())));
    let asked = fake.asked();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0], "https://api.github.com/repos/techneissa/photocraft/releases/tags/custom-build");
    assert_eq!(asked[1], format!("https://api.github.com/repos/techneissa/photocraft/compare/{OWN}...{BUILT}?per_page=1"));
    assert!(asked.iter().all(|u| !u.contains("storytold")), "never the official project: {asked:?}");
}

#[test]
fn an_identical_or_older_build_is_up_to_date() {
    assert_eq!(decide(&*Fake::built("identical"), OWN), Ok(Outcome::UpToDate));
    assert_eq!(decide(&*Fake::built("behind"), OWN), Ok(Outcome::UpToDate));
}

#[test]
fn the_app_built_from_the_published_commit_needs_no_compare() {
    let fake = Fake::built("ahead");
    assert_eq!(decide(&*fake, BUILT), Ok(Outcome::UpToDate));
    assert_eq!(fake.asked().len(), 1, "one request: the release only");
}

#[test]
fn a_build_in_progress_is_being_made() {
    let fake = Fake::new(Ok(Reply::Body(release_json("commit: updating"))), Err("unused".into()));
    assert_eq!(decide(&*fake, OWN), Ok(Outcome::BeingMade));
    assert_eq!(fake.asked().len(), 1);
}

#[test]
fn errors_are_short_messages() {
    let short = |r: Result<Outcome, String>| {
        let e = r.unwrap_err();
        assert!(!e.is_empty() && e.chars().count() < 100, "{e}");
        e
    };
    // offline
    short(decide(&*Fake::new(Err("Couldn't reach GitHub (timed out).".into()), Err("x".into())), OWN));
    // rate limit, on either request
    let limited = Ok(Reply::RateLimited { minutes: Some(12) });
    let e = short(decide(&*Fake::new(limited.clone(), Err("x".into())), OWN));
    assert!(e.contains("12 minutes"), "{e}");
    let e = short(decide(&*Fake::new(Ok(Reply::Body(release_json(&format!("commit: {BUILT}")))), limited), OWN));
    assert!(e.contains("limiting"), "{e}");
    let e = short(decide(&*Fake::new(Ok(Reply::RateLimited { minutes: None }), Err("x".into())), OWN));
    assert!(e.contains("a minute"), "{e}");
    // no release, a release with bad notes, a commit GitHub does not know
    short(decide(&*Fake::new(Ok(Reply::NotFound), Err("x".into())), OWN));
    short(decide(&*Fake::new(Ok(Reply::Body(release_json("whatever"))), Err("x".into())), OWN));
    let e = short(decide(&*Fake::new(Ok(Reply::Body(release_json(&format!("commit: {BUILT}")))), Ok(Reply::NotFound)), OWN));
    assert!(e.contains("does not know"), "{e}");
    // the comparison itself unreadable
    short(decide(&*Fake::new(Ok(Reply::Body(release_json(&format!("commit: {BUILT}")))), Ok(Reply::Body("<html>".into()))), OWN));
    // the app's own commit unknown: nothing is asked
    let fake = Fake::built("ahead");
    short(decide(&*fake, "unknown"));
    assert!(fake.asked().is_empty());
}

#[test]
fn transport_errors_read_as_one_sentence() {
    assert_eq!(friendly("couldn't reach GitHub (timed out)".into()), "Couldn't reach GitHub (timed out).");
    assert_eq!(friendly("".into()), "Could not reach GitHub.");
    let long = friendly("x".repeat(500));
    assert!(long.chars().count() <= 121, "{}", long.chars().count());
}

// ------------------------------------------------------------------ this app's commit

#[test]
fn the_compiled_commit_comes_first_and_the_stamp_only_helps_an_installed_app() {
    assert_eq!(choose_commit(Some(OWN), Some(BUILT), true), Ok(OWN.into()));
    assert_eq!(choose_commit(Some(&OWN.to_uppercase()), None, false), Ok(OWN.into()));
    // an app packaged without a commit (Info.plist says "unknown"): the stamp of the installed app
    assert_eq!(choose_commit(Some("unknown"), Some(&format!("{BUILT}\n")), true), Ok(BUILT.into()));
    assert_eq!(choose_commit(None, Some(BUILT), true), Ok(BUILT.into()));
    // a `cargo run` build is not the installed app
    assert!(choose_commit(None, Some(BUILT), false).is_err());
    // the stamp of an official release is not a commit
    assert!(choose_commit(None, Some("release 0.9.0"), true).is_err());
    assert!(choose_commit(None, None, true).is_err());
    assert!(choose_commit(Some(""), Some(""), true).is_err());
}

// ------------------------------------------------------------------ the update route

#[test]
fn the_update_app_is_used_first_then_the_script() {
    let home = Path::new("/Users/someone");
    let app = Path::new("/Applications/Update PhotoCraft.app");
    let script = home.join("scripts/craft/update-photocraft.sh");
    assert_eq!(find_route(home, |_| true), Ok(Route::App(app.to_path_buf())));
    assert_eq!(find_route(home, |p| p == script), Ok(Route::Script(script.clone())));
    let own = home.join("Applications/Update PhotoCraft.app");
    assert_eq!(find_route(home, |p| p == own || p == script), Ok(Route::App(own)));
    assert!(find_route(home, |_| false).is_err());
}

#[test]
fn the_update_commands_outlive_photocraft() {
    let home = Path::new("/Users/someone");
    let args = |c: &std::process::Command| c.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
    let app = route::command(&Route::App(PathBuf::from("/Applications/Update PhotoCraft.app")), home);
    assert_eq!(app.get_program(), "/usr/bin/open");
    assert_eq!(args(&app), ["/Applications/Update PhotoCraft.app"]);
    let script = route::command(&Route::Script(home.join("scripts/craft/update-photocraft.sh")), home);
    assert_eq!(script.get_program(), "/bin/sh");
    let a = args(&script);
    assert_eq!(a[0], "-c");
    assert!(a[1].ends_with('&') && a[1].contains("nohup") && a[1].contains("open -a PhotoCraft"), "{}", a[1]);
    assert_eq!(a[2], "/Users/someone/scripts/craft/update-photocraft.sh");
    assert_eq!(a[3], "/Users/someone/Library/Logs/update-photocraft.log");
}

// ------------------------------------------------------------------ the app

fn commit_known() -> Result<String, String> {
    Ok(OWN.into())
}

fn commit_unknown() -> Result<String, String> {
    Err("This build does not know which commit it came from.".into())
}

fn update_unused() -> Result<(), String> {
    Err("no update in this test".into())
}

fn app_with(fetch: std::sync::Arc<Fake>, commit: fn() -> Result<String, String>, update: fn() -> Result<(), String>) -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.fork_update_rt = Runtime::with_parts(Parts { fetch, commit, update });
    app
}

/// Frames until the check finishes.
fn settle(app: &mut PhotocraftApp) {
    let ctx = egui::Context::default();
    for _ in 0..500 {
        super::tick(app, &ctx);
        if app.fork_update_rt.state != State::Checking {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the check did not finish");
}

#[test]
fn the_start_check_is_quiet_unless_a_new_build_is_ready() {
    for (status, state, open) in [("identical", State::UpToDate, false), ("ahead", State::Ready, true)] {
        let mut app = app_with(Fake::built(status), commit_known, update_unused);
        settle(&mut app);
        assert_eq!((app.fork_update_rt.state.clone(), app.fork_update_rt.open), (state, open), "{status}");
    }
    let mut app = app_with(Fake::new(Ok(Reply::Body(release_json("commit: updating"))), Err("x".into())), commit_known, update_unused);
    settle(&mut app);
    assert_eq!((app.fork_update_rt.state.clone(), app.fork_update_rt.open), (State::BeingMade, false));
    // offline: the failure shows in Settings, the window stays shut
    let mut app = app_with(Fake::new(Err("Couldn't reach GitHub.".into()), Err("x".into())), commit_known, update_unused);
    settle(&mut app);
    assert_eq!((app.fork_update_rt.state.clone(), app.fork_update_rt.open), (State::Failed("Couldn't reach GitHub.".into()), false));
}

#[test]
fn the_start_check_runs_once_and_only_when_asked_for() {
    let fake = Fake::built("identical");
    let mut app = app_with(fake.clone(), commit_known, update_unused);
    settle(&mut app);
    settle(&mut app);
    settle(&mut app);
    assert_eq!(fake.asked().len(), 2, "one check (release, compare) however many frames");
    let fake = Fake::built("ahead");
    let mut off = app_with(fake.clone(), commit_known, update_unused);
    off.session.edit_prefs(|p| p.general.automatically_check_updates = false);
    settle(&mut off);
    assert!(fake.asked().is_empty());
    assert_eq!(off.fork_update_rt.state, State::Idle);
}

#[test]
fn an_unknown_commit_fails_without_asking_github() {
    let fake = Fake::built("ahead");
    let mut app = app_with(fake.clone(), commit_unknown, update_unused);
    settle(&mut app);
    assert!(matches!(app.fork_update_rt.state, State::Failed(_)));
    assert!(fake.asked().is_empty());
}

#[test]
fn help_menu_check_replaces_the_official_one_and_always_opens_the_window() {
    let fake = Fake::new(Err("Couldn't reach GitHub.".into()), Err("x".into()));
    let mut app = app_with(fake, commit_known, update_unused);
    // The official check must not start, whatever the engine's own service would do.
    let official = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = official.clone();
    app.session.updates.check = Some(Box::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        Err("official".into())
    }));
    let ctx = egui::Context::default();
    let reply = crate::menus::invoke(&mut app, &ctx, "help.checkForUpdates", json!({})).unwrap();
    assert_eq!(reply["status"], "checking");
    assert!(app.fork_update_rt.open);
    settle(&mut app);
    assert!(matches!(app.fork_update_rt.state, State::Failed(_)));
    assert!(app.fork_update_rt.open, "a failed manual check shows its message");
    assert_eq!(official.load(Ordering::SeqCst), 0, "the official update check never runs");
    assert_eq!(app.session.updates.status, photocraft_engine::updates_cmds::Status::Idle);
    assert!(crate::menus::invoke(&mut app, &ctx, "help.checkForUpdates", json!({"x": 1})).is_err());
}

#[test]
fn a_check_from_settings_shows_beside_the_button_without_a_window() {
    let mut app = app_with(Fake::built("ahead"), commit_known, update_unused);
    app.session.edit_prefs(|p| p.general.automatically_check_updates = false);
    super::start(&mut app, Origin::Settings);
    assert_eq!(app.fork_update_rt.state, State::Checking);
    settle(&mut app);
    assert_eq!(app.fork_update_rt.state, State::Ready);
    assert!(!app.fork_update_rt.open);
}

#[test]
fn a_second_request_while_checking_starts_nothing() {
    let fake = Fake::built("identical");
    let mut app = app_with(fake.clone(), commit_known, update_unused);
    // The launch check would start on the first frame once this one has finished, a second check
    // that depends on how fast the fake answers.
    app.session.edit_prefs(|p| p.general.automatically_check_updates = false);
    super::start(&mut app, Origin::Menu);
    super::start(&mut app, Origin::Menu);
    super::start(&mut app, Origin::Settings);
    settle(&mut app);
    assert_eq!(fake.asked().len(), 2);
}

static STARTED: AtomicUsize = AtomicUsize::new(0);

fn update_started() -> Result<(), String> {
    STARTED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[test]
fn update_starts_the_route_only_when_a_new_build_is_ready() {
    let mut app = app_with(Fake::built("ahead"), commit_known, update_started);
    super::update(&mut app);
    assert_eq!(STARTED.load(Ordering::SeqCst), 0, "nothing to install before a check");
    settle(&mut app);
    assert_eq!(app.fork_update_rt.state, State::Ready);
    super::update(&mut app);
    assert_eq!(app.fork_update_rt.state, State::Updating);
    super::update(&mut app);
    assert_eq!(STARTED.load(Ordering::SeqCst), 1, "once");
}

#[test]
fn a_route_that_cannot_start_is_a_message() {
    let mut app = app_with(Fake::built("ahead"), commit_known, update_unused);
    settle(&mut app);
    super::update(&mut app);
    assert_eq!(app.fork_update_rt.state, State::Failed("no update in this test".into()));
}

#[test]
fn the_words_shown_for_each_state() {
    assert_eq!(State::Idle.label(), "");
    assert_eq!(State::UpToDate.label(), "Up to Date");
    assert_eq!(State::Ready.label(), "A New Build Is Ready");
    assert_eq!(State::BeingMade.label(), "A New Build Is Being Made");
    assert_eq!(State::Failed("Offline.".into()).label(), "Offline.");
    assert!(release::REPO.starts_with("techneissa/"));
}

#[test]
fn the_official_check_is_off_in_the_app() {
    const { assert!(super::OFFICIAL_CHECK_OFF) };
    assert!(super::ui::row_matches("") && super::ui::row_matches("updates") && !super::ui::row_matches("zoom"));
}
