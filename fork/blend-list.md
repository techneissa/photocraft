# Blend list height and wheel

## What it does

Blend mode lists as tall as the window allows (no fixed 14-row cap, upstream issue #7261; upstream PR #7622 is stale and not taken), and Preferences ▸ Interface ▸ "Scroll Wheel Changes Blend Mode" (off by default: the wheel over a blend box scrolls instead of stepping the mode; on = Photoshop's way)

## Branch

`feat/blend-list` (before the craft UI merge); `feat/step2-blend-palette` (the rebuild)

## Own files

`crates/ui-egui/src/blend_wheel.rs` and `blend_wheel/tests.rs`

## Hooks in official files

Before the craft UI merge: `widgets/dropdown.rs` (`list_height()`), `panels/layers.rs`, `panels/options_bar.rs`, `panels/brush_controls.rs` (`dropdown_wheel_hovered` → `blend_wheel::dropdown`). Now: see below.

## After the craft UI merge (2026-11-27)

Rebuilt on craft's blend lists (`Select` over `blend_entries`), with two patches in the craft-libs fork copy ([craft-libs.md](craft-libs.md)):

- **Height:** craft capped a dropdown list at 40% of the window; patch 1 lets it take the room above or below its box (the larger side). A window about as tall as the list (about 700 px for the 29 modes at this text size, measured in a 1400 px window) shows every mode; in a shorter window the list still scrolls.
- **Wheel:** patch 2 makes the wheel over a blend mode box step the mode only when the preference is on; `blend_wheel.rs` copies the preference to the window every frame (`app/framework.rs`, one line). Off, the wheel no longer steps the mode (that it then scrolls the panel under it is not tested). Other dropdowns keep stepping with the wheel as craft has it.

Own files now: `crates/ui-egui/src/blend_wheel.rs` and `blend_wheel/tests.rs` (compiled in again). Official-file hooks: `app/framework.rs` (1 line), `lib.rs` (`mod blend_wheel`), and the official test `options_bar_mode_wheel_tests.rs` turns the preference on. The preference (`interface.scrollWheelChangesBlendMode`) stays in engine-core `prefs.rs`.

- **Tooltip:** patch 9 hides the box's own "Blend Mode" tooltip while its list is open (a window-tall list would sit under it and the tooltip covered "Normal"). Every component with a popup now shows no tooltip while its popup is open.

## Tests

`blend_wheel` (the preference defaults off and rejects bad values; off, the wheel over the options bar's Mode box changes no mode; on, it steps), `options_bar_mode_wheel` (on, the wheel steps the painting mode), `blend_rows` `an_open_blend_list_hides_the_boxs_tooltip` (closed, resting on the box shows its tooltip; with the list open and the pointer back on the box, none).

```bash
cargo test -p photocraft-ui-egui --lib -- blend_wheel options_bar_mode_wheel blend_rows
```
