//! Per-size changes (`at`).

use crate::*;
use serde_json::{Value, json};

fn with_sizes(layers: Value) -> Scene {
    scene_with(
        layers,
        json!({"sizes": [
            {"id": "big", "width": 1000, "height": 500},
            {"id": "small", "width": 300, "height": 600, "scale": 0.5},
        ]}),
    )
}

#[test]
fn at_changes_one_size_only() {
    let s = with_sizes(
        json!([{"id": "r", "type": "rect", "x": 100, "y": 50, "width": 200, "height": 100, "media": {"small": {"x": 20, "width": 100}}}]),
    );
    check(
        &boxes(&s, &size("big", 1000.0, 500.0, 1.0)),
        "r",
        (100.0, 50.0, 200.0, 100.0),
    );
    // At the small size the changed fields are in master px, then scaled.
    check(
        &boxes(&s, &size("small", 300.0, 600.0, 0.5)),
        "r",
        (10.0, 25.0, 50.0, 50.0),
    );
}

#[test]
fn at_can_switch_a_row_to_a_column() {
    let s = with_sizes(
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "gap": 10, "alignItems": "flex-start", "media": {"small": {"flexDirection": "column", "alignItems": "flex-start"}}, "children": [{"id": "a", "type": "rect", "width": 100, "height": 40}, {"id": "b", "type": "rect", "width": 100, "height": 40}]}]),
    );
    let big = boxes(&s, &size("big", 1000.0, 500.0, 1.0));
    check(&big, "f", (0.0, 0.0, 210.0, 40.0));
    check(&big, "b", (110.0, 0.0, 100.0, 40.0));
    // The merged stack keeps its gap; everything is scaled by 0.5.
    let small = boxes(&s, &size("small", 300.0, 600.0, 0.5));
    check(&small, "f", (0.0, 0.0, 50.0, 45.0));
    check(&small, "b", (0.0, 25.0, 50.0, 20.0));
}

#[test]
fn at_on_a_child_changes_it_inside_its_frame() {
    let s = with_sizes(
        json!([{"id": "f", "type": "frame", "width": 400, "height": 200, "children": [{"id": "c", "type": "rect", "x": 10, "y": 10, "width": 50, "height": 50, "media": {"small": {"y": 100, "constraints": {"horizontal": "right"}}}}]}]),
    );
    check(
        &boxes(&s, &size("big", 1000.0, 500.0, 1.0)),
        "c",
        (10.0, 10.0, 50.0, 50.0),
    );
    check(
        &boxes(&s, &size("small", 300.0, 600.0, 0.5)),
        "c",
        (5.0, 50.0, 25.0, 25.0),
    );
}

#[test]
fn at_can_resize_a_stack_child() {
    let s = with_sizes(
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "width": 100, "height": 40, "media": {"small": {"width": 300}}}, {"id": "b", "type": "rect", "width": 100, "height": 40}]}]),
    );
    let small = boxes(&s, &size("small", 300.0, 600.0, 0.5));
    check(&small, "a", (0.0, 0.0, 150.0, 20.0));
    check(&small, "b", (150.0, 0.0, 50.0, 20.0));
}
