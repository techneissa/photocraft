//! The Levels, Curves and Threshold histograms scale to the tallest level between the two ends
//! (fork/levels-histogram.md; the patch is in craft-libs `photocraft-fork`).
use craft::imaging::ui::histogram::HistogramScale;
use craft::imaging::ui::histogram::paint::{tone_heights, tone_mesh};
use egui::{Color32, Rect, pos2, vec2};

/// A photo two thirds pure black: the black spike runs off the top and the middle levels stay visible.
#[test]
fn a_mostly_black_histogram_shows_its_middle_levels() {
    let mut bins = vec![0.0f32; 256];
    bins[0] = 1_000_000.0;
    for (i, b) in bins.iter_mut().enumerate().take(200).skip(60) {
        *b = 10_000.0 + (i % 7) as f32 * 500.0;
    }
    let heights = tone_heights(&bins, HistogramScale::Peak);
    assert_eq!(heights[0], 1.0, "the black spike fills the box");
    assert!(heights[100] > 0.5, "the middle is visible, not flattened: {}", heights[100]);
    // Nothing between the ends: the tallest end sets the scale.
    let mut flat = vec![0.0f32; 256];
    flat[0] = 7.0;
    assert!(tone_heights(&flat, HistogramScale::Peak)[0] > 0.8);
    // A lone bin is a column as wide as its slot, not a triangle.
    let mesh = tone_mesh(Rect::from_min_size(pos2(0.0, 0.0), vec2(256.0, 100.0)), &heights, Color32::WHITE);
    let xs: Vec<f32> = mesh.vertices.iter().map(|v| v.pos.x).collect();
    assert!(xs.iter().all(|x| (x - x.round()).abs() < 1e-3), "columns sit on whole slots");
}
