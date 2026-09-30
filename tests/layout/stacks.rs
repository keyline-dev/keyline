//! Stacks: order, justify, align, hugging, nesting.

use crate::*;
use serde_json::json;

/// A 500 × 200 row (or 200 × 500 column) at 0, 0 with padding 20, gap 10
/// and three rects: 50 × 30, 60 × 40, 70 × 50.
fn stack(dir: &str, justify: &str, align: &str) -> Scene {
    let (w, h) = if dir == "row" { (500, 200) } else { (200, 500) };
    scene(
        json!([{"id": "f", "type": "frame", "width": w, "height": h, "flexDirection": dir, "gap": 10, "padding": 20, "alignItems": align, "justifyContent": justify, "children": [{"id": "a", "type": "rect", "width": 50, "height": 30}, {"id": "b", "type": "rect", "width": 60, "height": 40}, {"id": "c", "type": "rect", "width": 70, "height": 50}]}]),
    )
}

fn row_xs(justify: &str) -> [f32; 3] {
    let b = boxes(
        &stack("row", justify, "flex-start"),
        &size("s", 1000.0, 500.0, 1.0),
    );
    ["a", "b", "c"].map(|id| b[id].x)
}

#[test]
fn row_justify_places_children_along_the_main_axis() {
    // Inside: 460 wide; children and gaps use 200, leaving 260.
    assert_eq!(row_xs("flex-start"), [20.0, 80.0, 150.0]);
    assert_eq!(row_xs("center"), [150.0, 210.0, 280.0]);
    assert_eq!(row_xs("flex-end"), [280.0, 340.0, 410.0]);
    assert_eq!(row_xs("space-between"), [20.0, 210.0, 410.0]);
    assert_eq!(row_xs("space-evenly"), [85.0, 210.0, 345.0]);
}

#[test]
fn row_children_keep_their_own_size() {
    let b = boxes(
        &stack("row", "flex-start", "flex-start"),
        &size("s", 1000.0, 500.0, 1.0),
    );
    check(&b, "a", (20.0, 20.0, 50.0, 30.0));
    check(&b, "b", (80.0, 20.0, 60.0, 40.0));
    check(&b, "c", (150.0, 20.0, 70.0, 50.0));
}

#[test]
fn row_align_places_children_across() {
    let ys = |align: &str| {
        let b = boxes(
            &stack("row", "flex-start", align),
            &size("s", 1000.0, 500.0, 1.0),
        );
        ["a", "b", "c"].map(|id| b[id].y)
    };
    // Inside: 160 tall.
    assert_eq!(ys("flex-start"), [20.0, 20.0, 20.0]);
    assert_eq!(ys("center"), [85.0, 80.0, 75.0]);
    assert_eq!(ys("flex-end"), [150.0, 140.0, 130.0]);
}

#[test]
fn column_justify_and_align() {
    let get = |justify: &str, align: &str| {
        let b = boxes(
            &stack("column", justify, align),
            &size("s", 1000.0, 500.0, 1.0),
        );
        ["a", "b", "c"].map(|id| (b[id].x, b[id].y))
    };
    assert_eq!(
        get("flex-start", "flex-start"),
        [(20.0, 20.0), (20.0, 60.0), (20.0, 110.0)]
    );
    // Inside: 160 wide, 460 tall; children and gaps use 140, leaving 320.
    assert_eq!(
        get("flex-end", "center"),
        [(75.0, 340.0), (70.0, 380.0), (65.0, 430.0)]
    );
    assert_eq!(
        get("space-between", "flex-end"),
        [(130.0, 20.0), (120.0, 220.0), (110.0, 430.0)]
    );
}

#[test]
fn between_with_one_child_starts_at_the_start() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "height": 100, "flexDirection": "row", "alignItems": "flex-start", "justifyContent": "space-between", "children": [{"id": "a", "type": "rect", "width": 50, "height": 50}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "a",
        (0.0, 0.0, 50.0, 50.0),
    );
}

#[test]
fn stacked_children_ignore_their_x_and_y() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 10, "y": 10, "width": 300, "height": 100, "flexDirection": "row", "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "x": 999, "y": 999, "width": 50, "height": 50, "constraints": {"horizontal": "right", "vertical": "bottom"}}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "a",
        (10.0, 10.0, 50.0, 50.0),
    );
}

#[test]
fn an_unsized_row_hugs_its_children() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 10, "y": 10, "flexDirection": "row", "gap": 10, "padding": 20, "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "width": 50, "height": 30}, {"id": "b", "type": "rect", "width": 60, "height": 40}, {"id": "c", "type": "rect", "width": 70, "height": 50}]}]),
    );
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "f", (10.0, 10.0, 240.0, 90.0));
    check(&b, "a", (30.0, 30.0, 50.0, 30.0));
    check(&b, "c", (160.0, 30.0, 70.0, 50.0));
}

#[test]
fn an_unsized_column_hugs_its_children() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "flexDirection": "column", "gap": 10, "padding": 20, "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "width": 50, "height": 30}, {"id": "b", "type": "rect", "width": 60, "height": 40}, {"id": "c", "type": "rect", "width": 70, "height": 50}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "f",
        (0.0, 0.0, 110.0, 180.0),
    );
}

