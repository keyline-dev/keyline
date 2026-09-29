//! Intrinsic sizes of unsized layers, lines and rotation.

use crate::*;
use serde_json::json;

#[test]
fn unsized_rects_and_frames_default_to_100() {
    let s =
        scene(json!([{"id": "r", "type": "rect"}, {"id": "f", "type": "frame", "x": 5, "y": 5}]));
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "r", (0.0, 0.0, 100.0, 100.0));
    check(&b, "f", (5.0, 5.0, 100.0, 100.0));
}

#[test]
fn images_take_their_asset_size_and_aspect() {
    let s = scene(json!([
        {"id": "full", "type": "image", "asset": "img"},
        {"id": "w", "type": "image", "asset": "img", "width": 100},
        {"id": "h", "type": "image", "asset": "img", "height": 100},
        {"id": "both", "type": "image", "asset": "img", "width": 50, "height": 70},
    ]));
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "full", (0.0, 0.0, 400.0, 200.0));
    check(&b, "w", (0.0, 0.0, 100.0, 50.0));
    check(&b, "h", (0.0, 0.0, 200.0, 100.0));
    check(&b, "both", (0.0, 0.0, 50.0, 70.0));
}

#[test]
fn image_intrinsic_sizes_follow_the_scale_tool() {
    let s = scene(json!([{"id": "w", "type": "image", "asset": "img", "width": 100}]));
    check(
        &boxes(&s, &size("s", 500.0, 250.0, 0.5)),
        "w",
        (0.0, 0.0, 50.0, 25.0),
    );
}

#[test]
fn icons_are_24_tall_by_default_and_keep_their_aspect() {
    let a = keyline_mcp::icons::aspect(keyline_mcp::scene::IconSet::Lucide, "mail").unwrap();
    let s = scene(json!([
        {"id": "i", "type": "icon", "name": "mail"},
        {"id": "w", "type": "icon", "name": "mail", "width": 48},
        {"id": "h", "type": "icon", "name": "mail", "height": 48},
    ]));
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "i", (0.0, 0.0, 24.0 * a, 24.0));
    check(&b, "w", (0.0, 0.0, 48.0, 48.0 / a));
    check(&b, "h", (0.0, 0.0, 48.0 * a, 48.0));
}

#[test]
fn a_line_missing_a_side_has_zero_length_on_it() {
    let s = scene(json!([
        {"id": "hz", "type": "line", "x": 10, "y": 20, "width": 300},
        {"id": "vt", "type": "line", "x": 10, "y": 20, "height": 300},
    ]));
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "hz", (10.0, 20.0, 300.0, 0.0));
    check(&b, "vt", (10.0, 20.0, 0.0, 300.0));
}

#[test]
fn a_stretched_line_grows_with_its_parent() {
    let s = scene(
        json!([{"id": "l", "type": "line", "x": 100, "y": 250, "width": 800, "constraints": {"horizontal": "stretch", "vertical": "center"}}]),
    );
    check(
        &boxes(&s, &size("s", 1200.0, 600.0, 1.0)),
        "l",
        (100.0, 300.0, 1000.0, 0.0),
    );
}

#[test]
fn rotation_never_changes_the_layout_box() {
    let s = scene(
        json!([{"id": "r", "type": "rect", "x": 100, "y": 50, "width": 200, "height": 100, "rotate": 45}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "r",
        (100.0, 50.0, 200.0, 100.0),
    );
}
