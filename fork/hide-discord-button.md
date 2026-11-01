# Discord button hidden

## What it does

No "Join us on Discord" button on the start screen or in About (the Discord link in the link row stays)

## Branch

`feat/hide-discord-button`

## Own files

none

## Hooks in official files

ui-egui `canvas/start_screen.rs`, `dialogs.rs` (2 lines removed each), `links.rs` (`#[allow(dead_code)]` on `discord_button`), `canvas/tests.rs` (1 test appended)

## Tests

`start_screen`

```bash
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
```
