//! Reading GitHub's answers about techneissa's own builds: plain functions over text, so the
//! parsing, the compare result and the choice of state are tested with canned JSON.

use serde_json::Value;

use super::github::{Fetch, Reply};

/// The fork whose builds this app updates from, never the official project's releases.
pub const REPO: &str = "techneissa/photocraft";
/// The release holding the newest app GitHub built from `custom`; its notes say `commit: <sha>`
/// (`commit: updating` while a build runs).
pub const RELEASE_TAG: &str = "custom-build";
const API: &str = "https://api.github.com";

/// What the release notes of `custom-build` say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notes {
    /// The commit the published app was built from (lower case hex).
    Commit(String),
    /// A build is running; the app in the release is being swapped.
    Updating,
}

/// How the published build's commit stands against this app's (GitHub's compare `status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    Identical,
    /// The published build has commits this app lacks.
    Ahead,
    /// This app has commits the published build lacks.
    Behind,
    /// Each has commits the other lacks.
    Diverged,
}

/// What a finished check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    /// A newer build is published (its commit).
    Ready(String),
    BeingMade,
}

/// A commit id: 7 to 40 hex digits.
pub fn is_commit(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Do two commit ids (either may be shortened to 7 or more digits) name the same commit?
pub fn same_commit(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    is_commit(&a) && is_commit(&b) && (a.starts_with(&b) || b.starts_with(&a))
}

/// The `commit:` line of the release's notes, from GitHub's JSON for the release.
pub fn parse_notes(release_json: &str) -> Result<Notes, String> {
    let release: Value = serde_json::from_str(release_json).map_err(|_| "GitHub's answer about the build was not readable.".to_string())?;
    let body = release.get("body").and_then(Value::as_str).ok_or("The build has no notes yet.")?;
    let line = body.lines().find_map(|l| l.trim().strip_prefix("commit:")).map(str::trim).ok_or("The build notes name no commit.")?;
    let line = line.to_ascii_lowercase();
    match line.as_str() {
        "updating" => Ok(Notes::Updating),
        "none" | "" => Err("No build has been published yet.".into()),
        s if is_commit(s) => Ok(Notes::Commit(s.to_string())),
        _ => Err("The build notes name no commit.".into()),
    }
}

/// The `status` of GitHub's compare answer.
pub fn parse_compare(compare_json: &str) -> Result<Compare, String> {
    let answer: Value = serde_json::from_str(compare_json).map_err(|_| "GitHub's comparison was not readable.".to_string())?;
    match answer.get("status").and_then(Value::as_str) {
        Some("identical") => Ok(Compare::Identical),
        Some("ahead") => Ok(Compare::Ahead),
        Some("behind") => Ok(Compare::Behind),
        Some("diverged") => Ok(Compare::Diverged),
        _ => Err("GitHub's comparison had no result.".into()),
    }
}

/// What to tell the user. A build the app has not got is offered when it is ahead, or when each
/// side has commits the other lacks (the update script merges them). A build older than the app
/// is not an update.
pub fn outcome_of(compare: Compare, built: &str) -> Outcome {
    match compare {
        Compare::Identical | Compare::Behind => Outcome::UpToDate,
        Compare::Ahead | Compare::Diverged => Outcome::Ready(built.to_string()),
    }
}

fn rate_limited(minutes: Option<u64>) -> String {
    match minutes {
        Some(m) if m > 1 => format!("GitHub is limiting requests. Try again in {m} minutes."),
        _ => "GitHub is limiting requests. Try again in a minute.".into(),
    }
}

fn text(reply: Reply, missing: &str) -> Result<String, String> {
    match reply {
        Reply::Body(text) => Ok(text),
        Reply::NotFound => Err(missing.into()),
        Reply::RateLimited { minutes } => Err(rate_limited(minutes)),
    }
}

/// Ask GitHub whether the published build is newer than `own` (this app's commit).
pub fn decide(fetch: &dyn Fetch, own: &str) -> Result<Outcome, String> {
    if !is_commit(own) {
        return Err("This build does not know which commit it came from.".into());
    }
    let release = text(fetch.get(&format!("{API}/repos/{REPO}/releases/tags/{RELEASE_TAG}"))?, "GitHub has no build to update to yet.")?;
    let built = match parse_notes(&release)? {
        Notes::Updating => return Ok(Outcome::BeingMade),
        Notes::Commit(built) => built,
    };
    if same_commit(own, &built) {
        return Ok(Outcome::UpToDate);
    }
    // `per_page=1` keeps the answer small: only `status` is read.
    let compare = text(
        fetch.get(&format!("{API}/repos/{REPO}/compare/{own}...{built}?per_page=1"))?,
        "GitHub does not know this build's commit, so it cannot compare it.",
    )?;
    Ok(outcome_of(parse_compare(&compare)?, &built))
}
