//! Frames: children follow their parent's resize.

use crate::*;
use serde_json::json;

fn framed(child_h: &str, child_v: &str) -> Scene {
    scene(
        json!([{"id": "f", "type": "frame", "x": 100, "y": 100, "width": 400, "height": 200,
        "constraints": {"h": "stretch", "v": "stretch"},
        "children": [{"id": "c", "type": "rect", "x": 10, "y": 10, "width": 50, "height": 50,
                      "constraints": {"h": child_h, "v": child_v}}]}]),
    )
}

#[test]
fn children_are_placed_relative_to_their_frame() {
    let b = boxes(&framed("left", "top"), &size("s", 1000.0, 500.0, 1.0));
    check(&b, "f", (100.0, 100.0, 400.0, 200.0));
    check(&b, "c", (110.0, 110.0, 50.0, 50.0));
}

#[test]
fn children_pin_to_their_frame_not_the_canvas() {
    // The frame stretches 400 × 200 → 600 × 300; the child keeps its
    // distance to the frame's right and bottom edges.
    let b = boxes(&framed("right", "bottom"), &size("s", 1200.0, 600.0, 1.0));
    check(&b, "f", (100.0, 100.0, 600.0, 300.0));
    check(&b, "c", (310.0, 210.0, 50.0, 50.0));
}

#[test]
fn children_stretch_and_scale_with_their_frame() {
    let s = size("s", 1200.0, 600.0, 1.0);
    check(
        &boxes(&framed("stretch", "stretch"), &s),
        "c",
        (110.0, 110.0, 250.0, 150.0),
    );
    check(
        &boxes(&framed("scale", "scale"), &s),
        "c",
        (115.0, 115.0, 75.0, 75.0),
    );
    check(
        &boxes(&framed("center", "center"), &s),
        "c",
        (210.0, 160.0, 50.0, 50.0),
    );
}

#[test]
fn a_frame_that_keeps_its_size_keeps_its_children() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 700, "y": 300, "width": 200, "height": 100,
        "constraints": {"h": "right", "v": "bottom"},
        "children": [{"id": "c", "type": "rect", "x": 150, "y": 50, "width": 40, "height": 40,
                      "constraints": {"h": "right", "v": "bottom"}}]}]),
    );
    let b = boxes(&s, &size("s", 1200.0, 600.0, 1.0));
    check(&b, "f", (900.0, 400.0, 200.0, 100.0));
    check(&b, "c", (1050.0, 450.0, 40.0, 40.0));
}

#[test]
fn nesting_three_levels_deep_composes() {
    let s = scene(
        json!([{"id": "a", "type": "frame", "width": 1000, "height": 500,
        "constraints": {"h": "stretch", "v": "stretch"},
        "children": [{"id": "b", "type": "frame", "x": 100, "y": 100, "width": 800, "height": 300,
            "constraints": {"h": "stretch", "v": "stretch"},
            "children": [{"id": "c", "type": "rect", "x": 700, "y": 250, "width": 100, "height": 50,
                          "constraints": {"h": "right", "v": "bottom"}}]}]}]),
    );
    let b = boxes(&s, &size("s", 1400.0, 700.0, 1.0));
    check(&b, "a", (0.0, 0.0, 1400.0, 700.0));
    check(&b, "b", (100.0, 100.0, 1200.0, 500.0));
    check(&b, "c", (1200.0, 550.0, 100.0, 50.0));
}

#[test]
fn nested_frames_scale_with_the_scale_tool() {
    let b = boxes(&framed("left", "top"), &size("s", 500.0, 250.0, 0.5));
    check(&b, "f", (50.0, 50.0, 200.0, 100.0));
    check(&b, "c", (55.0, 55.0, 25.0, 25.0));
}

#[test]
fn children_outside_their_parent_keep_their_position() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 100, "y": 100, "width": 200, "height": 100,
        "children": [{"id": "c", "type": "rect", "x": -50, "y": 150, "width": 30, "height": 30}]}]),
    );
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "c", (50.0, 250.0, 30.0, 30.0));
}
