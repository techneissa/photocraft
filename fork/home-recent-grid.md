# Home Recent Grid

## What it does

The Home screen puts the title, New document / Open and the drop hint at the top and shows the recent files as a grid of image previews that fills the rest of the canvas (four to six per row by width, scrolls when there are more). Click a preview to open it; hover shows a × in its top-right corner that takes it off Recent; right-click offers Remove from Recent and Clear Recent Files. The PhotoCraft website · GitHub · ArtCraft links are gone. Preferences › Interface › Recent files on Home (1–100, default 12) sets how many previews show; File › Open Recent keeps the whole list. Previews decode one at a time on a background thread (only those on screen; the thread reads file dates too, so a slow drive never stalls the window), at most two are uploaded per frame, a file saved again gets a new preview, and decoding stops when a document opens. Under each name one dim line gives the image size in pixels and when the file was last opened in PhotoCraft (`6000 × 4000 · 2 h ago`, `Yesterday`, then `3 Nov`; dates in UTC); the size shows once the preview is decoded (a PSD's from its header), and entries from before the times were kept show only the size. The full path, with the folder, is in the hover tooltip. Opening a file records its time in `fileHandling.recentOpened` (left out of settings files, like the list). Files over 512 MB, missing files and files PhotoCraft can't read show a file icon with their extension.

## Branch

`feat/home-recent-grid`

## Own files

`crates/ui-egui/src/canvas/home_grid.rs`, `home_grid/thumbs.rs`, `home_grid/tests.rs`, `home_grid/snapshots.rs` (opt-in pictures: `PHOTOCRAFT_HOME_SNAPSHOTS`, `PHOTOCRAFT_HOME_PHASE`, `PHOTOCRAFT_HOME_SAMPLES`)

## Hooks in official files

- ui-egui `canvas.rs` (module), `canvas/start_screen.rs` (4 lines at the top of `start_screen`: the grid Home draws instead; the official body stays), `prefs/prefs_ui/fields.rs` (label of `homeRecentCount`)
- engine `prefs.rs` (`Interface::home_recent_count`, default 12; `FileHandling::recent_opened`), `prefs/settings_sync.rs` (leaves `recentOpened` out), ui-egui `app/files.rs` (`push_recent` records the time), `Cargo.toml` (chrono for the date), `prefs/validate.rs` (range 1–100)

## Tests

`home_grid` in ui-egui, plus the official Home tests (`home_recent`, `start_screen`)

```bash
cargo test -p photocraft-ui-egui --lib -- home_grid home_recent start_screen
```

## When upstream merges

If upstream changes `start_screen.rs`, keep the 4-line hook at the top of `start_screen`; copy any new header behaviour (buttons, drop hint) into `home_grid.rs` `header_ui`.
