//! Free layers whose size changes with the parent: a `%` or `fill` side
//! keeps the edge it's pinned to, and a side sized by the content follows
//! the new size of the other.

use crate::*;
use serde_json::json;

#[test]
fn a_percent_width_pinned_right_keeps_its_right_margin() {
    // 500 wide at the master, 100 from the right edge.
    let s = scene(
        json!([{"id": "r", "type": "rect", "x": 400, "width": "50%", "height": 100, "constraints": {"horizontal": "right"}}]),
    );
    check(
        &boxes(&s, &size("big", 2000.0, 500.0, 1.0)),
        "r",
        (900.0, 0.0, 1000.0, 100.0),
    );
    check(
        &boxes(&s, &size("small", 600.0, 500.0, 1.0)),
        "r",
        (200.0, 0.0, 300.0, 100.0),
    );
}

#[test]
fn a_percent_width_pinned_center_keeps_its_offset_from_the_center() {
    // Centred at 350, 150 left of the parent's centre.
    let s = scene(
        json!([{"id": "c", "type": "rect", "x": 100, "y": 200, "width": "50%", "height": 100, "constraints": {"horizontal": "center"}}]),
    );
    check(
        &boxes(&s, &size("big", 2000.0, 500.0, 1.0)),
        "c",
        (350.0, 200.0, 1000.0, 100.0),
    );
    check(
        &boxes(&s, &size("small", 600.0, 500.0, 1.0)),
        "c",
        (0.0, 200.0, 300.0, 100.0),
    );
}

#[test]
fn a_percent_height_pinned_bottom_keeps_its_bottom_margin() {
    // 250 tall at the master, 50 from the bottom.
    let s = scene(
        json!([{"id": "r", "type": "rect", "y": 200, "width": 100, "height": "50%", "constraints": {"vertical": "bottom"}}]),
    );
    check(
        &boxes(&s, &size("tall", 1000.0, 1000.0, 1.0)),
        "r",
        (0.0, 450.0, 100.0, 500.0),
    );
    check(
        &boxes(&s, &size("short", 1000.0, 300.0, 1.0)),
        "r",
        (0.0, 100.0, 100.0, 150.0),
    );
}

#[test]
fn a_clamped_percent_width_pinned_right_keeps_its_right_margin() {
    // 1000 at 2000 wide, capped at 600, still 100 from the right edge.
    let s = scene(
        json!([{"id": "r", "type": "rect", "x": 400, "width": "50%", "maxWidth": 600, "height": 100, "constraints": {"horizontal": "right"}}]),
    );
    check(
        &boxes(&s, &size("big", 2000.0, 500.0, 1.0)),
        "r",
        (1300.0, 0.0, 600.0, 100.0),
    );
}

#[test]
fn a_percent_width_image_keeps_its_aspect_at_every_size() {
    // The asset is 400 × 200.
    let s = scene(json!([{"id": "i", "type": "image", "asset": "img", "width": "50%"}]));
    check(
        &boxes(&s, &size("master", 1000.0, 500.0, 1.0)),
        "i",
        (0.0, 0.0, 500.0, 250.0),
    );
    check(
        &boxes(&s, &size("big", 2000.0, 500.0, 1.0)),
        "i",
        (0.0, 0.0, 1000.0, 500.0),
    );
}

#[test]
fn a_percent_width_with_an_aspect_ratio_keeps_it_at_every_size() {
    let s = scene(
        json!([{"id": "r", "type": "rect", "width": "50%", "aspectRatio": 2, "constraints": {"vertical": "top"}}]),
    );
    check(
        &boxes(&s, &size("big", 2000.0, 500.0, 1.0)),
        "r",
        (0.0, 0.0, 1000.0, 500.0),
    );
}

#[test]
fn a_fill_width_column_grows_down_with_its_rewrapped_text() {
    let s = scene(
        json!([{"id": "col", "type": "frame", "x": 40, "y": 40, "width": "fill", "flexDirection": "column", "children": [
            {"id": "t", "type": "text", "fontSize": 40, "text": "A long paragraph of copy that wraps onto several lines when the frame is narrow enough"}]}]),
    );
    let master = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    let narrow = size("narrow", 400.0, 500.0, 1.0);
    let b = boxes(&s, &narrow);
    assert!(b["t"].h > master["t"].h, "rewraps: {b:?}");
    assert!((b["col"].h - b["t"].h).abs() < 0.5, "hugs its text: {b:?}");
    assert_eq!(overflow(&s, &narrow, "col"), None);
}
