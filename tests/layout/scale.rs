//! The Scale tool: geometry × scale first, then constraints.

use crate::*;

#[test]
fn scale_applies_before_constraints() {
    // Scaled master 500 × 250 → target 600 × 300: growth 100 × 50.
    let s = size("s", 600.0, 300.0, 0.5);
    check(
        &boxes(&pinned("left", "top"), &s),
        "r",
        (50.0, 25.0, 100.0, 50.0),
    );
    check(
        &boxes(&pinned("right", "bottom"), &s),
        "r",
        (150.0, 75.0, 100.0, 50.0),
    );
    check(
        &boxes(&pinned("center", "center"), &s),
        "r",
        (100.0, 50.0, 100.0, 50.0),
    );
    check(
        &boxes(&pinned("stretch", "stretch"), &s),
        "r",
        (50.0, 25.0, 200.0, 100.0),
    );
    check(
        &boxes(&pinned("scale", "scale"), &s),
        "r",
        (60.0, 30.0, 120.0, 60.0),
    );
}

#[test]
fn scale_alone_shrinks_everything_proportionally() {
    let s = size("s", 250.0, 125.0, 0.25);
    for (h, v) in [
        ("left", "top"),
        ("right", "bottom"),
        ("center", "center"),
        ("stretch", "stretch"),
    ] {
        check(&boxes(&pinned(h, v), &s), "r", (25.0, 12.5, 50.0, 25.0));
    }
}
