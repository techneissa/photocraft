# Dropdown search

## What it does

Type to search in dropdown lists: typing in an open list narrows it (label starts, then word starts, then substrings), ↑/↓, ↩ or a click picks, Esc clears then closes, "No match"; Space and other held tool keys stay out while a list is open. Every dropdown built by `widgets/dropdown.rs`; not: crop ratio, kerning, JPEG quality preset, gradient map preset, shape stroke editor, credits sort (fonts have their own search)

## Branch

`feat/dropdown-search` (on `feat/blend-list`)

## Own files

`crates/ui-egui/src/widgets/dropdown_search.rs` and `dropdown_search/tests.rs`

## Hooks in official files

`widgets.rs` (module), `widgets/dropdown.rs` (`before(...)`, `.show_ui(` → `.search(`, the pick block replaces the `type_ahead::keys` lines, `row` / `no_match` hooks; 19+/12−), `input/hold_keys.rs` (3 early returns while a list is open), `examples/snapshot.rs` (`--type`); official tests `type_ahead.rs`, `blend_dropdown_keys_tests.rs` (same names, assertions for narrowing instead of the letter jump)

## Tests

`dropdown_search`, `blend_dropdown`, `type_ahead`, `hold_keys`

```bash
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
```
