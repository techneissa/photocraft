# This fork: PhotoCraft plus techneissa's own features

The branch **`custom`** is the official PhotoCraft ([storytold/photocraft](https://github.com/storytold/photocraft))
plus the features listed below. The installed app is built from `custom`.

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

## GitHub builds the app

The workflow `.github/workflows/fork-build.yml` (Actions ▸ Fork build) runs daily, on every push to `custom`, and
by hand. It merges the official update into `custom` the same way the script does, builds the Mac app with the
fonts of the official release (craft-fonts at `release.yml`'s `craft-fonts-ref`), and publishes it as the release
[`custom-build`](https://github.com/techneissa/photocraft/releases/tag/custom-build) (its notes name the commit).
The update script downloads it (above). A run that hits a merge clash fails and changes nothing; GitHub emails the
failure. The run pushes its merge with the secret `FORK_PUSH_TOKEN`, the fine-grained token "BuildCraft forks"
(Contents and Workflows: read and write): GitHub's own token may not push an official update that changes a
workflow file. Without the secret it uses GitHub's own token.

The official project's own workflows (CI, Docs, FreeBSD, Nix, Packaging lint, Perf nightly, Release, SonarCloud,
Windows arm64) are switched off in this fork: the official project runs them. A workflow the official project
adds later starts switched on; switch it off in Actions unless it is wanted here.

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
   - after each change: only the tests of the code it touched (`cargo test -p <crate> --lib <filter>`,
     `cargo clippy -p <crate>`);
   - once per round of work: `cargo clippy -p <crates> --all-targets -- -D warnings`, `cargo xtask layers`,
     `cargo xtask wasm` (when L0–L6 changed), `cargo xtask parity` (when commands were added; commit
     `docs/parity-checklist.md`);
   - once per merge into `custom`: the full `cargo xtask ci`.

   This fork reads `AGENTS.md`'s "before you finish a task" list as once per round, and the full gate as once
   per merge (techneissa, 2026-10-28).
4. `git switch custom && git merge --no-ff feat/<name>`, then `git push mine custom feat/<name>`.
5. Add a row to the table below, and run `update-photocraft.sh` to install it.

## The features

| Feature | Branch | Own files | Hooks in official files | Tests |
|---|---|---|---|---|

Every feature also has a row in `docs/parity-checklist.md` when it adds commands; clashes there are fixed by the
update script (both sides kept).

The tests of every feature, after a merge:

```bash
# (none yet)
```

## Known failures on this Mac (not caused by the fork)

None recorded yet.
