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
   - the full suite runs on GitHub, not on this Mac: a push to `custom` starts Fork CI (`fork-ci.yml`), which runs the
     official CI (`cargo xtask ci`) on `custom`. Check it with `gh run list -R techneissa/photocraft -w CI -b custom`
     and fix a failure in the next round.

   This fork reads `AGENTS.md`'s "before you finish a task" list as the above: no `cargo xtask ci` on this Mac
   (techneissa, 2026-10-29: the same tests ran several times per feature and loaded the Mac). The official CI runs on
   Linux, so macOS-only tests (`macvision`, the real-Vision tests) run only in a lane's own filtered tests.
4. `git switch custom && git merge --no-ff feat/<name>`, then `git push mine custom feat/<name>`.
5. Write `fork/<slug>.md` (headings as in the other files), add its line below and its tests to `fork/tests.sh`; then run `update-photocraft.sh`. FORK.md never holds a feature's details.

## The features

- [Open as Layers](fork/open-as-layers.md): opens the photos of a LightCraft `.pcstack` file as the layers of one new document. Branch `feat/open-as-layers`.
- [Transform live blend](fork/transform-live-blend.md): Free Transform, Warp, Puppet Warp and Perspective Warp show the moving layer with its blend mode, opacity and mask. Branch `feat/transform-live-blend`.
- [Double Exposure](fork/double-exposure.md): Layer ▸ Double Exposure sets the top layer to Screen with a Reveal All mask in one undo step. Branch `feat/double-exposure`.
- [Blend list height and wheel](fork/blend-list.md): blend mode lists are as tall as the window, and the wheel scrolls them by default (a preference makes it step the mode). Branch `feat/blend-list`.
- [Dropdown search](fork/dropdown-search.md): typing in an open dropdown list narrows it. Branch `feat/dropdown-search`.
- [Select Subject on macOS](fork/select-subject-macos.md): Select › Subject uses macOS's own subject cutout (Apple Vision). Branch `feat/select-subject-macos`.
- [AI Select Subject and Remove Subject](fork/select-subject-ai.md): upstream AI models (BiRefNet-lite and LaMa) joined with the macOS cutout under one engine preference. Branch `feat/select-subject-ai`.
- [Shared models folder](fork/shared-models.md): the AI models live in one folder shared by the Craft apps. Branch `feat/shared-models`.
- [Upstream select and mask fixes](fork/upstream-select-mask.md): four upstream fixes taken early, for mask filters, mask copy and paste, the Polygonal Lasso and Select › Modify previews. Branch `feat/upstream-select-mask`.
- [Copy Layers to Document](fork/duplicate-to-other-doc.md): refuses a destination with a running background job; two upstream PRs taken early. Branch `fix/duplicate-to-other-doc`.
- [PSD font scan](fork/psd-open-font-scan.md): PSD files with a big font collection open without the long font scan. Branch `fix/psd-open-font-scan`.
- [Top bar items](fork/topbar-items.md): show or hide the top bar's Logo, Document Title, Workspace, Search and Appearance Mode. Branch `feat/topbar-items`.
- [Assign Shortcut and command palette](fork/assign-shortcut-palette.md): Assign Shortcut… on right-click, and the command palette on ⌘P as well as ⌘F. Branch `feat/assign-shortcut-palette`.
- [Context menu editor](fork/context-menu-editor.md): arranges the right-click menus: hide, reorder and add commands, with presets and a backup file. Branch `feat/context-menu-editor`.
- [Keyboard shortcut upgrade](fork/shortcut-upgrade.md): chords, When conditions, a clash icon and export/import icons in Keyboard Shortcuts. Branch `feat/shortcut-upgrade`.
- [Crop W×H](fork/pr-9213.md): crop with Delete Cropped Pixels off no longer aborts the app (upstream PR #9213, taken early). Branch `pr-9213`.
- [Undo and redo stress test](fork/undo-redo-stress.md): hundreds of random edits, undos and redos must restore exact states. Branch `test/undo-redo-stress`.
- [Discord button hidden](fork/hide-discord-button.md): no Join us on Discord button on the start screen or in About. Branch `feat/hide-discord-button`.
- [⌘, opens Settings](fork/settings-cmd-comma.md): ⌘, opens Settings, as in every Mac app; Hide Layers has no default key. Branch `feat/settings-cmd-comma`.
- [⌘, or Esc closes Preferences](fork/settings-close-keys.md): closes Preferences, asking to save when there are unsaved changes. Branch `feat/settings-close-keys`.
- [Preferences scroll width](fork/prefs-scroll-width.md): Preferences lists fill the window, the Toolbar tab fits 1–4 columns, Keyboard Shortcuts keeps its width while searching. Branch `fix/prefs-scroll-width`, `fix/shortcuts-tabs-layout`, `fix/shortcuts-search-width`.
- [Context menus page size](fork/context-menus-resize.md): the Context Menus page keeps the size the Preferences window is dragged to. Branch `fix/context-menus-resize`.
- [Arabic AI Models text](fork/arabic-ai-models.md): Arabic text for the AI Models page. Branch `custom`.
- [Place Photos](fork/place-photos.md): Place Embedded on ⇧⌘P, several photos at once, a Place Photo button in the Layers panel. Branch `feat/place-photos`.

To find which feature owns an official file: `grep -l '<path>' fork/*.md`.

Every feature also has a row in `docs/parity-checklist.md` when it adds commands; clashes there are fixed by the
update script (both sides kept).

The tests of every feature, after a merge: run `fork/tests.sh`.

More: [GitHub builds](fork/github-builds.md), [Open as Layers interface (for LightCraft)](fork/open-as-layers-interface.md), [Known failures on this Mac](fork/known-failures.md).
