# craft-libs fork copy

## What it does

PhotoCraft takes the craft interface framework from [storytold/craft-libs](https://github.com/storytold/craft-libs). A few fork features need a small change inside it that upstream does not have. Those changes live on the branch `photocraft-fork` of [techneissa/craft-libs](https://github.com/techneissa/craft-libs) (a fork of the official repo), and every craft-libs dependency in `Cargo.toml` (the workspace file, `crates/engine-core/Cargo.toml`, `crates/ipp/Cargo.toml`) points there at one `rev`, so Cargo sees one source. Without the patches nothing else differs: the branch is the official rev plus the commits below.

## Branch

`photocraft-fork` in `techneissa/craft-libs`, started from official `a46af76c7c74dd8dd375d694baaef00d5ae836a2`. One small commit per patch; each change in code carries a `// fork: photocraft-fork, "<name>"` comment, so `git grep "fork: photocraft-fork"` lists them all.

## Patches

| # | Commit subject | Where | Needed by |
|---|---|---|---|
| 1 | dropdown lists may take the whole window height | `ui-contract` `overlay/props.rs`: `DROPDOWN_SHARE` height 0.4 → 1.0 (one line) | [blend-list.md](blend-list.md): blend lists show as many rows as the room above or below the box holds; the placement code still picks the larger side |
| 2 | the wheel over a blend mode select steps the mode only when the app allows it | `ui-contract` `entry.rs` (`Entry::wheel_by_setting`), `ui-patterns` `blend_keys.rs` (`blend_entries` marks its entries), `ui-widgets` `pickers/select.rs` (`set_wheel_steps_blend_modes`, `wheel_steps_blend_modes`; a `Select` over marked entries leaves the wheel alone while the setting is off; default on, so upstream behaviour is unchanged) | [blend-list.md](blend-list.md): "Scroll Wheel Changes Blend Mode" |
| 3 | the command palette reports the typed text of a pick and puts a learned command first | `ui-patterns` `panels/palette.rs`: `PaletteOutput::query`, `CommandPalette::learned` (a command that goes first among the matches) | [palette-frecency.md](palette-frecency.md): the command picked for a typed text |
| 4 | tone histograms scale to the tallest level between the two ends and draw one column per level | `imaging` `ui/histogram/paint.rs`: `tone_top` takes the interior peak (edge peak only when nothing is between), `tone_mesh` draws a column per bin | [levels-histogram.md](levels-histogram.md) |

| 5 | menu rows carry shortcut text and a right-click item; palette rows get the same item; Paths panel menu rows take shortcut text | `ui-contract` `entry.rs` (`Entry::shortcut`, `Entry::row_action`), `ui-widgets` `menu/popup.rs` (`entry_rows` shows both; `take_row_action`), `ui-patterns` `panels/palette.rs` (`CommandPalette::row_action`, `PaletteOutput::row_action`), `draw` `ui/paths/{panel,menu}.rs` (`PathsPanel::shortcut`). Two commits | [context-menu-shortcuts.md](context-menu-shortcuts.md), [assign-shortcut-palette.md](assign-shortcut-palette.md) |

The Refine Hair button adds its patch to the same branch when they are rebuilt; add a row here for each.

## Move to a new upstream rev

1. In a clone of `techneissa/craft-libs`: `git fetch origin` (the official repo), `git switch photocraft-fork`.
2. `git rebase <new official rev>`. Each patch is a few lines in a different file, so a clash is a moved line: keep the official change and put the patch back.
3. Build the crates you touched: `cargo check -p craft-ui-contract -p craft-ui-widgets -p craft-ui-patterns`.
4. `git push --force-with-lease fork photocraft-fork` (a rebased branch; nobody else works on it).
5. In PhotoCraft, set every craft-libs `rev` in `Cargo.toml`, `crates/engine-core/Cargo.toml` and `crates/ipp/Cargo.toml` to the new head of `photocraft-fork`, and `cargo update -w`.
6. When the official rev has the change (or an equivalent), drop its commit during the rebase and its row above, and the fork code that used it. When no patch is left, point the dependencies back at `storytold/craft-libs` and delete this file.

## Tests

None of its own: the patches are used by [blend-list.md](blend-list.md), [palette-frecency.md](palette-frecency.md) and [levels-histogram.md](levels-histogram.md), whose tests cover them (patch 5: [context-menu-shortcuts.md](context-menu-shortcuts.md)).
