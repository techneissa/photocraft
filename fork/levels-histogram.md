# Levels histogram drawing

## What it does

The histogram box in Image ▸ Adjustments ▸ Levels and Curves, and the Histogram panel's single-channel graph, draw each of the 256 levels as its own column, as Photoshop's Levels does, and scale to the tallest level between the two ends.

Before, an image that is mostly pure black or pure white (the NASA Earthrise photo, two thirds black) showed an empty box: the scale followed the black level, which squashed every other level to nothing, and the black spike was a triangle under one pixel wide at the left edge. Now the black or white spike is a full-height column at its edge and runs off the top, and the rest of the histogram fills the box. An image with nothing between the ends (flat black, or a black and white split) scales to its tallest end, so a 90/10 split still draws 9 : 1. Camera Raw's histogram already scaled this way (`dialogs/rgb_histogram.rs`).

The histogram data was checked on five public-domain photos against an independent count (PIL): peaks in the same places and heights; the data is right.

## Branch

`fix/levels-histogram-drawing`

## Own files

none (the change is inside `widgets/tone.rs`)

## Hooks in official files

none in this repo: craft-libs `photocraft-fork`, `ui/histogram/paint.rs` (`tone_top`, `tone_mesh`).

## Tests

ui-egui `levels_histogram_tests.rs`.

```bash
cargo test -p photocraft-ui-egui --lib -- levels_histogram
```

## When upstream merges

If upstream changes `histogram_scale_top` or `draw_histogram`, keep the interior-peak scale and the column drawing; the Histogram panel and Camera Raw share the look.

## After the craft UI merge (2026-11-27)

Rebuilt. The craft histograms did not do it: `tone_top` scaled to the tallest level, edges included, so a mostly black photo showed an empty box, and `tone_mesh` joined bin centres. The fix is a craft-libs patch on `photocraft-fork` ([craft-libs.md](craft-libs.md), patch 4): `tone_top` ignores the two end bins unless nothing lies between them, and `tone_mesh` draws one column per bin. It covers the Levels and Curves histograms, Threshold and the Histogram panel's single-channel graph (all `HistogramData::tone`). The RGB ribbons already scaled to the interior.

Hooks here: none in official code. Test: `levels_histogram_tests.rs` (a mostly black histogram's middle levels get height; an end-only histogram fills the box; columns sit on whole slots).
