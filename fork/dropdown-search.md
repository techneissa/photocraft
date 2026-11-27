# Dropdown search

## What it does

Type to search in dropdown lists: typing in an open list narrows it (label starts, then word starts, then substrings), ↑/↓, ↩ or a click picks, Esc clears then closes, "No match"; Space and other held tool keys stay out while a list is open. Every dropdown built by `widgets/dropdown.rs`; not: crop ratio, kerning, JPEG quality preset, gradient map preset, shape stroke editor, credits sort (fonts have their own search)

The field is as wide as the full list less its magnifier and spacing, and the list's width is read only while nothing is typed: a field wider than the list widened it, the widened rows were read back as the list's width, and the list grew on every frame (and repainted on every frame) while a query was typed.

## Branch

`feat/dropdown-search` (on `feat/blend-list`); `fix/dropdown-shortcuts-speed`

## Own files

`crates/ui-egui/src/dropdown_search.rs` and `dropdown_search/tests.rs` (the old `widgets/dropdown_search.rs`, compiled out)

## Hooks in official files

On the craft UI: `lib.rs` (`mod dropdown_search`), `app/framework.rs` (`dropdown_search::sync(ctx)`), craft-libs patch 11. Before it: `widgets.rs` (module), `widgets/dropdown.rs` (`before_with(...)` makes the labels only for an open list, `.show_ui(` → `.search(`, the pick block replaces the `type_ahead::keys` lines, `row` / `no_match` hooks; 19+/12−), `input/hold_keys.rs` (3 early returns while a list is open), `examples/snapshot.rs` (`--type`); official tests `type_ahead.rs`, `blend_dropdown_keys_tests.rs` (same names, assertions for narrowing instead of the letter jump)

## Tests

`dropdown_search`: an open Mode list narrows to the modes with the typed letters and ↓ ↩ picks; letters typed on the closed box all reach the field and ↩ picks the first match

```bash
cargo test -p photocraft-ui-egui --lib -- dropdown_search
```

**Official merge of 2026-11-27:** the official dropdown moved from `craft_ui::ComboBox` to its own `combo_list`, so the `Searchable` trait is gone: `dropdown_with` and `dropdown_with_tooltips` call `find.header(ui)` as the first row inside the list (it scrolls with the rows) and keep the official greyed-option rule (`enabled` rows are left out of the matches).

## After the craft UI merge (2026-11-27)

Rebuilt on craft's own dropdown (`Select`): `dropdown_search.rs` tells the window that every select with 8 entries or more searches, and craft-libs patch 11 ([craft-libs.md](craft-libs.md)) does the rest. The old `widgets/dropdown_search.rs` and its tests stay in the tree, compiled out, for reference.

**Comparison** (the brush Mode list, 29 modes, in the app's test window; the craft code read in `ui-widgets` `pickers/select.rs` and `search.rs`):

| | Official craft | Fork's old list |
|---|---|---|
| Typed letters on an open list | the highlight jumps to the first mode that begins with them ("li": Linear Burn); nothing is hidden | narrows to the modes that match: begin with, a word begins with, contain ("burn": Color Burn and Linear Burn) |
| Letters that begin no mode ("burn") | nothing happens, the highlight stays | still finds Color Burn and Linear Burn |
| No match | nothing says so | a dim "No match" line |
| ↑/↓ | step through all rows | step through the matches |
| ↩ | picks the highlight | picks the first match |
| Esc | closes | clears the text, then closes |
| Reset on close | nothing to reset | the text starts empty at every open |
| Letters typed on the closed box | open the list on the mode they begin | open the list with the letters in the field (the first one is kept) |
| Where it applies | only a select the app marks `.searchable()` (a search field, always shown): none of the blend, Mode or filter lists, only the font choice of Export Artboards and the command choice of Actions | every list built by `widgets/dropdown.rs` |

The official list does less where it matters: a blend list of 29 modes cannot be narrowed by a word inside a name, and a miss says nothing. Its `.searchable()` select does narrow, say "No matches" and start empty at every open, but needs a per-list opt-in the lists in craft's panels cannot get from the app. So the fork's behaviour is back, on craft's list:

- patch 11 `auto_search_from` / `set_auto_search_from`: the window says from how many entries a select is searchable; `dropdown_search::sync` sets 8 each pass.
- a searchable select takes the letters typed on its closed box into the field (the old jump used the first letter up) and keeps the highlight on the first match, so ↩ picks it.

What it adds over craft's own `.searchable()`: it applies to every long list without an opt-in, the first letter typed on the closed box counts, and ↩ picks the first match. What differs from the old list: the search field is there from the opening, as in craft's own searchable lists (the old one showed after the first letter); the matches keep craft's own ranking (the list's order, with word starts first only as craft scores them); lists with fewer than 8 entries stay plain (the Layers panel's filter type list has 6; its Kind filter is five icon buttons, no list); fonts keep craft's font picker, which has its own search.
