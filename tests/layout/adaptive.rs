//! Layouts that adapt by themselves: direction lists, `firstFit`, and
//! per-size changes by aspect class.

use crate::*;
use serde_json::json;

fn cards(n: usize) -> serde_json::Value {
    (0..n)
        .map(|i| json!({"id": format!("c{i}"), "type": "rect", "width": 100, "height": 50}))
        .collect()
}

#[test]
fn a_dir_list_uses_a_row_where_it_fits() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "stack": {"dir": ["row", "column"]}, "children": cards(3)}]),
    );
    let sz = size("s", 1000.0, 500.0, 1.0);
    let b = boxes(&s, &sz);
    assert_eq!(
        ["c0", "c1", "c2"].map(|id| (b[id].x, b[id].y)),
        [(0.0, 0.0), (100.0, 0.0), (200.0, 0.0)]
    );
    check(&b, "f", (0.0, 0.0, 300.0, 50.0));
    assert_eq!(chosen(&s, &sz, "f").as_deref(), Some("row"));
}

#[test]
fn a_dir_list_falls_back_to_a_column_where_the_row_is_too_wide() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "stack": {"dir": ["row", "column"]}, "children": cards(4)}]),
    );
    let sz = size("s", 1000.0, 500.0, 1.0);
    let b = boxes(&s, &sz);
    assert_eq!(
        ["c0", "c3"].map(|id| (b[id].x, b[id].y)),
        [(0.0, 0.0), (0.0, 150.0)]
    );
    check(&b, "f", (0.0, 0.0, 300.0, 200.0));
    assert_eq!(chosen(&s, &sz, "f").as_deref(), Some("column"));
}

#[test]
fn a_dir_list_switches_per_size() {
    // Full width: 3 × 100 fits a 1000-wide master; at 250 wide it doesn't.
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": "fill", "stack": {"dir": ["row", "column"]}, "children": cards(3)}]),
    );
    let wide = size("s", 1000.0, 500.0, 1.0);
    let narrow = size("s", 250.0, 800.0, 1.0);
    assert_eq!(chosen(&s, &wide, "f").as_deref(), Some("row"));
    assert_eq!(chosen(&s, &narrow, "f").as_deref(), Some("column"));
    let b = boxes(&s, &narrow);
    check(&b, "c2", (0.0, 100.0, 100.0, 50.0));
}

#[test]
fn a_single_dir_reports_no_choice() {
    let s =
        scene(json!([{"id": "f", "type": "frame", "stack": {"dir": "row"}, "children": cards(2)}]));
    assert_eq!(chosen(&s, &size("s", 1000.0, 500.0, 1.0), "f"), None);
}

#[test]
fn first_fit_draws_the_first_child_that_fits() {
    let s = scene(
        json!([{"id": "ff", "type": "firstFit", "x": 10, "y": 10, "width": 300, "height": 60, "children": [
            {"id": "wide", "type": "rect", "width": 400, "height": 60},
            {"id": "ok", "type": "rect", "width": 250, "height": 60},
            {"id": "also", "type": "rect", "width": 100, "height": 60},
        ]}]),
    );
    let sz = size("s", 1000.0, 500.0, 1.0);
    let b = boxes(&s, &sz);
    check(&b, "ff", (10.0, 10.0, 300.0, 60.0));
    check(&b, "ok", (10.0, 10.0, 250.0, 60.0));
    assert!(!b.contains_key("wide") && !b.contains_key("also"), "{b:?}");
    assert_eq!(chosen(&s, &sz, "ff").as_deref(), Some("ok"));
}

