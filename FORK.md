# This fork: PhotoCraft plus techneissa's own features

The branch **`custom`** is the official PhotoCraft ([storytold/photocraft](https://github.com/storytold/photocraft))
plus the features listed below. The installed app is built from `custom`.

**Mac only.** techneissa uses this fork on macOS only. A feature or fix here must work on the Mac; Linux,
Windows and the web build need not work (this relaxes `AGENTS.md`'s "Never break wasm" for fork code). Fork code
that cannot build elsewhere goes behind `cfg(target_os = "macos")`, so the official code still builds there and
Fork CI (Linux) keeps checking the official changes.

## Update the app

1. In Terminal: `~/Documents/workspace/gitClones/update-photocraft.sh` (`update-craft-apps.sh` updates every
   ArtCraft app).
2. It downloads the newest app GitHub built (below; seconds), when that build has everything the Mac's `custom`
   has. Official changes made since that build come with the next one (daily at 08:17, and after every push).
3. Otherwise, or with `update-photocraft.sh --latest`, it gets the official update, merges it into `custom` and
   backs `custom` up to this fork. Then it installs the app: a download of seconds when GitHub has already built
   that version, else a build on the Mac (1 to 10 minutes).
4. If it stops with "the update touches the same code as your edits", nothing was changed. Ask an agent to
   "update PhotoCraft" (it follows the `craftupdate` skill).

The script is `update-craft.sh` in the same folder, shared by every ArtCraft app; it keeps your edits in any
app whose repo has a `custom` branch.

GitHub builds the app from `custom` (Fork build) and runs the official CI on it (Fork CI): [fork/github-builds.md](fork/github-builds.md).

## Where things are

| Name | What it is |
|---|---|
| `origin` | the official repo, where updates come from (never pushed to) |
| `mine` | this fork, [techneissa/photocraft](https://github.com/techneissa/photocraft), the backup |
| `main` | a plain copy of the official `main`, kept in step by the update script |
| `custom` | the official code plus every feature below: the app is built from it (the default branch here) |
| `custom-build` | the release holding the app GitHub built from `custom` |
| `feat/…`, `fix/…` | one feature each, based on the official code, kept for reference |

Local folder: `~/Documents/workspace/gitClones/photocraft`. Git remembers how a merge conflict was fixed
(`rerere`) and fixes it the same way the next time.

## Add a feature

1. `git fetch origin && git switch -c feat/<name> origin/main`
2. Put the code in new files; touch existing files only with small hooks (a `mod` line, a menu entry, a match arm,
   a state field). Never reformat, rename or move official code. This is what keeps updates conflict-free.
3. Add tests. Check in batches:
   - after each change, and once more at the end of the round: only the tests of the code it touched
     (`cargo test -p <crate> --lib <filter>`), `cargo clippy -p <crate> --all-targets -- -D warnings` for the crates it
     touched (the known official lints below excepted), and `cargo fmt --all --check`;
   - the full suite runs on GitHub, not on this Mac: Fork CI (`fork-ci.yml`, `fork/ci.sh`: the steps of
     `cargo xtask ci` on Linux) runs on every pull request to `custom` and every push to it, and skips the tests in
     `fork/known-failures.txt`, so a red run is a new failure. Check it with
     `gh run list -R techneissa/photocraft -w "Fork CI"` and fix a failure in the next round.

   This fork reads `AGENTS.md`'s "before you finish a task" list as the above: no `cargo xtask ci` on this Mac
   (techneissa, 2026-10-29: the same tests ran several times per feature and loaded the Mac). Fork CI runs on
   Linux, so macOS-only tests (`macvision`, the real-Vision tests) run only in a lane's own filtered tests.
4. Write `fork/<slug>.md` (headings as in the other files; `fork/tests.sh` runs the commands in its `## Tests`
   block) and add its line at the end of `fork/features.md`. Leave FORK.md, `fork/tests.sh`, the generated
   `docs/parity-checklist.md` and `docs/scorecard.md`, and the official parity docs (`docs/gaps.md`,
   `docs/*-parity.md`, `ROADMAP.md`) alone; then two feature branches never clash on shared notes.
5. `git push mine feat/<name>` and open a pull request to `custom`; Fork CI checks it. Merge it (GitHub's button,
   or `git switch custom && git merge --no-ff feat/<name> && git push mine custom`). If the feature added
   commands, run `cargo xtask parity` on `custom` and commit `docs/parity-checklist.md`. Then run
   `update-photocraft.sh`.

At most 3 feature lanes build at a time on this Mac (techneissa, 2026-11-02): it also builds the other Craft apps.
Every worktree shares one compiler cache (sccache, set in `~/.cargo/config.toml`, never in the repo's
`.cargo/config.toml`), so a new worktree compiles only what changed.

## The features

[fork/features.md](fork/features.md): one line per feature.

To find which feature owns an official file: `grep -l '<path>' fork/*.md`.

`docs/parity-checklist.md` is regenerated on `custom` after a merge (step 5); clashes there with an official
update are fixed by the update script (both sides kept).

The tests of every feature, after a merge: run `fork/tests.sh`.

More: [GitHub builds](fork/github-builds.md), [Open as Layers interface (for LightCraft)](fork/open-as-layers-interface.md), [Known failures](fork/known-failures.md).
