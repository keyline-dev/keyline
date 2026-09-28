//! Grids: tracks, areas, cells, spans and hugging.

use crate::*;
use serde_json::{Value, json};

fn rects(n: usize) -> Value {
    (0..n)
        .map(|i| json!({"id": format!("c{i}"), "type": "rect"}))
        .collect()
}

#[test]
fn equal_columns_fill_rows_in_order() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 620, "height": 210,
        "grid": {"columns": 3, "gap": 10}, "children": rects(5)}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // 620 - 2 × 10 = 600 → 200 per column; two auto rows share nothing, so
    // each is as tall as its tallest rect (100, the default box).
    check(&b, "c0", (0.0, 0.0, 200.0, 100.0));
    check(&b, "c2", (420.0, 0.0, 200.0, 100.0));
    check(&b, "c3", (0.0, 110.0, 200.0, 100.0));
    check(&b, "c4", (210.0, 110.0, 200.0, 100.0));
}

#[test]
fn px_pct_auto_and_fr_tracks() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 1000, "height": 400,
        "grid": {"columns": "100px 1fr auto 2fr 10%", "rows": "1fr 3fr"},
        "children": [
            {"id": "a", "type": "rect"}, {"id": "b", "type": "rect"},
            {"id": "c", "type": "rect", "width": 70, "height": 30},
            {"id": "d", "type": "rect"}, {"id": "e", "type": "rect"}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // 1000 - 100 - 70 (auto: c's width) - 100 (10%) = 730 → 1fr 243.3, 2fr 486.7.
    check(&b, "a", (0.0, 0.0, 100.0, 100.0));
    check(&b, "b", (100.0, 0.0, 730.0 / 3.0, 100.0));
    // A px-sized child keeps its size at the cell's top-left.
    check(&b, "c", (100.0 + 730.0 / 3.0, 0.0, 70.0, 30.0));
    check(&b, "d", (170.0 + 730.0 / 3.0, 0.0, 1460.0 / 3.0, 100.0));
    check(&b, "e", (900.0, 0.0, 100.0, 100.0));
}

#[test]
fn areas_place_children_by_name() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 900, "height": 400,
        "grid": {"columns": "1fr 1fr 1fr", "rows": "3fr 1fr", "gap": 20,
                 "areas": ["hero hero side", "cta cta side"]},
        "children": [
            {"id": "s", "type": "rect", "area": "side"},
            {"id": "h", "type": "rect", "area": "hero"},
            {"id": "c", "type": "rect", "area": "cta"}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // Columns (900 - 40) / 3; rows 380 split 3:1.
    let col = 860.0 / 3.0;
    check(&b, "h", (0.0, 0.0, col * 2.0 + 20.0, 285.0));
    check(&b, "c", (0.0, 305.0, col * 2.0 + 20.0, 95.0));
    check(&b, "s", (col * 2.0 + 40.0, 0.0, col, 400.0));
}

#[test]
fn areas_alone_give_equal_columns() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 400, "height": 200,
        "grid": {"areas": ["a b", "a c"], "rows": "1fr 1fr"},
        "children": [{"id": "b", "type": "rect", "area": "b"}, {"id": "a", "type": "rect", "area": "a"},
                     {"id": "c", "type": "rect", "area": "c"}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "a", (0.0, 0.0, 200.0, 200.0));
    check(&b, "b", (200.0, 0.0, 200.0, 100.0));
    check(&b, "c", (200.0, 100.0, 200.0, 100.0));
}

#[test]
fn cells_and_spans_then_auto_flow_around_them() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 300, "height": 300,
        "grid": {"columns": 3, "rows": 3},
        "children": [
            {"id": "a", "type": "rect"},
            {"id": "big", "type": "rect", "cell": [1, 2], "span": [2, 2]},
            {"id": "b", "type": "rect"},
            {"id": "wide", "type": "rect", "span": [1, 3]}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "big", (100.0, 0.0, 200.0, 200.0));
    check(&b, "a", (0.0, 0.0, 100.0, 100.0));
    check(&b, "b", (0.0, 100.0, 100.0, 100.0));
    check(&b, "wide", (0.0, 200.0, 300.0, 100.0));
}

