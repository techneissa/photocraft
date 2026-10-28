//! The "Scroll wheel changes blend mode" preference: off by default, the wheel never changes a
//! blend mode (it scrolls instead); on, it steps modes as in Photoshop.

use egui::vec2;
use photocraft_doc::BlendMode;
use serde_json::json;

use crate::blend_dropdown_wheel_tests as layers;
use crate::options_bar_mode_wheel_tests as bar;
use crate::state::Tool;

const PATH: &str = "interface.scrollWheelChangesBlendMode";

#[test]
fn the_setting_defaults_off_and_is_checked_like_its_neighbours() {
    let mut s = photocraft_engine::Session::new();
    assert_eq!(s.execute("prefs.get", json!({"path": PATH})).unwrap(), json!(false));
    assert!(!s.prefs().interface.scroll_wheel_changes_blend_mode);
    s.execute("prefs.set", json!({"path": PATH, "value": true})).unwrap();
    assert!(s.prefs().interface.scroll_wheel_changes_blend_mode);
    assert!(s.execute("prefs.set", json!({"path": PATH, "value": "yes"})).is_err());
    assert!(s.execute("prefs.set", json!({"path": PATH, "value": 1})).is_err());
    assert!(s.prefs().interface.scroll_wheel_changes_blend_mode, "a bad value changes nothing");
    s.execute("prefs.reset", json!({"path": PATH})).unwrap();
    assert!(!s.prefs().interface.scroll_wheel_changes_blend_mode);
    // Settings saved before the option existed read as off.
    let old: photocraft_engine::prefs::Interface = serde_json::from_value(json!({"showTooltips": false})).unwrap();
    assert!(!old.scroll_wheel_changes_blend_mode);
}

#[test]
fn off_the_wheel_over_the_layers_blend_box_changes_no_mode_and_records_no_step() {
    let mut h = layers::harness_with(vec2(1440.0, 900.0), false, layers::pixel_layer);
    let at = layers::dropdown(&h).center();
    let undo = layers::steps(&h);
    for lines in [-1.0, -3.0, 2.0] {
        layers::wheel(&mut h, at, lines);
    }
    assert_eq!((layers::blend(&h), layers::steps(&h)), (BlendMode::Normal, undo));
}

#[test]
fn off_the_wheel_over_the_open_blend_list_scrolls_it_and_changes_no_mode() {
    // A short window, so the list scrolls.
    let size = vec2(1440.0, 500.0);
    let mut h = layers::harness_with(size, false, layers::pixel_layer);
    let at = layers::dropdown(&h).center();
    layers::click(&mut h, at);
    let (last, darken) = (layers::entry(&h, "Luminosity").top(), layers::entry(&h, "Darken").center());
    let undo = layers::steps(&h);
    for _ in 0..4 {
        layers::wheel(&mut h, darken, -3.0);
    }
    h.run_steps(20);
    assert!(layers::entry(&h, "Luminosity").top() < last, "the list scrolled");
    assert_eq!((layers::blend(&h), layers::steps(&h)), (BlendMode::Normal, undo));
}

#[test]
fn turning_it_on_lets_the_wheel_step_modes_again() {
    let mut h = layers::harness_with(vec2(1440.0, 900.0), false, layers::pixel_layer);
    let at = layers::dropdown(&h).center();
    layers::wheel(&mut h, at, -1.0);
    assert_eq!(layers::blend(&h), BlendMode::Normal);
    h.state_mut().run("prefs.set", json!({"path": PATH, "value": true})).unwrap();
    h.run_steps(3);
    layers::wheel(&mut h, at, -1.0);
    assert_eq!(layers::blend(&h), BlendMode::Dissolve);
}

#[test]
fn off_the_wheel_over_the_options_bar_mode_box_changes_no_mode() {
    for tool in [Tool::Brush, Tool::Pencil, Tool::Gradient, Tool::PaintBucket, Tool::CloneStamp] {
        let mut h = bar::harness_with(tool, false);
        let at = bar::mode_dropdown(&h).center();
        let before = (bar::brush_mode(&h), h.state().ui.tool_options.gradient_blend_mode, h.state().ui.tool_options.bucket_blend_mode);
        bar::wheel(&mut h, at, -2.0);
        let after = (bar::brush_mode(&h), h.state().ui.tool_options.gradient_blend_mode, h.state().ui.tool_options.bucket_blend_mode);
        assert_eq!(before, after, "{tool:?}");
        assert!(!h.state().ui.tool_options.brush_clear && !h.state().ui.tool_options.bucket_clear, "{tool:?}");
    }
}
