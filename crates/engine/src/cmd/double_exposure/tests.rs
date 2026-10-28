use photocraft_color::{BlendMode, SampleType};
use photocraft_doc::{Document, Layer, LayerId};

use super::*;
use crate::channel_cmds::ChannelTarget;

const DEPTHS: [u64; 3] = [8, 16, 32];

/// A transparent document with the pixel layers `A`, `B`, `C` (bottom to top), `C` active.
fn session(depth: u64) -> (Session, [LayerId; 3]) {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 24, "height": 16, "depth": depth, "background": "transparent"})).unwrap();
    let a = s.active().unwrap().active_layer.unwrap();
    let b = id(s.execute("layer.new.layer", json!({"name": "B"})).unwrap());
    let c = id(s.execute("layer.new.layer", json!({"name": "C"})).unwrap());
    (s, [a, b, c])
}

fn id(v: Value) -> LayerId {
    LayerId(v["layer"].as_u64().unwrap())
}

fn doc(s: &Session) -> &Document {
    &s.active().unwrap().doc
}

fn layer(s: &Session, id: LayerId) -> &Layer {
    doc(s).layer(id).unwrap()
}

fn select(s: &mut Session, id: LayerId) {
    s.execute("layer.select", json!({"layer": id.0})).unwrap();
}

fn mask_targeted(s: &Session) -> bool {
    s.active().unwrap().channel_view.target == ChannelTarget::LayerMask && s.tools.mask_targeted
}

#[test]
fn double_exposure_needs_two_pixel_layers() {
    let mut s = Session::new();
    assert!(s.disabled_reason(CMD).is_some(), "no document");
    assert!(s.execute(CMD, json!({})).is_err());

    s.execute("file.new", json!({"width": 24, "height": 16, "background": "transparent"})).unwrap();
    let why = s.disabled_reason(CMD).expect("one layer");
    assert!(why.contains("two visible pixel layers"), "{why}");
    let before = s.active().unwrap().doc.clone();
    assert!(s.execute(CMD, json!({})).is_err());
    assert!(std::sync::Arc::ptr_eq(&before, &s.active().unwrap().doc), "a refused command changes nothing");

    // An adjustment layer and a group are not pixel layers.
    s.execute("layer.newAdjustmentLayer.invert", json!({})).unwrap();
    s.execute("layer.new.group", json!({})).unwrap();
    assert!(s.disabled_reason(CMD).is_some(), "one pixel layer under an adjustment layer and a group");
    s.execute("layer.new.layer", json!({})).unwrap();
    assert!(s.disabled_reason(CMD).is_none());
}

#[test]
fn sets_screen_adds_a_mask_and_targets_it() {
    for depth in DEPTHS {
        let (mut s, [_, _, c]) = session(depth);
        s.execute("layer.setProps", json!({"layer": c.0, "opacity": 0.6})).unwrap();
        let out = s.execute(CMD, json!({})).unwrap();
        assert_eq!(out, json!({"layer": c.0, "mode": "screen", "maskAdded": true}), "depth {depth}");

        let l = layer(&s, c);
        assert_eq!(l.blend, BlendMode::Screen);
        assert_eq!(l.opacity, 0.6, "opacity is left alone");
        let mask = l.mask.as_ref().expect("mask");
        assert!(mask.enabled && mask.linked);
        assert_eq!(mask.surface.sample_channel(3, 3, 0), 1.0, "Reveal All");
        let want = match depth {
            8 => SampleType::U8,
            16 => SampleType::U16,
            _ => SampleType::F32,
        };
        assert_eq!(mask.surface.format().sample, want, "the mask has the document's depth");
        assert_eq!(s.active().unwrap().active_layer, Some(c));
        assert!(mask_targeted(&s), "depth {depth}");
    }
}

#[test]
fn lighten_and_mode_names() {
    for depth in DEPTHS {
        let (mut s, [_, _, c]) = session(depth);
        let out = s.execute(CMD, json!({"mode": "LIGHTEN"})).unwrap();
        assert_eq!(out["mode"], "lighten");
        assert_eq!(layer(&s, c).blend, BlendMode::Lighten, "depth {depth}");
        assert!(layer(&s, c).mask.is_some());
    }
    let (mut s, [_, _, c]) = session(8);
    s.execute(CMD, json!({"mode": "screen"})).unwrap();
    assert_eq!(layer(&s, c).blend, BlendMode::Screen);
    s.execute(CMD, json!({"mode": null})).unwrap();
    assert_eq!(layer(&s, c).blend, BlendMode::Screen);
}

