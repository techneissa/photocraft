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

ui-egui `widgets/tone.rs`: `histogram_scale_top` (the scale), new `histogram_columns`, `draw_histogram` (columns instead of strips joining bin centres); the tests of both.

## Tests

ui-egui `tone` (scale and column tests).

```bash
cargo test -p photocraft-ui-egui --lib -- tone histogram
```

## When upstream merges

If upstream changes `histogram_scale_top` or `draw_histogram`, keep the interior-peak scale and the column drawing; the Histogram panel and Camera Raw share the look.
