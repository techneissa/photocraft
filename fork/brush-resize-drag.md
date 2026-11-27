# Brush resize drag

## What it does

Control+Option+drag with a painting tool (on a Mac; Control+Alt elsewhere) changes the brush, and now follows the pointer on every step:

- It begins on the press, not once egui calls the movement a drag (about 6 points later); the brush stood still until then, and the first stretch was lost.
- The diameter and hardness move by the pointer's travel and are held at their limits (1 px, the maximum, 0 %, 100 %). Before, they were set from the pointer's distance to where the drag began, so dragging far past a limit (left of the start, or beyond the maximum) and turning back changed nothing until the pointer had retraced the whole overshoot.

Photoshop's look: the brush circle (it stays where the drag began) is filled with a translucent red overlay (Preferences › Cursors › Brush Preview colour, about 50 %) that is solid to the edge for a hard brush and fades out for a soft one; the outline stays; a small dark rounded label beside the pointer always shows Diameter, Hardness and Opacity together (the official label; the fork's earlier label showed Hardness only once it changed).

## Branch

`feat/step2-brush-resize` (on the craft UI, from `step2/official-merge`)

## Own files

`crates/ui-egui/src/tools/brush_resize/press.rs` (the stand-in pointer and its test), `look.rs` (the label rows and the overlay alpha), `drag_tests.rs`

## Hooks in official files

- `tools/brush_resize.rs`: `PaintResize` keeps `held: Option<press::Held>`; `press` creates it, `drag` and `release` pass the framework's `track` / `finish` the stand-in position, `cancel` drops it; `hud` takes the brush and shows `look::rows`; the preview tint alpha is `look::OVERLAY_ALPHA` (128, was 110); `mod look; mod press; mod drag_tests` lines.
- `canvas/paint_tool.rs`: the `hud` call passes the brush.
- No change to storytold/craft-libs. The framework's `BrushResize` sets the values from the pointer's distance to the press, clamped, so turning back after an overshoot waits for the retrace. `Held` moves a stand-in pointer by the real pointer's travel inside the box where both values are within their limits, and gives that to `track`: the framework's own rule, so the preview, the rounding and the readout stay its.
- Begin-on-press and the red falloff preview (solid hard, fading soft) are the official ones now.

## Tests

`brush_resize`

```bash
cargo test -p photocraft-ui-egui --lib -- brush_resize
```

`drag_tests` turns back after passing the diameter limit and after passing the hardness limit through the real canvas; both fail without `Held` (checked by passing the pointer through). `press::tests` checks the stand-in's limits and zoom.

## After the craft UI merge (2026-11-27)

Rebuilt on the craft UI. Compiled out, awaiting rebuild: nothing.
