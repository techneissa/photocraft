//! Release checks through `craft-update`, the library every Craft App shares. Only public release
//! metadata is requested.

use photocraft_engine::updates_cmds::{CheckFn, CheckResult, Release};

pub fn service() -> CheckFn {
    Box::new(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("update check".into())
            .spawn(move || {
                let result = check();
                if let Err(e) = &result {
                    log::warn!("Update check: {e}");
                }
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Could not start update check: {e}"))?;
        Ok(rx)
    })
}

fn check() -> CheckResult {
    // This fork updates from its own builds (fork/fork-update-check.md): the official releases are never asked.
    if photocraft_ui_egui::fork_update::OFFICIAL_CHECK_OFF {
        return Err("This build updates from its own builds, not the official releases.".into());
    }
    let http = craft_update::Http::new("PhotoCraft/Update")?;
    let found = craft_update::check_for_update(&photocraft_ui_egui::UPDATE_APP, &http, photocraft_engine::build_info::VERSION)?;
    Ok(found.map(|release| Release { version: release.tag.trim_start_matches(['v', 'V']).to_string(), url: release.html_url }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_official_release_check_is_never_asked_in_this_fork() {
        // Answers before any network call: the engine's service reports a short message.
        const { assert!(photocraft_ui_egui::fork_update::OFFICIAL_CHECK_OFF) };
        let message = check().unwrap_err();
        assert!(message.contains("not the official"), "{message}");
    }
}
