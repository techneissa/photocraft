# Windows open centred

## What it does

Every dialog (Settings, Keyboard Shortcuts, the Load Settings from Folder prompt, every command dialog) goes through `dialogs::show`, so one rule lives there:
- The first time a dialog opens it is centred on the app window.
- A dialog you moved by its title bar reopens where you left it (egui memory, saved in `ui.ron`). If that place is now off screen (a smaller window or display, another Mac) it is centred again.
- Short confirm and alert prompts (Load Settings from Folder / Import Settings, Embedded Profile Mismatch, errors, the delete / replace / rasterize / disk-change / mask / preset-group prompts) always open centred and never remember a place.
- Cause of "opens wherever": Settings, Keyboard Shortcuts, the Import prompt and every other dialog without a command id shared one remembered place, so moving one moved them all. Settings and the other `__prefsui` windows now each have their own.
- Not changed: the Blur Tools panel keeps its deliberate first place at the right edge; the full-screen workspaces (Liquify, Camera Raw, Select and Mask…) fill the window; floating panels (`egui::Window`: Brush Settings, Type Styles, Glyphs, Gradient Editor…) keep egui's own memory, and the anchored prompts (Import keyboard shortcuts, Switched to CPU, Check Spelling) already centre.

## Branch

`feat/windows-open-centred`

## Own files

ui-egui `dialogs/placement.rs` (rule, pure functions and tests), `examples/dialog_place_snapshot.rs` (offscreen picture of a dialog's first open)

## Hooks in official files

ui-egui `dialogs.rs` (`mod placement`, `place_id` calls it, the remembered place goes through `start_offset` and is skipped for alerts, a move of an alert is not saved)

## Tests

ui-egui `dialogs::placement`

```bash
cargo test -p photocraft-ui-egui --lib placement
```
