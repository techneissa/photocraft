//! Which commit this app was built from, and the existing update route that installs a newer
//! build. The route is techneissa's own: the "Update PhotoCraft" app, else `update-photocraft.sh`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::release::is_commit;

/// The commit this app was built from.
///
/// First the commit compiled in: `packaging/env.sh` sets `PHOTOCRAFT_BUILD_SHA` to `git rev-parse
/// HEAD` for every `packaging/macos/package.sh` run, which is how both the GitHub build (the
/// commit it names in the `custom-build` notes) and `update-craft.sh`'s build on the Mac (the
/// commit it stamps) make the app. Only when that is missing (an app packaged without it) does the
/// stamp the update script keeps for the installed app answer, and only for an app running from
/// an installed `.app`, so a `cargo run` build never claims the installed app's commit.
pub fn own_commit() -> Result<String, String> {
    let stamp = stamp_path().and_then(|path| std::fs::read_to_string(path).ok());
    let exe = std::env::current_exe().ok();
    choose_commit(photocraft_engine::build_info::git_sha(), stamp.as_deref(), exe.as_deref().is_some_and(in_app_bundle))
}

/// The stamp the update script writes after an install (`stamp()` in `update-craft.sh`).
fn stamp_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/Caches/craft-update/photocraft.commit"))
}

fn in_app_bundle(exe: &Path) -> bool {
    exe.to_string_lossy().contains(".app/Contents/MacOS/")
}

/// The compiled-in commit, else the stamp (for an installed app only), else an error.
pub fn choose_commit(compiled: Option<&str>, stamp: Option<&str>, installed_app: bool) -> Result<String, String> {
    let valid = |s: &str| Some(s.trim().to_ascii_lowercase()).filter(|s| is_commit(s));
    compiled
        .and_then(valid)
        .or_else(|| stamp.filter(|_| installed_app).and_then(valid))
        .ok_or_else(|| "This build does not know which commit it came from.".to_string())
}

/// How an update is started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// The "Update PhotoCraft" app: shows progress, installs, opens PhotoCraft again.
    App(PathBuf),
    /// `update-photocraft.sh`, run in the background and followed by opening PhotoCraft.
    Script(PathBuf),
}

/// The update route this Mac has, the app first. `exists` is the file check (faked in tests).
pub fn find_route(home: &Path, exists: impl Fn(&Path) -> bool) -> Result<Route, String> {
    let app = Path::new("/Applications/Update PhotoCraft.app");
    if exists(app) {
        return Ok(Route::App(app.to_path_buf()));
    }
    let home_app = home.join("Applications/Update PhotoCraft.app");
    if exists(&home_app) {
        return Ok(Route::App(home_app));
    }
    let script = home.join("scripts/craft/update-photocraft.sh");
    if exists(&script) {
        return Ok(Route::Script(script));
    }
    Err("This Mac has no Update PhotoCraft app to install the build.".into())
}

/// The command that starts `route`, not yet run. Both routes outlive PhotoCraft: `open` hands the
/// app to macOS; the script runs in a shell that has already returned (a subshell started in the
/// background with its output in the update log), so quitting PhotoCraft does not stop it.
pub fn command(route: &Route, home: &Path) -> Command {
    match route {
        Route::App(app) => {
            let mut c = Command::new("/usr/bin/open");
            c.arg(app);
            c
        }
        Route::Script(script) => {
            let mut c = Command::new("/bin/sh");
            // $0 is the script, $1 the log; the script quits PhotoCraft itself just before it installs.
            c.arg("-c")
                .arg(r#"( /usr/bin/nohup /bin/bash "$0" >"$1" 2>&1 </dev/null; /usr/bin/open -a PhotoCraft ) >/dev/null 2>&1 </dev/null &"#)
                .arg(script)
                .arg(home.join("Library/Logs/update-photocraft.log"));
            c
        }
    }
}

/// Start the update and return at once; the update continues after PhotoCraft quits.
pub fn start_update() -> Result<(), String> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("This Mac has no home folder to find the update in.")?;
    let route = find_route(&home, Path::exists)?;
    let mut run = command(&route, &home);
    run.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = run.spawn().map_err(|e| format!("Could not start the update: {e}."))?;
    // `open` and the outer shell finish at once.
    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err("The update could not be started.".into()),
        Err(e) => Err(format!("Could not start the update: {e}.")),
    }
}