#[test]
fn picks_the_selected_layer_else_the_topmost() {
    let (mut s, [a, b, c]) = session(8);

    // One selected pixel layer that isn't the lowest: that one.
    select(&mut s, b);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], b.0);
    assert_eq!(layer(&s, b).blend, BlendMode::Screen);
    assert_eq!(layer(&s, c).blend, BlendMode::Normal);
    assert_eq!(s.active().unwrap().selected_layers(), vec![b]);

    // The lowest pixel layer: the topmost instead.
    select(&mut s, a);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);
    assert_eq!(layer(&s, a).blend, BlendMode::Normal);
    assert_eq!(s.active().unwrap().active_layer, Some(c));

    // Several selected: the topmost.
    let (mut s, [a, b, c]) = session(8);
    select(&mut s, a);
    s.execute("layer.select", json!({"layer": b.0, "mode": "add"})).unwrap();
    assert_eq!(s.active().unwrap().selected_layers().len(), 2);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);
    assert_eq!(s.active().unwrap().selected_layers(), vec![c]);
}

#[test]
fn a_selected_group_or_adjustment_layer_falls_back_to_the_topmost_pixel_layer() {
    let (mut s, [_, _, c]) = session(8);
    let adj = id(s.execute("layer.newAdjustmentLayer.invert", json!({})).unwrap());
    assert_eq!(s.active().unwrap().active_layer, Some(adj));
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);

    let (mut s, [_, _, c]) = session(8);
    let group = id(s.execute("layer.new.group", json!({})).unwrap());
    assert_eq!(s.active().unwrap().active_layer, Some(group));
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);
    assert_eq!(layer(&s, group).blend, BlendMode::PassThrough, "the group is untouched");
}

#[test]
fn a_pixel_layer_inside_a_group_counts() {
    let (mut s, [_, b, c]) = session(8);
    // B and C go into one group: A is the only pixel layer outside it, and still the lowest.
    select(&mut s, b);
    s.execute("layer.select", json!({"layer": c.0, "mode": "add"})).unwrap();
    s.execute("layer.groupLayers", json!({})).unwrap();
    assert!(doc(&s).layers.iter().any(|l| l.children().is_some()), "grouped");
    assert!(s.disabled_reason(CMD).is_none());
    select(&mut s, b);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], b.0);
    assert_eq!(layer(&s, b).blend, BlendMode::Screen);
}

#[test]
fn an_existing_mask_is_kept() {
    let (mut s, [_, _, c]) = session(8);
    s.execute("layer.layerMask.hideAll", json!({"layer": c.0})).unwrap();
    let out = s.execute(CMD, json!({})).unwrap();
    assert_eq!(out["maskAdded"], false);
    assert_eq!(layer(&s, c).mask.as_ref().unwrap().surface.sample_channel(3, 3, 0), 0.0, "Hide All stays");
    assert!(mask_targeted(&s));

    // Run again: still one mask, nothing replaced.
    let out = s.execute(CMD, json!({})).unwrap();
    assert_eq!(out["maskAdded"], false);
    assert_eq!(layer(&s, c).mask.as_ref().unwrap().surface.sample_channel(3, 3, 0), 0.0);
}

#[test]
fn one_undo_step_restores_everything() {
    let (mut s, [_, _, c]) = session(8);
    s.execute("layer.setProps", json!({"layer": c.0, "opacity": 0.6})).unwrap();
    s.execute(CMD, json!({})).unwrap();

    assert!(s.undo());
    let l = layer(&s, c);
    assert_eq!(l.blend, BlendMode::Normal);
    assert!(l.mask.is_none());
    assert_eq!(l.opacity, 0.6, "the step before it is still done");
    assert!(!mask_targeted(&s), "the mask is gone, so it is no longer the target");

    assert!(s.redo());
    let l = layer(&s, c);
    assert_eq!(l.blend, BlendMode::Screen);
    assert!(l.mask.is_some());
    assert!(mask_targeted(&s), "the redone step targets its mask again");
}

#[test]
fn redo_targets_the_mask_when_the_pixels_were_targeted_meanwhile() {
    let (mut s, [_, _, c]) = session(8);
    s.execute(CMD, json!({})).unwrap();
    assert!(s.undo());
    s.execute("channel.target", json!({"channel": "composite"})).unwrap();
    assert!(!mask_targeted(&s));
    assert!(s.redo());
    assert!(mask_targeted(&s));
    assert_eq!(s.active().unwrap().active_layer, Some(c));

    // Redoing some other step leaves the target alone.
    let (mut s, [_, _, c]) = session(8);
    s.execute("layer.layerMask.revealAll", json!({"layer": c.0})).unwrap();
    s.execute("channel.target", json!({"channel": "composite"})).unwrap();
    assert!(s.undo());
    assert!(s.redo());
    assert!(!mask_targeted(&s));
}

