//! Constraints when the parent grows or shrinks.

use crate::*;
use serde_json::json;

#[test]
fn left_top_keeps_position_and_size() {
    check(
        &boxes(&pinned("left", "top"), &size("s", 1200.0, 600.0, 1.0)),
        "r",
        (100.0, 50.0, 200.0, 100.0),
    );
}

#[test]
fn right_bottom_keeps_distance_to_far_edges() {
    check(
        &boxes(&pinned("right", "bottom"), &size("s", 1200.0, 600.0, 1.0)),
        "r",
        (300.0, 150.0, 200.0, 100.0),
    );
}

#[test]
fn center_moves_by_half_the_growth() {
    check(
        &boxes(&pinned("center", "center"), &size("s", 1200.0, 600.0, 1.0)),
        "r",
        (200.0, 100.0, 200.0, 100.0),
    );
}

#[test]
fn stretch_grows_by_the_growth() {
    check(
        &boxes(
            &pinned("stretch", "stretch"),
            &size("s", 1200.0, 600.0, 1.0),
        ),
        "r",
        (100.0, 50.0, 400.0, 200.0),
    );
}

#[test]
fn scale_multiplies_position_and_size() {
    check(
        &boxes(&pinned("scale", "scale"), &size("s", 1200.0, 600.0, 1.0)),
        "r",
        (120.0, 60.0, 240.0, 120.0),
    );
}

#[test]
fn mixed_axes_apply_independently() {
    check(
        &boxes(&pinned("right", "stretch"), &size("s", 1200.0, 600.0, 1.0)),
        "r",
        (300.0, 50.0, 200.0, 200.0),
    );
    check(
        &boxes(&pinned("scale", "center"), &size("s", 1200.0, 600.0, 1.0)),
        "r",
        (120.0, 100.0, 240.0, 100.0),
    );
}

#[test]
fn shrinking_moves_right_and_center_pins_back() {
    let s = size("s", 800.0, 400.0, 1.0);
    check(
        &boxes(&pinned("right", "bottom"), &s),
        "r",
        (-100.0, -50.0, 200.0, 100.0),
    );
    check(
        &boxes(&pinned("center", "center"), &s),
        "r",
        (0.0, 0.0, 200.0, 100.0),
    );
    check(
        &boxes(&pinned("scale", "scale"), &s),
        "r",
        (80.0, 40.0, 160.0, 80.0),
    );
}

#[test]
fn stretch_never_goes_below_zero() {
    let s = scene(
        json!([{"id": "r", "type": "rect", "x": 10, "y": 10, "width": 100, "height": 50, "constraints": {"horizontal": "stretch", "vertical": "stretch"}}]),
    );
    check(
        &boxes(&s, &size("s", 800.0, 400.0, 1.0)),
        "r",
        (10.0, 10.0, 0.0, 0.0),
    );
}

#[test]
fn a_size_equal_to_the_master_changes_nothing() {
    for (h, v) in [
        ("left", "top"),
        ("right", "bottom"),
        ("center", "center"),
        ("stretch", "stretch"),
        ("scale", "scale"),
    ] {
        check(
            &boxes(&pinned(h, v), &size("s", 1000.0, 500.0, 1.0)),
            "r",
            (100.0, 50.0, 200.0, 100.0),
        );
    }
}
