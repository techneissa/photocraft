# Fork update check

## What it does

The app updates itself to techneissa's own builds (the release `custom-build` of techneissa/photocraft, made from `custom`), never to storytold's official release. The official check never runs.

- **Check for Updates** in Help, a **Check for Updates** button in Settings › General with a one-line status beside it, and one check at start (only when Automatically Check for Updates is on).
- States shown: **Up to Date**, **A New Build Is Ready** (with an **Update** button), **A New Build Is Being Made**, or a short message (offline, GitHub's request limit, release notes without a commit, a commit GitHub does not know, no commit in this build, no update app on the Mac).
- At start the check is quiet: the window opens only when a new build is ready (Update / Later). A failure at start shows only in Settings. Help › Check for Updates… always opens its window (Check Again / Close). The Settings button shows its answer beside the button and opens no window.
- **How it decides**: reads the notes of `custom-build` (`commit: <sha>`, or `commit: updating` while a build runs, which is "A New Build Is Being Made"), then asks GitHub's compare API (`compare/<app commit>...<built commit>?per_page=1`, `status`): `ahead` and `diverged` are "A New Build Is Ready" (a diverged pair is what the update script merges), `identical` and `behind` are "Up to Date" (an app newer than the build has nothing to install). The same commit needs no compare request.
- **The app's own commit** is the one compiled in: `packaging/env.sh` sets `PHOTOCRAFT_BUILD_SHA` to `git rev-parse HEAD` for every `packaging/macos/package.sh` run, and both the GitHub build (`fork-build.yml`, which names that HEAD in the release notes) and `update-craft.sh`'s build on the Mac (which stamps that HEAD) go through it, so no build script or workflow change was needed. Only when it is missing or `unknown` does the stamp `~/Library/Caches/craft-update/photocraft.commit` answer, and only for an app running from an installed `.app` (a `cargo run` build never claims the installed app's commit). No commit at all is the message "This build does not know which commit it came from."
- **Update** starts the existing route and returns at once, so the update survives PhotoCraft quitting: `/Applications/Update PhotoCraft.app` (via `open`; it runs `update-photocraft.sh`, which quits PhotoCraft just before the install and the app opens it again), else `~/scripts/craft/update-photocraft.sh` in the background (`nohup`, log `~/Library/Logs/update-photocraft.log`) followed by `open -a PhotoCraft`. The update scripts are not edited.
- The official check is off in three places: `updates_ui` hands `check`, `tick` and `show` to this module (`OFFICIAL_CHECK_OFF`), and the app's own release service answers "This build updates from its own builds, not the official releases." without a network call, so even the engine command `help.checkForUpdates` from automation reaches nobody.
- Only public data is asked (no token, user agent `PhotoCraft/Update`). The HTTP sits behind the `Fetch` trait; tests fake it with canned JSON.
- Native only (`cfg(not(target_arch = "wasm32"))`).

## Branch

`feat/fork-update-check` (from `custom`)

## Own files

ui-egui `fork_update.rs` (state, check, update), `fork_update/release.rs` (reading the notes and the compare answer, the decision), `fork_update/github.rs` (the `Fetch` trait and the real reader), `fork_update/route.rs` (the app's commit, the update route), `fork_update/ui.rs` (window, Settings row), `fork_update/tests.rs`, `fork_update/snapshots.rs` (opt-in picture: `PHOTOCRAFT_UPDATE_SNAPSHOTS` folder)

## Hooks in official files

ui-egui `lib.rs` (mod line, the field `fork_update_rt`), `app.rs` (its initial value), `updates_ui.rs` (the three hand-overs at the top of `check`, `tick`, `show`), `prefs/prefs_ui/fields.rs` (the Settings row in General); app `apps/photocraft/src/updates.rs` (the official service refuses, + test)

## Tests

```bash
cargo test -p photocraft-ui-egui --lib -- fork_update updates_ui
```
