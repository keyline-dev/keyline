//! Layout tests: build a scene, lay it out for a size, and check every
//! layer's final box. No rendering; expected values are worked out by hand
//! from the rules (Scale tool first, then constraints against the parent;
//! stacks place children in order).

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod at;
mod constraints;
mod frames;
mod intrinsic;
mod scale;
mod stacks;
mod text;

use std::collections::BTreeMap;

use keyline_mcp::layout::{Placed, Rect, layout};
use keyline_mcp::scene::{Scene, Size};
use serde_json::{Value, json};

/// A scene with a 1000 × 500 master and the given layers; `extra` merges in
/// root fields (sizes, assets).
pub(crate) fn scene_with(layers: Value, extra: Value) -> Scene {
    let mut v = json!({
        "width": 1000, "height": 500,
        "sizes": [{"id": "master", "width": 1000, "height": 500}],
        "assets": {"img": {"sha256": "x", "width": 400, "height": 200}},
    });
    v["layers"] = layers;
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    serde_json::from_value(v).unwrap()
}

pub(crate) fn scene(layers: Value) -> Scene {
    scene_with(layers, json!({}))
}

pub(crate) fn size(id: &str, w: f32, h: f32, scale: f32) -> Size {
    serde_json::from_value(json!({"id": id, "width": w, "height": h, "scale": scale})).unwrap()
}

/// Every layer's box by id, for one size, after styles and per-size changes.
pub(crate) fn boxes(scene: &Scene, size: &Size) -> BTreeMap<String, Rect> {
    fn flat(placed: &[Placed], out: &mut BTreeMap<String, Rect>) {
        for p in placed {
            out.insert(p.layer.id.clone(), p.rect);
            flat(&p.children, out);
        }
    }
    let resolved = scene.resolved();
    let sized = resolved.for_size(&size.id);
    let mut out = BTreeMap::new();
    flat(&layout(&sized, size), &mut out);
    out
}

#[track_caller]
pub(crate) fn check(b: &BTreeMap<String, Rect>, id: &str, want: (f32, f32, f32, f32)) {
    let r = b.get(id).unwrap_or_else(|| panic!("no layer {id}"));
    let got = (r.x, r.y, r.w, r.h);
    let close = |a: f32, b: f32| (a - b).abs() < 0.01;
    assert!(
        close(got.0, want.0)
            && close(got.1, want.1)
            && close(got.2, want.2)
            && close(got.3, want.3),
        "{id}: got {got:?}, want {want:?}"
    );
}

/// A 200 × 100 rect at 100, 50 with the given constraints.
pub(crate) fn pinned(h: &str, v: &str) -> Scene {
    scene(
        json!([{"id": "r", "type": "rect", "x": 100, "y": 50, "width": 200, "height": 100,
                  "constraints": {"h": h, "v": v}}]),
    )
}
