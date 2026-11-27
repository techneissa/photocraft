# Place Photos

## What it does

File ▸ Place Embedded… on ⇧⌘P (Place Linked unchanged), and an icon-only "Place Photo" button in the Layers-panel footer (greyed like the menu item, tooltip shows the key in effect; with Double Exposure back it sits next to it). Taking several photos at once is official now (`file_dialog::pick_files_bytes`, `place_picked`; official PRs #14107, #14110), so the fork's own multi-select picker is gone

## Branch

`feat/place-photos` (on `feat/double-exposure`)

## Own files

`crates/ui-egui/src/place_photos.rs` (the button's description and the command), `assets/icons/place-photo.svg`

## Hooks in official files

- engine-core `cmd/file_cmds.rs` (`file.placeEmbedded` default key `Cmd+Shift+P`), ui-egui `menu_catalog.rs` (the key)
- ui-egui `lib.rs` (module), `panels/layers_tree.rs` (`place_photo` in the model and its facts; `footer()` pushes it onto `LayersFooterState::more`), `panels/layers_events.rs` (`LayersFooterAction::More(0)` runs Place Embedded with null params, which opens the picker), `icon_data.rs` (icon)
- craft-libs fork copy, patch 8 ([craft-libs.md](craft-libs.md)): the footer's `more` buttons
- `ATTRIBUTION.md` (icon row; `place-photo.svg` in the Lucide row's exceptions)

## Tests

None of its own: the multi-photo picker is the official one and has official tests (`file_dialog`); the button is wiring.

```bash
cargo test -p photocraft-ui-egui --lib -- file_dialog icons::
```

## After the craft UI merge (2026-11-27)

Rebuilt on the craft UI (`feat/step2-layers-transform`): the Place Photo button is back through patch 8; several photos at once come from the official code, so `place_photos.rs` shrank to the button and its tests went (they tested the fork's own picker).
