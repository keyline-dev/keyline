//! v2 sizing in free layout: `fill`, percentages, min/max, aspect ratio,
//! `place`, `hidden`, and frames that hug their children.

use crate::*;
use serde_json::json;

#[test]
fn fill_takes_the_rest_of_the_parent_from_x() {
    let s = scene(json!([{"id": "r", "type": "rect", "x": 100, "width": "fill", "height": 50}]));
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "r",
        (100.0, 0.0, 900.0, 50.0),
    );
    check(
        &boxes(&s, &size("s", 1200.0, 500.0, 1.0)),
        "r",
        (100.0, 0.0, 1100.0, 50.0),
    );
}

#[test]
fn fill_height_reaches_the_bottom() {
    let s = scene(json!([{"id": "r", "type": "rect", "y": 100, "width": 10, "height": "fill"}]));
    check(
        &boxes(&s, &size("s", 1000.0, 800.0, 1.0)),
        "r",
        (0.0, 100.0, 10.0, 700.0),
    );
}

#[test]
fn percentages_are_a_share_of_the_parent_at_every_size() {
    let s = scene(
        json!([{"id": "r", "type": "rect", "x": "10%", "y": "20%", "width": "50%", "height": "25%"}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "r",
        (100.0, 100.0, 500.0, 125.0),
    );
    check(
        &boxes(&s, &size("s", 2000.0, 1000.0, 1.0)),
        "r",
        (200.0, 200.0, 1000.0, 250.0),
    );
    check(
        &boxes(&s, &size("s", 300.0, 600.0, 0.5)),
        "r",
        (30.0, 120.0, 150.0, 150.0),
    );
}

#[test]
fn percentages_inside_frames_use_the_frame() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 100, "y": 100, "width": 400, "height": 200,
        "children": [{"id": "c", "type": "rect", "x": "50%", "width": "25%", "height": "50%"}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "c",
        (300.0, 100.0, 100.0, 100.0),
    );
}

#[test]
fn min_and_max_clamp_stretched_sizes() {
    let s = scene(
        json!([{"id": "r", "type": "rect", "x": 100, "width": 800, "height": 50,
        "minWidth": 50, "maxWidth": 1000, "constraints": {"h": "stretch"}}]),
    );
    check(
        &boxes(&s, &size("s", 1600.0, 500.0, 1.0)),
        "r",
        (100.0, 0.0, 1000.0, 50.0),
    );
    check(
        &boxes(&s, &size("s", 150.0, 500.0, 1.0)),
        "r",
        (100.0, 0.0, 50.0, 50.0),
    );
}

#[test]
fn min_and_max_clamp_intrinsic_sizes() {
    let s = scene(
        json!([{"id": "i", "type": "image", "asset": "img", "maxWidth": 200, "minHeight": 150}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "i",
        (0.0, 0.0, 200.0, 200.0),
    );
}

#[test]
fn aspect_ratio_derives_the_missing_side() {
    let s = scene(json!([
        {"id": "a", "type": "rect", "width": 300, "aspectRatio": 1.5},
        {"id": "b", "type": "rect", "height": 100, "aspectRatio": 2},
        {"id": "c", "type": "image", "asset": "img", "width": "fill", "aspectRatio": 4},
    ]));
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "a", (0.0, 0.0, 300.0, 200.0));
    check(&b, "b", (0.0, 0.0, 200.0, 100.0));
    check(&b, "c", (0.0, 0.0, 1000.0, 250.0));
}

#[test]
fn place_pins_to_a_spot_at_every_size() {
    let s = scene(json!([
        {"id": "br", "type": "rect", "width": 100, "height": 50, "place": "bottom-right", "inset": 20},
        {"id": "c", "type": "rect", "width": 100, "height": 50, "place": "center"},
        {"id": "t", "type": "rect", "width": 100, "height": 50, "place": "top", "inset": [0, 10]},
        {"id": "l", "type": "rect", "width": 100, "height": 50, "place": "left", "inset": [30, 0]},
    ]));
    let a = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&a, "br", (880.0, 430.0, 100.0, 50.0));
    check(&a, "c", (450.0, 225.0, 100.0, 50.0));
    check(&a, "t", (450.0, 10.0, 100.0, 50.0));
    check(&a, "l", (30.0, 225.0, 100.0, 50.0));
    let b = boxes(&s, &size("s", 1200.0, 600.0, 1.0));
    check(&b, "br", (1080.0, 530.0, 100.0, 50.0));
    check(&b, "c", (550.0, 275.0, 100.0, 50.0));
}

#[test]
fn place_insets_scale_with_the_scale_tool() {
    let s = scene(
        json!([{"id": "br", "type": "rect", "width": 100, "height": 50, "place": "bottom-right", "inset": 20}]),
    );
    check(
        &boxes(&s, &size("s", 600.0, 300.0, 0.5)),
        "br",
        (540.0, 265.0, 50.0, 25.0),
    );
}

#[test]
fn place_works_with_fill_and_percent_sizes() {
    let s = scene(
        json!([{"id": "bar", "type": "rect", "width": "100%", "height": 80, "place": "bottom"}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "bar",
        (0.0, 420.0, 1000.0, 80.0),
    );
    check(
        &boxes(&s, &size("s", 400.0, 900.0, 1.0)),
        "bar",
        (0.0, 820.0, 400.0, 80.0),
    );
}

#[test]
fn hidden_layers_are_not_laid_out() {
    let s =
        scene(json!([{"id": "r", "type": "rect", "hidden": true}, {"id": "v", "type": "rect"}]));
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    assert!(!b.contains_key("r"));
    assert!(b.contains_key("v"));
}

#[test]
fn a_hugging_free_frame_wraps_its_children() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 5, "y": 5, "width": "hug", "height": "hug",
        "children": [{"id": "a", "type": "rect", "x": 10, "y": 20, "width": 50, "height": 50},
                     {"id": "b", "type": "rect", "x": 100, "width": 30, "height": 30},
                     {"id": "h", "type": "rect", "x": 900, "width": 30, "height": 30, "hidden": true}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "f",
        (5.0, 5.0, 130.0, 70.0),
    );
}

#[test]
fn x_and_y_reject_hug_and_fill() {
    let s = scene(json!([{"id": "r", "type": "rect", "x": "fill"}]));
    assert!(
        s.validate()
            .unwrap_err()
            .contains("x must be px or a percentage")
    );
}

#[test]
fn bad_clamps_and_ratios_are_rejected() {
    let bad = |extra: serde_json::Value| {
        let mut l = json!({"id": "r", "type": "rect"});
        for (k, v) in extra.as_object().unwrap() {
            l[k] = v.clone();
        }
        scene(json!([l])).validate().unwrap_err()
    };
    assert!(bad(json!({"minWidth": 50, "maxWidth": 10})).contains("minWidth must be <= maxWidth"));
    assert!(bad(json!({"aspectRatio": 0})).contains("aspectRatio must be > 0"));
    assert!(bad(json!({"inset": 4})).contains("inset needs place"));
    assert!(bad(json!({"width": -5})).contains("width and height must be >= 0"));
}
