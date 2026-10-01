//! How a stack shares its main axis: `fill` from nothing as CSS `flex: 1`,
//! priorities giving way in turn, children that can't give way, spacing
//! with a gap, and wrapping by the right gap.

use crate::*;
use keyline_mcp::text::Text;
use serde_json::{Value, json};

/// A stack frame at 0, 0 with the given fields and children.
fn stack(fields: Value, children: Value) -> keyline_mcp::scene::Scene {
    let mut f =
        json!({"id": "f", "type": "frame", "flexDirection": "row", "alignItems": "flex-start"});
    if let Value::Object(m) = fields {
        for (k, v) in m {
            f[k] = v;
        }
    }
    f["children"] = children;
    scene(Value::Array(vec![f]))
}

fn rect(id: &str, w: f32, h: f32) -> Value {
    json!({"id": id, "type": "rect", "width": w, "height": h})
}

fn at_master(
    s: &keyline_mcp::scene::Scene,
) -> std::collections::BTreeMap<String, keyline_mcp::layout::Rect> {
    boxes(s, &size("master", 1000.0, 500.0, 1.0))
}

#[test]
fn fill_texts_share_a_row_evenly_whatever_their_length() {
    let s = stack(
        json!({"width": 600, "height": 100}),
        json!([{"id": "a", "type": "text", "width": "fill", "text": "Extraordinarily"},
               {"id": "b", "type": "text", "width": "fill", "text": "Hi"}]),
    );
    let b = at_master(&s);
    assert!(
        (b["a"].w - 300.0).abs() < 0.5 && (b["b"].w - 300.0).abs() < 0.5,
        "{b:?}"
    );
}

#[test]
fn a_fill_text_whose_word_is_wider_than_its_share_keeps_the_word() {
    let long = json!({"id": "a", "type": "text", "width": "fill", "fontSize": 32, "text": "Extraordinarily"});
    let word = Text::of(&serde_json::from_value(long.clone()).unwrap(), 1.0)
        .unwrap()
        .min_width();
    assert!(word > 150.0, "the word must beat an even share: {word}");
    let s = stack(
        json!({"width": 300, "height": 100}),
        json!([long, {"id": "b", "type": "text", "width": "fill", "text": "Hi"}]),
    );
    let b = at_master(&s);
    assert!((b["a"].w - word).abs() < 0.5, "{b:?}");
    assert!((b["b"].w - (300.0 - word)).abs() < 0.5, "{b:?}");
}

#[test]
fn three_priorities_give_way_lowest_first() {
    // Three 200-wide groups in 300: low gives all 200, normal 100, high none.
    let group = |id: &str, p: i32| {
        json!({"id": id, "type": "frame", "layoutPriority": p,
               "children": [{"id": format!("{id}-r"), "type": "rect", "width": 200, "height": 20}]})
    };
    let s = stack(
        json!({"width": 300, "height": 100}),
        json!([group("low", -1), group("mid", 0), group("high", 1)]),
    );
    let b = at_master(&s);
    check(&b, "low", (0.0, 0.0, 0.0, 20.0));
    check(&b, "mid", (0.0, 0.0, 100.0, 20.0));
    check(&b, "high", (100.0, 0.0, 200.0, 20.0));
}

#[test]
fn children_that_cannot_give_way_overflow_and_say_what_they_need() {
    let s = stack(
        json!({"width": 300, "height": 100}),
        json!([rect("a", 200.0, 20.0), rect("b", 200.0, 20.0)]),
    );
    let b = at_master(&s);
    check(&b, "b", (200.0, 0.0, 200.0, 20.0));
    assert_eq!(
        overflow(&s, &size("master", 1000.0, 500.0, 1.0), "f"),
        Some((400.0, 100.0))
    );
}

#[test]
fn space_evenly_and_between_add_to_the_gap() {
    let three = json!([
        rect("a", 50.0, 20.0),
        rect("b", 50.0, 20.0),
        rect("c", 50.0, 20.0)
    ]);
    // 500 − 150 − 2 × 10 = 330 free.
    let evenly = at_master(&stack(
        json!({"width": 500, "height": 100, "gap": 10, "justifyContent": "space-evenly"}),
        three.clone(),
    ));
    check(&evenly, "a", (82.5, 0.0, 50.0, 20.0));
    check(&evenly, "b", (225.0, 0.0, 50.0, 20.0));
    check(&evenly, "c", (367.5, 0.0, 50.0, 20.0));
    let between = at_master(&stack(
        json!({"width": 500, "height": 100, "gap": 10, "justifyContent": "space-between"}),
        three,
    ));
    check(&between, "a", (0.0, 0.0, 50.0, 20.0));
    check(&between, "b", (225.0, 0.0, 50.0, 20.0));
    check(&between, "c", (450.0, 0.0, 50.0, 20.0));
}

#[test]
fn a_wrapped_column_puts_the_row_gap_between_children_and_the_column_gap_between_lines() {
    // 40 + 10 + 40 fits 100; a third wraps to the next column, 20 across.
    let s = stack(
        json!({"width": 200, "height": 100, "flexDirection": "column", "flexWrap": "wrap", "gap": [10, 20]}),
        json!([
            rect("a", 50.0, 40.0),
            rect("b", 50.0, 40.0),
            rect("c", 50.0, 40.0),
            rect("d", 50.0, 40.0)
        ]),
    );
    let b = at_master(&s);
    check(&b, "a", (0.0, 0.0, 50.0, 40.0));
    check(&b, "b", (0.0, 50.0, 50.0, 40.0));
    check(&b, "c", (70.0, 0.0, 50.0, 40.0));
    check(&b, "d", (70.0, 50.0, 50.0, 40.0));
}

#[test]
fn wrapped_lines_center_their_children_on_each_line() {
    // Line 1 is 40 tall (a, b); line 2 holds c alone.
    let s = stack(
        json!({"width": 120, "flexWrap": "wrap", "alignItems": "center"}),
        json!([
            rect("a", 50.0, 20.0),
            rect("b", 50.0, 40.0),
            rect("c", 50.0, 30.0)
        ]),
    );
    let b = at_master(&s);
    check(&b, "a", (0.0, 10.0, 50.0, 20.0));
    check(&b, "b", (50.0, 0.0, 50.0, 40.0));
    check(&b, "c", (0.0, 40.0, 50.0, 30.0));
    check(&b, "f", (0.0, 0.0, 120.0, 70.0));
}

#[test]
fn text_in_a_centred_column_wraps_at_the_column_width() {
    let s = stack(
        json!({"width": 300, "flexDirection": "column", "alignItems": "center"}),
        json!([{"id": "t", "type": "text", "fontSize": 32, "text": "A headline far wider than the column"}]),
    );
    let master = size("master", 1000.0, 500.0, 1.0);
    let b = boxes(&s, &master);
    assert!(b["t"].w <= 300.5 && b["t"].x >= -0.5, "{b:?}");
    assert!(b["t"].h > 60.0, "wraps to more lines: {b:?}");
    assert!((b["f"].h - b["t"].h).abs() < 0.5, "the column grows: {b:?}");
    assert_eq!(overflow(&s, &master, "f"), None);
}

#[test]
fn short_text_in_a_centred_column_keeps_its_own_width() {
    let s = stack(
        json!({"width": 300, "flexDirection": "column", "alignItems": "center"}),
        json!([{"id": "t", "type": "text", "text": "Short"}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    assert!(b["t"].w < 100.0, "{b:?}");
    assert!(
        (b["t"].x - (300.0 - b["t"].w) / 2.0).abs() < 0.5,
        "centred: {b:?}"
    );
}
