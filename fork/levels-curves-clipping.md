# Levels and Curves clipping view

## What it does

Hold ⌥ while dragging the black or white slider in Levels or Curves and the canvas shows what the sliders clip, as Photoshop does. The white slider turns the image black except the pixels clipped to white, in the colour of the clipped channels (red alone is red, all three are white). The black slider turns the image white except the pixels clipped to black (red alone is cyan, all three are black). Grayscale shows black and white only. Letting go of ⌥ or the mouse shows the normal preview; nothing reaches the document or OK.

- Levels: the black and white Input sliders (gamma and Output Levels do not change what is marked).
- Curves: new black point and white point sliders under the graph move the curve's first and last point along the input axis; the Show Clipping checkbox keeps the view on while one is dragged, without ⌥. A single channel in the Channel menu marks that channel's colour only. The pencil mode has no points, so it has no sliders.
- A pixel is marked in Curves when the channel's curve and then the composite curve return black (or white) for it, rounded to the document's levels: the run of inputs from the black end (or the white end). A curve that comes back to black elsewhere (an inverted curve) is not marked there.

Works for RGB at 8, 16 and 32 bit and for Grayscale. CMYK, Lab, Quick Mask, channel and mask targets and non-pixel layers keep the normal preview; the Properties panel's adjustment layers have no clipping view yet.

How the canvas gets it: an opaque texture drawn over the visible part of the image (one texel per screen pixel, about). Per block of document pixels the target layer's minimum and maximum of each channel are scanned once per document revision, layer, view and block size, in parallel rows. A slider move only compares those with the new thresholds (a 16-bit key per channel and end, found from the compositor's own LUTs), so no frame scans the surface. On 24 and 36 MP the first scan is 30 to 50 ms and a slider move 3 to 6 ms (see Tests).

Camera Raw's clipping check, `photocraft_algo::histogram::clipping`, is the oracle of the Curves test: the display marks exactly the channels it finds at an end in the compositor's result. The scan itself is not Camera Raw's overlay, which paints a proxy of at most a few MP; the canvas can be 36 MP.

Takes upstream PR [storytold/photocraft#3693](https://github.com/storytold/photocraft/pull/3693) early (Levels, for [#3328](https://github.com/storytold/photocraft/issues/3328)), merged with its head `5a779498b` and moved onto the current layout; Curves is added on top.

## Branch

`feat/levels-curves-clipping`

## Own files

ui-egui `levels_clipping.rs` + `levels_clipping_tests.rs` (from #3693, widened to Curves: dialog, sync, request), `curves_clipping.rs` + `curves_clipping_tests.rs`, `dialogs/adjust_editors/curve_points.rs` (point sliders, Show Clipping)

## Hooks in official files

ui-egui `lib.rs` (two `mod` lines, the `levels_clipping` field), `app.rs` (its initial value), `canvas.rs` (the paint call, `visible_doc_rect` re-exported), `canvas/textures.rs` (`visible_doc_rect` is `pub(crate)`), `dialogs/adjust_dialog.rs` (`sync` call), `dialogs/adjust_editors.rs` (`mod curve_points`, re-exports), `dialogs/adjust_editors/levels.rs` (`levels_drag`), `dialogs/adjust_editors/curves.rs` (the sliders and the checkbox), `dialogs/adjust_preview.rs` (`targets_channel` taken out of `update`); `docs/control-protocol.md` (the `__clipping` field)

## Tests

ui-egui `levels_clipping curves_clipping`; the speed numbers come from the two ignored benchmarks.

```bash
cargo test -p photocraft-ui-egui --lib -- levels_clipping curves_clipping
cargo test -p photocraft-ui-egui --lib -- clipping_bench --ignored --nocapture --test-threads=1
```

## When upstream merges

If #3693 merges (it is open and conflicts with `main`), take its version of the Levels files and keep the Curves parts: `curves_clipping*`, `curve_points.rs`, and the three places in `levels_clipping.rs` that name Curves (`sync`, `dialog`, `thresholds`). If upstream adds its own Curves clipping view, drop `curves_clipping*` and `curve_points.rs` and the hooks in `curves.rs`.
