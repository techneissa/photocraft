# Home Recent Grid

## What it does

Home is the craft framework's start screen (New / Open and Home / Learn in the left rail, Recent on the right, a switch between a grid of cards and a list, a sort). The fork adds: Preferences › Interface › Recent files on Home (1–100, default 12) sets how many files show (File › Open Recent keeps the whole list); one dim line under each name (grid) or at the end of its row (list) gives the image size in pixels and when the file was last opened in PhotoCraft (`6000 × 4000 · 2 h ago`, `Yesterday`, then `3 Nov`; the size shows once the preview is decoded, an open document's is its own, entries from before the times were kept show only the size); big PSDs are previewed from their thumbnail (`big-psd-open.md`). The website, Discord and GitHub links are gone from Home. Opening a file records its time in `fileHandling.recentOpened` (left out of settings files, like the list); it also sorts Recent by Last Opened. The framework supplies the rest: the × on a card and the revealed × in a row, right-click Remove from Recent / Clear Recent Files, previews decoded off the main thread, only those on screen.

Differences from the fork's own Home before the craft UI move: New / Open sit in the rail, not above the grid; the cards keep the framework's size (about six to a row on a desktop window) instead of filling the width in four to six columns; the full path shows in the card's tooltip and under each name in the list.

## Branch

`feat/home-recent-grid`

## Own files

`crates/ui-egui/src/canvas/home_grid.rs` (the list Home gets: count, opened time, dim line), `home_grid/caption.rs` (the dim line), `home_grid/preview.rs` (PSD preview, sizes), `home_grid/tests.rs`, `home_grid/snapshots.rs` (opt-in pictures: `PHOTOCRAFT_HOME_SNAPSHOTS`, `PHOTOCRAFT_HOME_PHASE`, `PHOTOCRAFT_HOME_SAMPLES`; grid and list view)

## Hooks in official files

- ui-egui `canvas.rs` (`mod home_grid`), `canvas/start_screen.rs` (the recent list and `max_recent` come from `home_grid`; no footer links), `home_recent_ui.rs` (`ThumbnailCache.sizes`, and the decoding job calls `home_grid::load`), `prefs/prefs_ui/fields.rs` (label of `homeRecentCount`), `app/files.rs` (`push_recent_at` records the time)
- engine-core `prefs.rs` (`Interface::home_recent_count`, default 12; `FileHandling::recent_opened`), `prefs/settings_sync.rs` (leaves `recentOpened` out), `prefs/validate.rs` (range 1–100)
- tests: `home_recent_gate_tests.rs` and `canvas/start_screen/tests.rs` find a row or card by `get_by_label_contains` (its name now carries the dim line)

## Tests

`home_grid` in ui-egui, plus the official Home tests (`home_recent`, `start_screen`)

```bash
cargo test -p photocraft-ui-egui --lib -- home_grid home_recent start_screen
```

## When upstream merges

If upstream changes `canvas/start_screen.rs` or the job in `home_recent_ui.rs`, keep the hooks above. A craft-libs move can change how a card's accessible name is built; the tests that look it up by `contains` follow it.
