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
        json!({"x": 100, "y": 50, "constraints": {"horizontal": "right"}})
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
        json!({"x": 100, "y": 50, "constraints": {"horizontal": "stretch"}})
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
        json!({"x": 0, "width": 300, "constraints": {"horizontal": "stretch"},
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
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "gap": 12, "padding": 6, "alignItems": "flex-start", "children": [text("a", json!({})), text("b", json!({"text": "World"}))]}]),
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

#[test]
fn padding_adds_to_a_text_box_and_insets_the_text() {
    let plain = scene(json!([text("t", json!({}))]));
    let padded = scene(json!([text("t", json!({"padding": [10, 20]}))]));
    let a = boxes(&plain, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&padded, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert!(
        (b.w - (a.w + 40.0)).abs() < 0.01 && (b.h - (a.h + 20.0)).abs() < 0.01,
        "{a:?} {b:?}"
    );
}

#[test]
fn trim_cap_takes_the_space_above_caps_and_below_the_baseline() {
    let plain = scene(json!([text("t", json!({"text": "HELLO"}))]));
    let trimmed = scene(json!([text("t", json!({"text": "HELLO", "trim": "cap"}))]));
    let a = boxes(&plain, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&trimmed, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert_eq!(a.w, b.w);
    // Inter's caps are about 0.73 of the font size (40 px here).
    assert!(b.h > 26.0 && b.h < 32.0, "{b:?}");
    assert!(a.h > b.h + 10.0, "{a:?} {b:?}");
}

#[test]
fn balanced_text_keeps_its_box_and_line_count() {
    let words = "A headline long enough to wrap onto a second short line";
    let plain = scene(json!([text("t", json!({"width": 700, "text": words}))]));
    let balanced = scene(json!([text(
        "t",
        json!({"width": 700, "text": words, "textWrap": "balance"})
    )]));
    let a = boxes(&plain, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&balanced, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert_eq!((a.w, a.h), (b.w, b.h), "same box, same number of lines");
}

#[test]
fn curved_text_is_taller_by_the_arc() {
    let flat = scene(json!([text("t", json!({"text": "CURVED BADGE TEXT"}))]));
    let curved = scene(json!([text(
        "t",
        json!({"text": "CURVED BADGE TEXT", "curve": 200})
    )]));
    let a = boxes(&flat, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&curved, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert_eq!(a.w, b.w);
    assert!(b.h > a.h + 20.0, "{a:?} {b:?}");
}

#[test]
fn markup_does_not_change_the_measured_text() {
    let plain = scene(json!([text("t", json!({"text": "Proven RESULTS"}))]));
    let marked = scene(json!([text(
        "t",
        json!({"text": "Proven <span style=\"color:#D0202E\">RESULTS</span>"})
    )]));
    let a = boxes(&plain, &size("s", 1000.0, 500.0, 1.0))["t"];
    let b = boxes(&marked, &size("s", 1000.0, 500.0, 1.0))["t"];
    assert_eq!((a.w, a.h), (b.w, b.h));
}

#[test]
fn max_lines_cuts_text_that_only_has_a_width() {
    // B1: two lines and an ellipsis, reported, not five.
    let s = scene(
        json!([{"id": "t", "type": "text", "width": 200, "fontSize": 30, "maxLines": 2,
        "text": "one two three four five six seven eight nine ten eleven twelve"}]),
    );
    let d = keyline_mcp::describe::describe(&s, None, true, None).unwrap();
    let line = d
        .lines()
        .find(|l| l.trim_start().starts_with("t "))
        .unwrap();
    assert!(
        line.contains(" 2L") && line.contains("!truncated at maxLines 2"),
        "{d}"
    );
}

#[test]
fn text_that_needs_more_room_is_measured_with_its_padding() {
    // The needed height is the box's, padding included, so setting it fits.
    let s = scene(
        json!([{"id": "t", "type": "text", "width": 200, "height": 40, "padding": 20, "fontSize": 30,
        "text": "one two three four five six"}]),
    );
    let d = keyline_mcp::describe::describe(&s, None, true, None).unwrap();
    let need: f32 = d
        .split("needs 200×")
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .unwrap()
        .parse()
        .unwrap();
    let mut fixed = s.clone();
    fixed.layers[0].height = Some(keyline_mcp::scene::Length::Px(need));
    let d = keyline_mcp::describe::describe(&fixed, None, true, None).unwrap();
    assert!(!d.contains("!truncated"), "{d}");
}
