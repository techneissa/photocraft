# Preferences scroll width

## What it does

Preferences pages, Keyboard Shortcuts tabs: lists fill the window with the scrollbar at the right edge; Toolbar tab in 1–4 columns by width; Keyboard Shortcuts keeps its width while searching

## Branch

`fix/prefs-scroll-width`, `fix/shortcuts-tabs-layout` (from `origin/main`), `fix/shortcuts-search-width` (from `feat/shortcut-upgrade`)

## Own files

none

## Hooks in official files

ui-egui `prefs/prefs_ui/fields.rs` (`auto_shrink`), `prefs_ui/shortcuts.rs` (`name_width`, `toolbar_columns`, min width), `prefs_ui/tests.rs` (tests appended)

## Tests

`prefs_ui`

```bash
cargo test -p photocraft-ui-egui --lib -- prefs_ui
```