#[test]
fn a_stack_with_one_side_set_hugs_the_other() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 500, "flexDirection": "row", "padding": 10, "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "width": 50, "height": 30}, {"id": "b", "type": "rect", "width": 50, "height": 80}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "f",
        (0.0, 0.0, 500.0, 100.0),
    );
}

#[test]
fn an_empty_stack_hugs_to_its_padding() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "gap": 10, "padding": 8, "alignItems": "flex-start", "children": []}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "f",
        (0.0, 0.0, 16.0, 16.0),
    );
}

#[test]
fn a_stretched_stack_spreads_children_over_its_new_width() {
    // Full-width row, evenly spaced: at 1000 wide and at 1600 wide.
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 1000, "height": 100, "constraints": {"horizontal": "stretch"}, "flexDirection": "row", "alignItems": "flex-start", "justifyContent": "space-evenly", "children": [{"id": "a", "type": "rect", "width": 100, "height": 100}, {"id": "b", "type": "rect", "width": 100, "height": 100}, {"id": "c", "type": "rect", "width": 100, "height": 100}]}]),
    );
    let xs = |w: f32| {
        let b = boxes(&s, &size("s", w, 500.0, 1.0));
        ["a", "b", "c"].map(|id| b[id].x)
    };
    assert_eq!(xs(1000.0), [175.0, 450.0, 725.0]);
    assert_eq!(xs(1600.0), [325.0, 750.0, 1175.0]);
}

#[test]
fn stacks_scale_gap_padding_and_children() {
    let b = boxes(
        &stack("row", "flex-start", "flex-start"),
        &size("s", 500.0, 250.0, 0.5),
    );
    check(&b, "f", (0.0, 0.0, 250.0, 100.0));
    check(&b, "a", (10.0, 10.0, 25.0, 15.0));
    check(&b, "b", (40.0, 10.0, 30.0, 20.0));
    check(&b, "c", (75.0, 10.0, 35.0, 25.0));
}

#[test]
fn stacks_nest() {
    // A column holding a hugging row and a rect.
    let s = scene(
        json!([{"id": "col", "type": "frame", "x": 100, "y": 100, "width": 400, "height": 300, "flexDirection": "column", "gap": 20, "padding": 10, "alignItems": "center", "children": [{"id": "row", "type": "frame", "flexDirection": "row", "gap": 5, "alignItems": "flex-start", "children": [{"id": "r1", "type": "rect", "width": 40, "height": 40}, {"id": "r2", "type": "rect", "width": 60, "height": 20}]}, {"id": "big", "type": "rect", "width": 200, "height": 100}]}]),
    );
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    // The row hugs to 105 × 40 and is centred in the 380-wide inside.
    check(&b, "row", (247.5, 110.0, 105.0, 40.0));
    check(&b, "r1", (247.5, 110.0, 40.0, 40.0));
    check(&b, "r2", (292.5, 110.0, 60.0, 20.0));
    check(&b, "big", (200.0, 170.0, 200.0, 100.0));
}

#[test]
fn a_free_frame_inside_a_stack_keeps_its_children_relative() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 50, "y": 50, "width": 500, "height": 200, "flexDirection": "row", "gap": 10, "padding": 10, "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "width": 100, "height": 100}, {"id": "card", "type": "frame", "width": 150, "height": 120, "children": [{"id": "badge", "type": "rect", "x": 120, "y": 5, "width": 20, "height": 20, "constraints": {"horizontal": "right"}}]}]}]),
    );
    let b = boxes(&s, &size("s", 1000.0, 500.0, 1.0));
    check(&b, "card", (170.0, 60.0, 150.0, 120.0));
    check(&b, "badge", (290.0, 65.0, 20.0, 20.0));
}

#[test]
fn stacks_inside_constrained_frames_follow_their_frame() {
    let s = scene(
        json!([{"id": "bar", "type": "frame", "x": 0, "y": 400, "width": 1000, "height": 100, "constraints": {"horizontal": "stretch", "vertical": "bottom"}, "flexDirection": "row", "alignItems": "center", "justifyContent": "center", "children": [{"id": "cta", "type": "rect", "width": 200, "height": 60}]}]),
    );
    let b = boxes(&s, &size("s", 1200.0, 700.0, 1.0));
    check(&b, "bar", (0.0, 600.0, 1200.0, 100.0));
    check(&b, "cta", (500.0, 620.0, 200.0, 60.0));
}

#[test]
fn a_hug_stack_capped_by_max_width_measures_its_wrapped_text() {
    let s = scene(
        json!([{"id": "col", "type": "frame", "maxWidth": 700, "flexDirection": "column",
        "alignItems": "stretch", "children": [
        {"id": "name", "type": "text", "text": "Sir Winston", "fontSize": 140, "fontWeight": 800},
        {"id": "age", "type": "text", "text": "7 yrs old, and ready for a sofa of their own.", "fontSize": 36}]}]),
    );
    let b = boxes(&s, &size("s", 1080.0, 1080.0, 1.0));
    let (col, name, age) = (b["col"], b["name"], b["age"]);
    assert_eq!(col.w, 700.0);
    assert!(name.h > 250.0, "two lines of 140 px: {name:?}");
    assert!(
        (col.h - (name.h + age.h)).abs() < 1.0,
        "the column holds both: {col:?} {name:?} {age:?}"
    );
}
