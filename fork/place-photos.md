# Place Photos

## What it does

File ▸ Place Embedded… on ⇧⌘P (Place Linked unchanged) and takes several photos at once: the picker is a multi-select, one photo is placed as before, two or more are queued like a multi-file canvas drop (each its own layer, Free Transform in turn, ↩ applies, Esc takes back, a photo that fails is reported and skipped); an icon-only "Place Photo" button in the Layers-panel footer next to Double Exposure (greyed like the menu item, tooltip shows the key in effect)

## Branch

`feat/place-photos` (on `feat/double-exposure`)

## Own files

`crates/ui-egui/src/place_photos.rs` and `place_photos/tests.rs`, `assets/icons/place-photo.svg`

## Hooks in official files

- engine `cmd/file_cmds.rs` (`file.placeEmbedded` default key `Cmd+Shift+P`)
- ui-egui `lib.rs` (module), `menu_catalog.rs` (the key), `menus/view_cmds/dialogs.rs` (3 lines: Place Embedded without params → `place_photos::pick`), `file_open.rs` (`DropPlace::Path` and its arm in `place_next_dropped`), `file_dialog.rs` (`read_picked` made `pub(crate)`; `place_photos` passes `usize::MAX` for main's size cap), `panels/layers.rs` (1 line: the button), `icon_data.rs` (icon), `file_dialog/tests.rs` (the Place Embedded test expects a multi-select)
- `ATTRIBUTION.md` (icon row; `place-photo.svg` in the Lucide row's exceptions)

## Tests

`place_photos` in ui-egui

```bash
cargo test -p photocraft-ui-egui --lib -- place_photos file_dialog
```
