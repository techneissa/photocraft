# Command palette learns

## What it does

The command palette (⌘F, ⌘P) learns from the commands run through it, as launchers such as Alfred and Raycast do ([#28](https://github.com/techneissa/photocraft/issues/28)):

- An empty search lists the commands run most often and most recently first (a use counts half as much after a week), then this session's recently run ones, then the rest.
- While typing, text match comes first; among matches about as good, the one run more often goes first. A match 7 points better (about one more letter in a row) always wins over past use.
- A command picked after typing a text (for example "exp" → Export As…) comes first the next time that text, or its start ("ex"), is typed. The newest pick wins.
- The history is kept in the preferences (`paletteHistory`), so it survives a restart: up to 300 commands and 200 typed texts. It stays on this Mac (left out of Settings sync).

## Branch

`feat/palette-frecency`

## Own files

`crates/ui-egui/src/dialogs/palette/frecency.rs` and `frecency/tests.rs`

## Hooks in official files

- engine `prefs.rs` (`Preferences::palette_history`), `prefs/settings_sync.rs` (`paletteHistory` in `NOT_IN_FILE`)
- ui-egui `dialogs/palette.rs` (`mod frecency`; the history passed to the list; `frecency::note` when a row runs), `dialogs/palette/list.rs` (fork file: `all_hits` takes the history, always lists the rest under the recents, `History::apply` before ranking), `dialogs/palette/list/upgrade_tests.rs` (the new arguments)

## Tests

`frecency` (ranking rules, restart, size limits); the palette's own tests still pass.

```bash
cargo test -p photocraft-ui-egui --lib -- frecency palette
```

## When upstream merges

The official palette moves to the craft UI framework in the Nov 17 update (step two of the official merge): move `History::apply` and `note` to wherever it builds and runs its rows.
