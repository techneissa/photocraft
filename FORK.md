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
| Open as Layers: a `.pcstack` file (from LightCraft's Edit In ▸ Open as Layers in PhotoCraft) opens its photos as the layers of one new document, top layer first, smaller ones centred, all converted to the top file's mode, depth and profile; also the command `file.openAsLayers`. Runs as a background job (Esc cancels). At most 256 files and 2^29 pixels in all (a file past that is skipped and named). Interface: "Open as Layers interface" below | `feat/open-as-layers` | `crates/engine/src/cmd/file_cmds/open_as_layers.rs` and `open_as_layers/tests.rs`, `crates/ui-egui/src/open_as_layers.rs` and `open_as_layers/tests.rs` | engine `cmd/file_cmds.rs` (module), `cmd.rs` (`specs()` in the command list); ui-egui `lib.rs` (module), `file_open.rs` (3 lines in `open_path`; `opens_in_place` replaces `is_preset_file` at its 3 call sites so a dropped `.pcstack` is never placed; one test at the end of `file_open/tests.rs`), `jobs_ui.rs` (6 lines in `on_event`: the finished job's notice); `packaging/macos/Info.plist.in` (a `pcstack` document type) | `open_as_layers` in engine and ui-egui |
| Free Transform, Warp, Puppet Warp and Perspective Warp show the moving layer in its place with its blend mode, opacity, fill, mask, clipping and style (a Screen layer stays Screen while you scale it); the Interpolation preference (Nearest) applies to the Free Transform preview too. This is upstream PR #8216 taken early (a real merge of its commit `79370e6fc`, ported onto main's split files) plus fork fixes. **When upstream merges #8216:** if by merge commit, git sees it as already merged; if squashed, the merge conflicts in the files below: keep upstream's version where it matches and do not cherry-pick #8216 again. Hooks are larger than the fork usually allows because this is upstream's own code | `feat/transform-live-blend` | `crates/compose/src/live.rs`, `crates/gpu/src/live.rs`, `crates/gpu/src/live.wgsl`, `crates/ui-egui/src/tools/transform_live.rs`, `crates/gpu/tests/it/live_transform.rs` | gpu `lib.rs`, `kit.rs`, `setup.rs`, `encode.rs`, `cells.rs` (also a `get` for a pool slot), `plan.rs`, `plan/types.rs`, `plan/planner.rs`; ui-egui `canvas/display.rs`, `navigator.rs`, `textures.rs`, `paint.rs`, `gpu_canvas.rs`, `gpu_canvas/resources.rs`, `tools/transform_tool.rs` + `begin.rs` + `draw.rs` + `tests.rs`, `puppet_ui.rs`, `perspective_ui.rs`, `distort_ui.rs`, `lib.rs`, `canvas/tests.rs`; deleted `transform_fx.rs` and `transform_fx_tests.rs` (as the PR does); `docs/ui-parity.md` | `live_transform` (gpu), `live` (compose), ui-egui `transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview` |
| Layer ▸ Double Exposure (also a Layers-panel button next to Add Mask, hidden with Preferences ▸ Interface ▸ "Double Exposure button"): the top visible, unlocked pixel layer (or the selected one) gets Screen and a Reveal All mask in one undo step, the mask is targeted (also after Redo), and a click by the user switches to a soft black brush that is temporary (the saved Brush comes back). Scripts and the control channel get the layer setup only. Command `layer.doubleExposure` (`mode`: `screen` or `lighten`) | `feat/double-exposure` | `crates/engine/src/cmd/double_exposure.rs` and `double_exposure/tests.rs`, `crates/ui-egui/src/double_exposure.rs` and `double_exposure/tests.rs` | engine `cmd.rs` (module, `specs()`), `cmd/channel_cmds.rs` + `channel_cmds/targeting.rs` (`target` made `pub(crate)`), `session/history.rs` (one call in `redo`, matched on the step label "Double Exposure"), `prefs.rs` (`interface.showDoubleExposureButton`); ui-egui `lib.rs`, `menus/invoke.rs` (3 lines), `menus/items.rs` (`PLACE_AFTER` row), `menus/render.rs` (tooltip), `panels/layers.rs` (the button, 8 lines), `prefs/prefs_ui.rs`, `canvas/paint_mouse.rs` (1 line in `sync_tool_brush`), `icon_data.rs` (icon); `ATTRIBUTION.md` (icon row) | `double_exposure` in engine and ui-egui |
| Blend mode lists as tall as the window allows (no fixed 14-row cap, upstream issue #7261; upstream PR #7622 is stale and not taken), and Preferences ▸ Interface ▸ "Scroll Wheel Changes Blend Mode" (off by default: the wheel over a blend box scrolls instead of stepping the mode; on = Photoshop's way) | `feat/blend-list` | `crates/ui-egui/src/blend_wheel.rs` and `blend_wheel/tests.rs` | `widgets/dropdown.rs` (`list_height()`, one `let list` per call site), engine `prefs.rs` (`interface.scroll_wheel_changes_blend_mode`), ui-egui `lib.rs`, `prefs/prefs_ui/save.rs` (1 line), `panels/layers.rs` (2), `panels/options_bar.rs` (1), `panels/brush_controls.rs` (2): `dropdown_wheel_hovered` → `blend_wheel::dropdown`; official tests `blend_dropdown_wheel_tests.rs`, `options_bar_mode_wheel_tests.rs` (helpers `pub(crate)`, preference on) | `blend_wheel`, `blend_dropdown`, `options_bar_mode_wheel` |
| Type to search in dropdown lists: typing in an open list narrows it (label starts, then word starts, then substrings), ↑/↓, ↩ or a click picks, Esc clears then closes, "No match"; Space and other held tool keys stay out while a list is open. Every dropdown built by `widgets/dropdown.rs`; not: crop ratio, kerning, JPEG quality preset, gradient map preset, shape stroke editor, credits sort (fonts have their own search) | `feat/dropdown-search` (on `feat/blend-list`) | `crates/ui-egui/src/widgets/dropdown_search.rs` and `dropdown_search/tests.rs` | `widgets.rs` (module), `widgets/dropdown.rs` (`before(...)`, `.show_ui(` → `.search(`, the pick block replaces the `type_ahead::keys` lines, `row` / `no_match` hooks; 19+/12−), `input/hold_keys.rs` (3 early returns while a list is open), `examples/snapshot.rs` (`--type`); official tests `type_ahead.rs`, `blend_dropdown_keys_tests.rs` (same names, assertions for narrowing instead of the letter jump) | `dropdown_search`, `blend_dropdown`, `type_ahead`, `hold_keys` |

Every feature also has a row in `docs/parity-checklist.md` when it adds commands; clashes there are fixed by the
update script (both sides kept).

The tests of every feature, after a merge:

```bash
cargo test -p photocraft-engine --lib -- open_as_layers double_exposure
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
cargo test -p photocraft-compose --lib live
cargo test -p photocraft-gpu --test it live_transform
```

## Open as Layers interface (for LightCraft)

LightCraft's **Edit In ▸ Open as Layers in PhotoCraft** hands several photos to PhotoCraft, which opens them as
the layers of one new document (double and multiple exposures). Agreed 2026-10-28; version 1.

1. LightCraft writes the photos as files (as for Edit in PhotoCraft), then a small UTF-8 JSON file with the
   extension **`.pcstack`** next to them:

   ```json
   {
     "kind": "photocraft-open-as-layers",
     "version": 1,
     "name": "DSC01234 + 2",
     "files": ["/abs/path/top.psd", "/abs/path/middle.tif", "/abs/path/bottom.jpg"]
   }
   ```

   - `files`: absolute paths, **top layer first**, 1 to 256 of them, in any format PhotoCraft opens.
   - `name`: optional; the new document's name (default: the first file's name without its extension).
2. LightCraft opens the `.pcstack` file the way it opens a file today: `open -a <PhotoCraft.app> <file.pcstack>` on
   macOS, `photocraft <file.pcstack>` on Windows and Linux. This works whether PhotoCraft is running or not.
3. PhotoCraft makes one new, unsaved document:
   - canvas: the largest width × the largest height of the files; each smaller image centred on it;
   - one layer per file, named after the file without its extension, in the order given (first file on top);
   - colour mode, bit depth, resolution and profile of the top file (the others are converted to it).
4. A file that is missing or can't be read is skipped and named in a notice; no file left gives an error notice
   and no document. A malformed `.pcstack` (wrong `kind`, newer `version`, bad JSON, no files) gives an error
   notice. Nothing crashes.
5. PhotoCraft never deletes or changes the `.pcstack` file or the photos; the folder belongs to LightCraft.
6. The same action is the command `file.openAsLayers` (`{"paths": [...], "name": "..."}`), so the control
   channel, CLI and MCP can use it too.
7. LightCraft can tell whether the installed PhotoCraft has this: its `Info.plist` lists `pcstack` among
   `CFBundleDocumentTypes`. When it doesn't (the official PhotoCraft), LightCraft opens the files one by one,
   as today.

## Known failures on this Mac (not caused by the fork)

Checked 2026-10-28 on official `main` (b8c3f3aa8) as well: the same failures, none in a fork file.

- `clippy -D warnings`: this Mac's clippy (1.96) is newer than upstream's and flags official code:
  `nonminimal_bool` (`codecs/src/codecs/via_image.rs`), `overly_complex_bool_expr` (`text/src/psd_path.rs`),
  `drop_non_drop` (`io/src/psd_export/document.rs`), cognitive complexity (`apps/tests/mac_menu_appkit.rs`), `manual_range_contains` (`engine/src/canvas_geom/tests.rs`).
- 28 `photocraft-ui-egui` tests (`i18n::live_tests`, `camera_raw_localization`, `type_tool`, `layer_row_ui`,
  `brush_panel`, `vector_ui`, `input::hold_keys`): `No glyph for …`; this Mac has no CJK font installed.
- `cargo xtask scorecard --check`: the total of settings ("12 of N") is higher than the committed 181: the fork's own settings (Double Exposure button, Scroll Wheel Changes Blend Mode) add to it; `docs/scorecard.md` is left as upstream's so updates don't clash on it. For the same reason the xtask test `scorecard::tests::staleness_check_and_determinism` fails ("docs/scorecard.md is stale").
- `cargo xtask ci` stops at its first failing step; run the rest by hand (`cmd_ci` in `xtask/src/main.rs` lists them).
