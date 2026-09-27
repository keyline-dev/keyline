//! Text boxes: auto width, auto height, fixed, scaled, in stacks.

use crate::*;
use serde_json::{Value, json};

fn text(id: &str, extra: Value) -> Value {
    let mut v = json!({"id": id, "type": "text", "text": "Hello layout", "fontSize": 40});
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v
}

#[test]
fn auto_width_text_measures_itself() {
    let s = scene(json!([text("t", json!({"x": 100, "y": 50}))]));
    let r = boxes(&s, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert_eq!((r.x, r.y), (100.0, 50.0));
    assert!(r.w > 150.0 && r.w < 300.0, "{r:?}");
    assert!(r.h > 40.0 && r.h < 60.0, "{r:?}");
}

#[test]
fn auto_width_text_keeps_its_width_when_pinned_right() {
    let s = scene(json!([text(
        "t",
        json!({"x": 100, "y": 50, "constraints": {"h": "right"}})
    )]));
    let a = boxes(&s, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&s, &size("s", 1200.0, 500.0, 1.0))["t"];
    assert_eq!(b.w, a.w);
    assert!((b.x - (a.x + 200.0)).abs() < 0.01, "{a:?} {b:?}");
}

#[test]
fn auto_width_text_stays_centered_when_stretched() {
    let s = scene(json!([text(
        "t",
        json!({"x": 100, "y": 50, "constraints": {"h": "stretch"}})
    )]));
    let a = boxes(&s, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&s, &size("s", 1200.0, 500.0, 1.0))["t"];
    assert_eq!(b.w, a.w);
    assert!((b.x - (a.x + 100.0)).abs() < 0.01, "{a:?} {b:?}");
}

#[test]
fn auto_height_text_wraps_at_its_width_and_grows_down() {
    let one = scene(json!([text("t", json!({"width": 800}))]));
    let many = scene(json!([text(
        "t",
        json!({"width": 200, "text": "Hello layout, this wraps onto several lines"})
    )]));
    let a = boxes(&one, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&many, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert_eq!(a.w, 800.0);
    assert_eq!(b.w, 200.0);
    assert!(b.h > 2.5 * a.h, "{a:?} {b:?}");
}

#[test]
fn auto_height_text_rewraps_when_its_width_stretches() {
    let s = scene(json!([text(
        "t",
        json!({"x": 0, "width": 300, "constraints": {"h": "stretch"},
        "text": "Hello layout, this wraps onto several lines"})
    )]));
    let narrow = boxes(&s, &size("s", 1000.0, 500.0, 1.0))["t"];
    let wide = boxes(&s, &size("s", 1700.0, 500.0, 1.0))["t"];
    assert_eq!(wide.w, 1000.0);
    assert!(wide.h < narrow.h, "{narrow:?} {wide:?}");
}

#[test]
fn a_fixed_text_box_keeps_its_box() {
    let s = scene(json!([text(
        "t",
        json!({"x": 10, "y": 20, "width": 300, "height": 120})
    )]));
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "t",
        (10.0, 20.0, 300.0, 120.0),
    );
}

#[test]
fn text_measures_smaller_under_the_scale_tool() {
    let s = scene(json!([text("t", json!({}))]));
    let full = boxes(&s, &size("s", 1000.0, 500.0, 1.0))["t"];
    let half = boxes(&s, &size("s", 500.0, 250.0, 0.5))["t"];
    assert!(
        (half.w - full.w / 2.0).abs() < full.w * 0.02,
        "{full:?} {half:?}"
    );
    assert!(
        (half.h - full.h / 2.0).abs() < full.h * 0.02,
        "{full:?} {half:?}"
    );
}

#[test]
fn a_hugging_stack_adds_up_its_texts() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "stack": {"dir": "row", "gap": 12, "padding": 6},
        "children": [text("a", json!({})), text("b", json!({"text": "World"}))]}]),
    );
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    let (a, w, f) = (b["a"], b["b"], b["f"]);
    assert!((f.w - (a.w + w.w + 12.0 + 12.0)).abs() < 0.01, "{f:?}");
    assert!((f.h - (a.h.max(w.h) + 12.0)).abs() < 0.01, "{f:?}");
    assert!((w.x - (a.right() + 12.0)).abs() < 0.01, "{a:?} {w:?}");
}

#[test]
fn a_style_can_set_a_text_box() {
    let s = scene_with(
        json!([text("t", json!({"style": "box"}))]),
        json!({"styles": {"box": {"width": 250, "height": 90}}}),
    );
    check(
        &boxes(&s, &size("master", 1000.0, 500.0, 1.0)),
        "t",
        (0.0, 0.0, 250.0, 90.0),
    );
}