#[test]
fn first_fit_picks_a_shorter_headline_where_the_long_one_is_too_wide() {
    let s = scene(
        json!([{"id": "ff", "type": "firstFit", "width": "fill", "children": [
            {"id": "long", "type": "text", "text": "A really rather long headline for a banner", "fontSize": 40},
            {"id": "short", "type": "text", "text": "Short headline", "fontSize": 40},
        ]}]),
    );
    assert_eq!(
        chosen(&s, &size("s", 1200.0, 500.0, 1.0), "ff").as_deref(),
        Some("long")
    );
    assert_eq!(
        chosen(&s, &size("s", 400.0, 500.0, 1.0), "ff").as_deref(),
        Some("short")
    );
}

#[test]
fn first_fit_rejects_text_that_would_shrink() {
    let text = |id: &str, t: &str| json!({"id": id, "type": "text", "text": t, "fontSize": 40, "width": "fill", "height": "fill"});
    let s = scene(
        json!([{"id": "ff", "type": "firstFit", "width": 300, "height": 50, "children": [
            text("long", "Much too long for one line here"), text("short", "Fits"),
        ]}]),
    );
    assert_eq!(
        chosen(&s, &size("s", 1000.0, 500.0, 1.0), "ff").as_deref(),
        Some("short")
    );
}

#[test]
fn first_fit_falls_back_to_its_last_child() {
    let s = scene(
        json!([{"id": "ff", "type": "firstFit", "width": 50, "height": 50, "children": [
            {"id": "a", "type": "rect", "width": 400, "height": 60},
            {"id": "b", "type": "rect", "width": 300, "height": 60},
        ]}]),
    );
    assert_eq!(
        chosen(&s, &size("s", 1000.0, 500.0, 1.0), "ff").as_deref(),
        Some("b")
    );
}

#[test]
fn first_fit_skips_stacks_that_would_have_to_squeeze() {
    let row = |id: &str, n: usize| {
        json!({"id": id, "type": "frame", "width": "fill", "stack": {"dir": "row"},
        "children": (0..n).map(|i| json!({"id": format!("{id}{i}"), "type": "rect", "width": 100, "height": 20})).collect::<Vec<_>>()})
    };
    let s = scene(
        json!([{"id": "ff", "type": "firstFit", "width": 250, "children": [row("four", 4), row("two", 2)]}]),
    );
    assert_eq!(
        chosen(&s, &size("s", 1000.0, 500.0, 1.0), "ff").as_deref(),
        Some("two")
    );
}

#[test]
fn at_by_aspect_class_applies_to_every_size_of_that_shape() {
    let s = scene_with(
        json!([{"id": "r", "type": "rect", "width": 100, "height": 100, "at": {"tall": {"hidden": true}, "wide": {"width": 300}}}]),
        json!({"sizes": [{"id": "sky", "width": 160, "height": 600}, {"id": "leader", "width": 728, "height": 90},
                         {"id": "square", "width": 500, "height": 500}]}),
    );
    s.validate().unwrap();
    assert!(!boxes(&s, &size("sky", 160.0, 600.0, 1.0)).contains_key("r"));
    check(
        &boxes(&s, &size("leader", 728.0, 90.0, 1.0)),
        "r",
        (0.0, 0.0, 300.0, 100.0),
    );
    check(
        &boxes(&s, &size("square", 500.0, 500.0, 1.0)),
        "r",
        (0.0, 0.0, 100.0, 100.0),
    );
}

#[test]
fn at_by_size_id_wins_over_its_aspect_class() {
    let s = scene_with(
        json!([{"id": "r", "type": "rect", "width": 100, "height": 100, "at": {"portrait": {"width": 50}, "story": {"width": 70}}}]),
        json!({"sizes": [{"id": "story", "width": 1080, "height": 1920}, {"id": "post", "width": 1080, "height": 1350}]}),
    );
    check(
        &boxes(&s, &size("story", 1080.0, 1920.0, 1.0)),
        "r",
        (0.0, 0.0, 70.0, 100.0),
    );
    check(
        &boxes(&s, &size("post", 1080.0, 1350.0, 1.0)),
        "r",
        (0.0, 0.0, 50.0, 100.0),
    );
}