#[test]
fn min_columns_fit_as_many_as_the_width_allows() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 700, "height": 100,
        "grid": {"columns": {"min": 160}, "gap": 20}, "children": rects(5)}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // (700 + 20) / (160 + 20) = 4 columns of (700 - 60) / 4 = 160.
    check(&b, "c3", (540.0, 0.0, 160.0, 100.0));
    check(&b, "c4", (0.0, 120.0, 160.0, 100.0));
}

#[test]
fn a_grid_without_a_size_hugs_its_tracks_and_padding() {
    let s = scene(json!([{"id": "g", "type": "frame", "x": 10, "y": 20,
        "grid": {"columns": "auto auto", "gap": [5, 10], "padding": 8},
        "children": [
            {"id": "a", "type": "rect", "width": 50, "height": 30},
            {"id": "b", "type": "rect", "width": 80, "height": 20},
            {"id": "c", "type": "rect", "width": 60, "height": 40}]}]));
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // Columns 60 and 80, rows 30 and 40: 8 + 60 + 10 + 80 + 8 = 166 wide,
    // 8 + 30 + 5 + 40 + 8 = 91 tall.
    check(&b, "g", (10.0, 20.0, 166.0, 91.0));
    check(&b, "b", (88.0, 28.0, 80.0, 20.0));
    check(&b, "c", (18.0, 63.0, 60.0, 40.0));
}

#[test]
fn auto_rows_grow_to_wrapped_text() {
    let s = scene(json!([{"id": "g", "type": "frame", "width": 400,
        "grid": {"columns": 2},
        "children": [
            {"id": "t", "type": "text", "text": "one two three four five six seven eight", "fontSize": 20},
            {"id": "r", "type": "rect"}]}]));
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    let t = b["t"];
    assert_eq!((t.x, t.w), (0.0, 200.0));
    assert!(t.h > 40.0, "the text wraps in its 200 px column: {t:?}");
    // The rect stretches to the row the text made.
    check(&b, "r", (200.0, 0.0, 200.0, t.h));
    check(&b, "g", (0.0, 0.0, 400.0, t.h));
}

#[test]
fn at_rearranges_a_grid_per_size() {
    let s = scene_with(
        json!([{"id": "g", "type": "frame", "width": "fill", "height": "fill",
            "grid": {"columns": "1fr 1fr", "rows": "1fr 1fr", "areas": ["a b", "a c"]},
            "at": {"wide": {"grid": {"columns": "1fr 1fr 1fr", "rows": "1fr", "areas": ["a b c"]}}},
            "children": [{"id": "a", "type": "rect", "area": "a"}, {"id": "b", "type": "rect", "area": "b"},
                         {"id": "c", "type": "rect", "area": "c"}]}]),
        json!({"width": 400, "height": 400, "sizes": [
            {"id": "sq", "width": 400, "height": 400}, {"id": "banner", "width": 900, "height": 300}]}),
    );
    let sq = boxes(&s, &size("sq", 400.0, 400.0, 1.0));
    check(&sq, "c", (200.0, 200.0, 200.0, 200.0));
    let banner = boxes(&s, &size("banner", 900.0, 300.0, 1.0));
    check(&banner, "a", (0.0, 0.0, 300.0, 300.0));
    check(&banner, "c", (600.0, 0.0, 300.0, 300.0));
}

#[test]
fn grid_errors_are_one_line() {
    let bad = |g: Value, child: Value| {
        let s = scene(json!([{"id": "g", "type": "frame", "grid": g, "children": [child]}]));
        s.validate().unwrap_err()
    };
    let r = json!({"id": "r", "type": "rect"});
    assert_eq!(
        bad(json!({"areas": ["a a", "b a"]}), r.clone()),
        "g: grid area a isn't a rectangle"
    );
    assert!(
        bad(
            json!({"areas": ["a"]}),
            json!({"id": "r", "type": "rect", "area": "x"})
        )
        .contains("r: no grid area x")
    );
    assert!(
        bad(
            json!({"columns": 2}),
            json!({"id": "r", "type": "rect", "cell": [0, 1]})
        )
        .contains("count from 1")
    );
}