#[test]
fn bad_params_are_errors_and_change_nothing() {
    let (mut s, [_, _, c]) = session(8);
    let before = s.active().unwrap().doc.clone();
    for p in [json!({"mode": "multiply"}), json!({"mode": 3}), json!({"mode": ["screen"]}), json!({"mode": ""}), json!([1, 2]), json!("screen"), json!(7)] {
        assert!(s.execute(CMD, p.clone()).is_err(), "{p}");
    }
    assert!(std::sync::Arc::ptr_eq(&before, &s.active().unwrap().doc));
    assert_eq!(layer(&s, c).blend, BlendMode::Normal);
}

fn hide(s: &mut Session, id: LayerId) {
    s.execute("layer.setProps", json!({"layer": id.0, "visible": false})).unwrap();
}

fn lock(s: &mut Session, id: LayerId) {
    s.execute("layer.setProps", json!({"layer": id.0, "locks": {"all": true}})).unwrap();
}

#[test]
fn hidden_layers_are_never_the_target() {
    let (mut s, [_, b, c]) = session(8);
    hide(&mut s, c);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], b.0, "the topmost visible one");
    assert_eq!(layer(&s, c).blend, BlendMode::Normal);

    // A hidden layer that is selected is passed over too.
    let (mut s, [_, b, c]) = session(8);
    hide(&mut s, b);
    select(&mut s, b);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);

    // With the upper layers hidden, the bottom one has nothing below it.
    let (mut s, [_, b, c]) = session(8);
    hide(&mut s, b);
    hide(&mut s, c);
    assert!(s.disabled_reason(CMD).is_some());
    assert!(s.execute(CMD, json!({})).is_err());

    // A hidden bottom layer leaves two visible ones: the lower of them is the new bottom.
    let (mut s, [a, _, c]) = session(8);
    hide(&mut s, a);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);
}

#[test]
fn layers_in_a_hidden_group_are_hidden() {
    let (mut s, [_, b, c]) = session(8);
    select(&mut s, b);
    s.execute("layer.select", json!({"layer": c.0, "mode": "add"})).unwrap();
    let group = id(s.execute("layer.groupLayers", json!({})).unwrap());
    hide(&mut s, group);
    assert!(s.disabled_reason(CMD).is_some(), "only the bottom layer is visible");
}

#[test]
fn locked_layers_are_never_the_target_but_may_be_the_bottom() {
    let (mut s, [_, b, c]) = session(8);
    lock(&mut s, c);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], b.0);
    assert_eq!(layer(&s, c).blend, BlendMode::Normal);

    // The Background of a photo is locked: still the layer underneath.
    let (mut s, [a, _, c]) = session(8);
    lock(&mut s, a);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], c.0);
    assert_eq!(layer(&s, a).blend, BlendMode::Normal);

    // Every upper layer locked: greyed.
    let (mut s, [_, b, c]) = session(8);
    lock(&mut s, b);
    lock(&mut s, c);
    assert!(s.disabled_reason(CMD).is_some());
    assert!(s.execute(CMD, json!({})).is_err());
}

#[test]
fn the_active_artboard_limits_the_layers() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 100, "height": 80, "background": "transparent"})).unwrap();
    let a = s.active().unwrap().active_layer.unwrap();
    let b = id(s.execute("layer.new.layer", json!({"name": "B"})).unwrap());
    s.execute("layer.new.artboard", json!({"rect": [0, 0, 50, 50]})).unwrap();
    let p = id(s.execute("layer.new.layer", json!({"name": "P"})).unwrap());
    let q = id(s.execute("layer.new.layer", json!({"name": "Q"})).unwrap());
    let board = doc(&s).path_of(p).unwrap()[0];
    assert_eq!(doc(&s).path_of(q).unwrap()[0], board, "P and Q are in the artboard");
    assert_ne!(doc(&s).path_of(b).unwrap()[0], board);

    // P is selected inside the artboard, where it is the lowest layer: Q instead, though P has
    // layers below it in the document.
    select(&mut s, p);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], q.0);
    assert_eq!(layer(&s, p).blend, BlendMode::Normal);

    // Outside the artboard the whole document counts.
    select(&mut s, b);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], b.0);
    select(&mut s, a);
    assert_eq!(s.execute(CMD, json!({})).unwrap()["layer"], q.0, "the topmost of the document");
}

#[test]
fn an_artboard_with_one_pixel_layer_is_greyed_while_it_is_active() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 100, "height": 80, "background": "transparent"})).unwrap();
    let a = s.active().unwrap().active_layer.unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.execute("layer.new.artboard", json!({"rect": [0, 0, 50, 50]})).unwrap();
    let p = id(s.execute("layer.new.layer", json!({"name": "P"})).unwrap());
    assert!(s.disabled_reason(CMD).is_some(), "one layer inside the active artboard");
    select(&mut s, a);
    assert!(s.disabled_reason(CMD).is_none(), "outside it the document's layers count");
    let _ = p;
}
