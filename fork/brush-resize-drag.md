# Brush resize drag

## What it does

Control+Option+drag with a painting tool (on a Mac; Control+Alt elsewhere) changes the brush, and now follows the pointer on every step:

- It begins on the press, not once egui calls the movement a drag (about 6 points later); the brush stood still until then, and the first stretch was lost.
- The diameter and hardness move by the pointer's travel and are held at their limits (1 px, the maximum, 0 %, 100 %). Before, they were set from the pointer's distance to where the drag began, so dragging far past a limit (left of the start, or beyond the maximum) and turning back changed nothing until the pointer had retraced the whole overshoot.

Photoshop's look: the brush circle (it stays where the drag began) is filled with a translucent red overlay (Preferences › Cursors › Brush Preview colour, about 50 %) that is solid to the edge for a hard brush and fades out for a soft one; the outline stays; a small dark rounded label beside the pointer reads `Diameter: 120 px`, and `Hardness: 50%` (`Opacity:` with "Vary Round Brush Hardness on HUD" off) once the hardness is being changed.

## Branch

`fix/brush-resize-drag`

## Own files

`crates/ui-egui/src/tools/brush_resize/press.rs`, `look.rs`, `drag_tests.rs`

## Hooks in official files

`tools/brush_resize.rs` (`Resize` keeps the unrounded brush and the last pointer position, `travel` replaces `resized` in `update`, `draw` calls `look`, `mod` lines), `canvas.rs` (three calls in `handle_canvas_tool_stroke`: begin on press and skip egui's drag start and click, feed moves while the button is down, end on release); official test `tools/brush_resize/tests.rs` (the overlay's alpha 110 → 128)

## Tests

`brush_resize`

```bash
cargo test -p photocraft-ui-egui --lib -- brush_resize
```
