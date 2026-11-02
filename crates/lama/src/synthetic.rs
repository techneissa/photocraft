//! A tiny generator with deterministic weights, for tests that need a model without the real 206 MB file
//! (here, in `photocraft-ml` and in the engine: they enable the `synthetic` feature in their dev-dependencies).
//! Its output is arbitrary but finite, in `0..1` and the same on every run. Compiled only for tests and the
//! `synthetic` feature, never in the app, so it may unwrap.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use candle_core::{Device, Tensor};

use crate::Lama;

/// A deterministic small value for weight number `i` of tensor `salt`.
fn value(salt: usize, i: usize) -> f32 {
    (((i * 2654435761usize + salt * 40503) % 1000) as f32 / 1000.0 - 0.5) * 0.2
}

/// `(name, shape)` of every tensor of a generator with base width `ngf` and `blocks` residual blocks.
pub fn layout(ngf: usize, blocks: usize) -> Vec<(String, Vec<usize>)> {
    let widths = [ngf, ngf * 2, ngf * 4, ngf * 8];
    let (g, l) = (widths[3] * 3 / 4, widths[3] / 4);
    let mut v: Vec<(String, Vec<usize>)> = Vec::new();
    let bn = |v: &mut Vec<(String, Vec<usize>)>, p: &str, c: usize| {
        for n in ["weight", "bias", "running_mean", "running_var"] {
            v.push((format!("{p}.{n}"), vec![c]));
        }
    };
    v.push(("1.ffc.convl2l.weight".into(), vec![ngf, 4, 7, 7]));
    bn(&mut v, "1.bn_l", ngf);
    for i in 0..3 {
        let p = i + 2;
        if i < 2 {
            v.push((format!("{p}.ffc.convl2l.weight"), vec![widths[i + 1], widths[i], 3, 3]));
            bn(&mut v, &format!("{p}.bn_l"), widths[i + 1]);
        } else {
            v.push((format!("{p}.ffc.convl2l.weight"), vec![l, widths[i], 3, 3]));
            v.push((format!("{p}.ffc.convl2g.weight"), vec![g, widths[i], 3, 3]));
            bn(&mut v, &format!("{p}.bn_l"), l);
            bn(&mut v, &format!("{p}.bn_g"), g);
        }
    }
    for b in 0..blocks {
        for c in ["conv1", "conv2"] {
            let p = format!("{}.{c}", 5 + b);
            v.push((format!("{p}.ffc.convl2l.weight"), vec![l, l, 3, 3]));
            v.push((format!("{p}.ffc.convl2g.weight"), vec![g, l, 3, 3]));
            v.push((format!("{p}.ffc.convg2l.weight"), vec![l, g, 3, 3]));
            v.push((format!("{p}.ffc.convg2g.conv1.0.weight"), vec![g / 2, g, 1, 1]));
            bn(&mut v, &format!("{p}.ffc.convg2g.conv1.1"), g / 2);
            v.push((format!("{p}.ffc.convg2g.conv2.weight"), vec![g, g / 2, 1, 1]));
            v.push((format!("{p}.ffc.convg2g.fu.conv_layer.weight"), vec![g, g, 1, 1]));
            bn(&mut v, &format!("{p}.ffc.convg2g.fu.bn"), g);
            bn(&mut v, &format!("{p}.bn_l"), l);
            bn(&mut v, &format!("{p}.bn_g"), g);
        }
    }
    let at = 5 + blocks + 1;
    for i in 0..3 {
        let (inp, out) = (widths[3 - i], widths[2 - i]);
        v.push((format!("{}.weight", at + 3 * i), vec![inp, out, 3, 3]));
        v.push((format!("{}.bias", at + 3 * i), vec![out]));
        bn(&mut v, &format!("{}", at + 3 * i + 1), out);
    }
    v.push((format!("{}.weight", at + 10), vec![3, ngf, 7, 7]));
    v.push((format!("{}.bias", at + 10), vec![3]));
    v
}

pub fn synthetic(ngf: usize, blocks: usize) -> BTreeMap<String, Tensor> {
    layout(ngf, blocks)
        .into_iter()
        .enumerate()
        .map(|(salt, (name, shape))| {
            let n: usize = shape.iter().product();
            let bn_stat = |s: &str| name.ends_with(s);
            let data: Vec<f32> = (0..n)
                .map(|i| {
                    if bn_stat(".running_var") || (bn_stat(".weight") && shape.len() == 1) {
                        1.0
                    } else if bn_stat(".running_mean") || (bn_stat(".bias") && name.contains("bn")) {
                        0.0
                    } else {
                        value(salt, i)
                    }
                })
                .collect();
            (name, Tensor::from_vec(data, shape, &Device::Cpu).unwrap())
        })
        .collect()
}

/// A loaded tiny model (base width 8, two residual blocks) on the CPU.
pub fn tiny_model() -> Lama {
    Lama::from_tensors(synthetic(8, 2), &Device::Cpu).expect("the synthetic layout is a valid generator")
}
