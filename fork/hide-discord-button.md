# Discord button hidden

## What it does

No Discord link on Home (the rail's website, Discord and GitHub links are gone) and no "Join us on Discord" button in About (the Help menu's Discord item stays)

## Branch

`feat/hide-discord-button`

## Own files

none

## Hooks in official files

ui-egui `canvas/start_screen.rs` (an empty footer in place of `footer_links`), `dialogs/credits.rs` (no `.community(...)`; its three tests assert `community.is_none()`), `canvas/start_screen/tests.rs` (the rail test asserts the links are absent)

## Tests

`start_screen` and `nightly_about`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen nightly_about layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```

## After the craft UI merge (2026-11-27)

Rebuilt on the craft Home screen and About dialog.
