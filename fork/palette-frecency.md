# Command palette learns

## What it does

The command palette (⌘F, ⌘P) learns from the commands run through it, as launchers such as Alfred and Raycast do ([#28](https://github.com/techneissa/photocraft/issues/28)):

- An empty search lists the commands run most often and most recently first (a use counts half as much after a week), then this session's recently run ones (the first 8, as in craft's palette).
- While typing, text match comes first; among matches about as good, the one run more often goes first. A clearly better text match always wins over past use.
- A command picked after typing a text (for example "exp" → Export As…) comes first the next time that text, or its start ("ex"), is typed. The newest pick wins.
- The history is kept in the preferences (`paletteHistory`), so it survives a restart: up to 300 commands and 200 typed texts. It stays on this Mac (left out of Settings sync).

## Branch

`feat/palette-frecency` (before the craft UI merge); `feat/step2-blend-palette` (the rebuild)

## Own files

`crates/ui-egui/src/dialogs/palette/frecency.rs` and `frecency/tests.rs`

## Hooks in official files

- engine-core `prefs.rs` (`Preferences::palette_history`), `prefs/settings_sync.rs` (`paletteHistory` in `NOT_IN_FILE`)
- ui-egui `dialogs/palette.rs` (`mod frecency`; the history passed to craft's palette; `frecency::note` when a row ran)

## After the craft UI merge (2026-11-27)

Rebuilt on craft's `CommandPalette`, with one patch in the craft-libs fork copy ([craft-libs.md](craft-libs.md), patch 3) and the rest through craft's own hooks:

- **Empty search:** `History::recents` (most used and most recent first, then this session's) is passed as the palette's `recents` (craft lists the first 8; the fork's "then the rest" is gone).
- **Typed search:** `History::uses` goes to craft's `uses` hook, which orders matches that grade the same; a clearly better text match still wins over past use (craft's tiers, in place of the fork's 7-point rule).
- **Typed text:** patch 3 gives `PaletteOutput::query` (the text typed when a row ran) and `CommandPalette::learned` (the command that goes first among the matches). `dialogs/palette.rs` records `History::record` from the output and answers `learned` from `History::learned`. Only a command that matches the text comes first; a command that does not match is not added.
- craft alone already puts a command picked this session first among matches that grade the same (its `recents`). What the fork adds: past use across restarts, and the pick for a typed text beating better matches ("exp" → Quick Export as PNG), including a start of the text ("ex").

Own files: `dialogs/palette/frecency.rs` and `frecency/tests.rs` (compiled in again; `mod frecency;` in `dialogs/palette.rs`). `palette/list.rs` and `palette/nav.rs` stay compiled out (see [assign-shortcut-palette.md](assign-shortcut-palette.md)). The preference `paletteHistory` is unchanged and stays out of Settings sync.

## Tests

`frecency` (use grading, recents order, the pick for a text and its start, restart through the preferences, size limits, and one run through the real palette: type "exp", pick the last row, reopen, "exp" and "ex" list it first); the palette's own tests still pass.

```bash
cargo test -p photocraft-ui-egui --lib -- frecency palette
```
