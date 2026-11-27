//! The "Scroll wheel changes blend mode" preference: off by default, the wheel never changes a
//! blend mode (it scrolls instead); on, it steps modes as in Photoshop.

use photocraft_doc::BlendMode;
use serde_json::json;

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

/// The wheel over the options bar's Mode box, one turn of `lines` lines, with the preference `on`.
fn wheeled_brush_mode(on: bool, lines: f32) -> BlendMode {
    let mut h = bar::harness(Tool::Brush);
    h.state_mut().run("prefs.set", json!({"path": PATH, "value": on})).unwrap();
    h.run_steps(3);
    let at = bar::mode_dropdown(&h).center();
    bar::wheel(&mut h, at, lines);
    bar::brush_mode(&h)
}

#[test]
fn off_the_wheel_over_a_blend_mode_box_changes_no_mode_and_on_it_steps_the_mode() {
    assert_eq!(wheeled_brush_mode(false, -2.0), BlendMode::Normal);
    assert_eq!(wheeled_brush_mode(true, -1.0), BlendMode::Dissolve);
}
