//! Reversed stacks: `row-reverse` and `column-reverse` start at the far
//! edge, as in CSS, so `justifyContent` and wrapping mirror a plain stack.

use crate::*;
use serde_json::{Value, json};

/// Rects `a`, `b`, … of 50 × 50.
fn squares(ids: &[&str]) -> Value {
    ids.iter()
        .map(|id| json!({"id": id, "type": "rect", "width": 50, "height": 50}))
        .collect()
}

/// A stack frame at 0, 0 with the given fields and children.
fn stack(fields: Value, children: Value) -> keyline_mcp::scene::Scene {
    let mut f = json!({"id": "f", "type": "frame", "alignItems": "flex-start"});
    if let Value::Object(m) = fields {
        for (k, v) in m {
            f[k] = v;
        }
    }
    f["children"] = children;
    scene(Value::Array(vec![f]))
}

fn at_master(
    s: &keyline_mcp::scene::Scene,
) -> std::collections::BTreeMap<String, keyline_mcp::layout::Rect> {
    boxes(s, &size("master", 1000.0, 500.0, 1.0))
}

#[test]
fn row_reverse_packs_at_the_right() {
    let s = stack(
        json!({"width": 500, "height": 100, "flexDirection": "row-reverse"}),
        squares(&["a", "b"]),
    );
    let b = at_master(&s);
    check(&b, "a", (450.0, 0.0, 50.0, 50.0));
    check(&b, "b", (400.0, 0.0, 50.0, 50.0));
}

#[test]
fn row_reverse_flex_end_packs_at_the_left() {
    let s = stack(
        json!({"width": 500, "height": 100, "flexDirection": "row-reverse", "justifyContent": "flex-end"}),
        squares(&["a", "b"]),
    );
    let b = at_master(&s);
    check(&b, "a", (50.0, 0.0, 50.0, 50.0));
    check(&b, "b", (0.0, 0.0, 50.0, 50.0));
}

#[test]
fn row_reverse_centers_with_its_gap() {
    // 110 used, 390 free: 195 on each side, the first child on the right.
    let s = stack(
        json!({"width": 500, "height": 100, "flexDirection": "row-reverse", "justifyContent": "center", "gap": 10}),
        squares(&["a", "b"]),
    );
    let b = at_master(&s);
    check(&b, "a", (255.0, 0.0, 50.0, 50.0));
    check(&b, "b", (195.0, 0.0, 50.0, 50.0));
}

#[test]
fn column_reverse_packs_at_the_bottom() {
    let s = stack(
        json!({"width": 100, "height": 500, "flexDirection": "column-reverse"}),
        squares(&["a", "b"]),
    );
    let b = at_master(&s);
    check(&b, "a", (0.0, 450.0, 50.0, 50.0));
    check(&b, "b", (0.0, 400.0, 50.0, 50.0));
}

#[test]
fn row_reverse_wraps_in_order_from_the_right() {
    // Three fit on a 170 line: a, b, c from the right, then d alone.
    let s = stack(
        json!({"width": 170, "flexDirection": "row-reverse", "flexWrap": "wrap"}),
        squares(&["a", "b", "c", "d"]),
    );
    let b = at_master(&s);
    check(&b, "a", (120.0, 0.0, 50.0, 50.0));
    check(&b, "b", (70.0, 0.0, 50.0, 50.0));
    check(&b, "c", (20.0, 0.0, 50.0, 50.0));
    check(&b, "d", (120.0, 50.0, 50.0, 50.0));
}
