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
        json!([{"id": "g", "type": "frame", "width": 620, "height": 210, "gridTemplateColumns": "repeat(3, 1fr)", "gap": 10, "children": rects(5)}]),
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
        json!([{"id": "g", "type": "frame", "width": 1000, "height": 400, "gridTemplateColumns": "100px 1fr auto 2fr 10%", "gridTemplateRows": "1fr 3fr", "children": [{"id": "a", "type": "rect"}, {"id": "b", "type": "rect"}, {"id": "c", "type": "rect", "width": 70, "height": 30}, {"id": "d", "type": "rect"}, {"id": "e", "type": "rect"}]}]),
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
        json!([{"id": "g", "type": "frame", "width": 900, "height": 400, "gridTemplateColumns": "1fr 1fr 1fr", "gridTemplateRows": "3fr 1fr", "gridTemplateAreas": ["hero hero side", "cta cta side"], "gap": 20, "children": [{"id": "s", "type": "rect", "gridArea": "side"}, {"id": "h", "type": "rect", "gridArea": "hero"}, {"id": "c", "type": "rect", "gridArea": "cta"}]}]),
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
        json!([{"id": "g", "type": "frame", "width": 400, "height": 200, "gridTemplateRows": "1fr 1fr", "gridTemplateAreas": ["a b", "a c"], "children": [{"id": "b", "type": "rect", "gridArea": "b"}, {"id": "a", "type": "rect", "gridArea": "a"}, {"id": "c", "type": "rect", "gridArea": "c"}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "a", (0.0, 0.0, 200.0, 200.0));
    check(&b, "b", (200.0, 0.0, 200.0, 100.0));
    check(&b, "c", (200.0, 100.0, 200.0, 100.0));
}

#[test]
fn cells_and_spans_then_auto_flow_around_them() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 300, "height": 300, "gridTemplateColumns": "repeat(3, 1fr)", "gridTemplateRows": "repeat(3, 1fr)", "children": [{"id": "a", "type": "rect"}, {"id": "big", "type": "rect", "gridRow": "1 / span 2", "gridColumn": "2 / span 2"}, {"id": "b", "type": "rect"}, {"id": "wide", "type": "rect", "gridColumn": "span 3"}]}]),
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
        json!([{"id": "g", "type": "frame", "width": 700, "height": 100, "gridTemplateColumns": "repeat(auto-fill, minmax(160px, 1fr))", "gap": 20, "children": rects(5)}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // (700 + 20) / (160 + 20) = 4 columns of (700 - 60) / 4 = 160.
    check(&b, "c3", (540.0, 0.0, 160.0, 100.0));
    check(&b, "c4", (0.0, 120.0, 160.0, 100.0));
}

#[test]
fn a_grid_without_a_size_hugs_its_tracks_and_padding() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "x": 10, "y": 20, "gridTemplateColumns": "auto auto", "gap": [5, 10], "padding": 8, "children": [{"id": "a", "type": "rect", "width": 50, "height": 30}, {"id": "b", "type": "rect", "width": 80, "height": 20}, {"id": "c", "type": "rect", "width": 60, "height": 40}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // Columns 60 and 80, rows 30 and 40: 8 + 60 + 10 + 80 + 8 = 166 wide,
    // 8 + 30 + 5 + 40 + 8 = 91 tall.
    check(&b, "g", (10.0, 20.0, 166.0, 91.0));
    check(&b, "b", (88.0, 28.0, 80.0, 20.0));
    check(&b, "c", (18.0, 63.0, 60.0, 40.0));
}

#[test]
fn auto_rows_grow_to_wrapped_text() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 400, "gridTemplateColumns": "repeat(2, 1fr)", "children": [{"id": "t", "type": "text", "text": "one two three four five six seven eight", "fontSize": 20}, {"id": "r", "type": "rect"}]}]),
    );
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
        json!([{"id": "g", "type": "frame", "width": "fill", "height": "fill", "gridTemplateColumns": "1fr 1fr", "gridTemplateRows": "1fr 1fr", "gridTemplateAreas": ["a b", "a c"], "media": {"wide": {"gridTemplateColumns": "1fr 1fr 1fr", "gridTemplateRows": "1fr", "gridTemplateAreas": ["a b c"]}}, "children": [{"id": "a", "type": "rect", "gridArea": "a"}, {"id": "b", "type": "rect", "gridArea": "b"}, {"id": "c", "type": "rect", "gridArea": "c"}]}]),
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
        let mut f = json!({"id": "g", "type": "frame", "children": [child]});
        for (k, v) in g.as_object().unwrap() {
            f[k] = v.clone();
        }
        scene(json!([f])).validate().unwrap_err()
    };
    let r = json!({"id": "r", "type": "rect"});
    assert_eq!(
        bad(json!({"gridTemplateAreas": ["a a", "b a"]}), r.clone()),
        "g: grid area a isn't a rectangle"
    );
    assert!(
        bad(
            json!({"gridTemplateAreas": ["a"]}),
            json!({"id": "r", "type": "rect", "gridArea": "x"})
        )
        .contains("r: no grid area x")
    );
    let e =
        serde_json::from_value::<keyline_mcp::scene::Layer>(json!({"type": "rect", "gridRow": 0}))
            .unwrap_err();
    assert!(e.to_string().contains("a line like 2"), "{e}");
}

#[test]
fn min_columns_never_outnumber_the_children() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 1000, "height": 100, "gridTemplateColumns": "repeat(auto-fill, minmax(1px, 1fr))", "children": rects(2)}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    // A 1 px minimum fits 1000 columns; two children get two, 500 each.
    check(&b, "c1", (500.0, 0.0, 500.0, 100.0));
}

#[test]
fn a_lone_grid_column_keeps_the_child_in_that_column() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 300, "height": 200, "gridTemplateColumns": "repeat(3, 1fr)", "children": [
            {"id": "a", "type": "rect", "gridColumn": 3},
            {"id": "b", "type": "rect", "gridColumn": "1 / span 2"},
            {"id": "c", "type": "rect", "gridRow": 2}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "a", (200.0, 0.0, 100.0, 100.0));
    check(&b, "b", (0.0, 0.0, 200.0, 100.0));
    check(&b, "c", (0.0, 100.0, 100.0, 100.0));
}

#[test]
fn an_item_spanning_auto_rows_gets_their_sum() {
    // "today" spans two auto rows beside two 100 px items: the rows grow
    // evenly so it gets its 300 px, as in CSS.
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 400, "gridTemplateColumns": "1fr 1fr", "gap": 10,
        "children": [
            {"id": "today", "type": "rect", "height": 300, "gridRow": "1 / span 2", "gridColumn": "1"},
            {"id": "b", "type": "rect", "height": 100, "gridColumn": "2"},
            {"id": "c", "type": "rect", "height": 100, "gridColumn": "2"}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "today", (0.0, 0.0, 195.0, 300.0));
    check(&b, "g", (0.0, 0.0, 400.0, 300.0));
    // b and c each take their 100 px at the top of a 145 px row.
    assert_eq!(b["c"].y, 155.0);
}

#[test]
fn an_auto_row_is_as_tall_as_its_item_at_the_items_own_width() {
    // The copy is 200 px wide in a 600 px cell: its text wraps at 200, and
    // the auto row holds all of it.
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 600, "gridTemplateColumns": "1fr", "children": [
        {"id": "copy", "type": "frame", "width": 200, "flexDirection": "column", "children": [
            {"id": "t", "type": "text", "text": "A sentence that wraps onto several lines at two hundred", "width": "fill", "fontSize": 24}]}]}]),
    );
    let b = boxes(&s, &s.sizes[0]);
    let (g, copy) = (b["g"], b["copy"]);
    assert!(copy.w <= 200.5, "{copy:?}");
    assert!(
        g.h + 0.5 >= copy.h && copy.h > 60.0,
        "the row holds it: grid {g:?}, copy {copy:?}"
    );
}

#[test]
fn a_grid_whose_rows_outgrow_it_reports_the_size_it_needs() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 200, "height": 100, "gridTemplateColumns": "1fr", "children": [
            {"id": "a", "type": "rect", "height": 80}, {"id": "b", "type": "rect", "height": 80}]}]),
    );
    assert_eq!(
        overflow(&s, &size("master", 1000.0, 500.0, 1.0), "g"),
        Some((200.0, 160.0))
    );
}

#[test]
fn min_columns_fit_exactly_at_the_boundary() {
    let grid = |w: f32| {
        scene(json!([{"id": "g", "type": "frame", "width": w, "gap": 10,
            "gridTemplateColumns": "repeat(auto-fill, minmax(100px, 1fr))", "children": rects(3)}]))
    };
    // 3 × 100 + 2 × 10 = 320 holds three; a pixel less holds two.
    let b = boxes(&grid(320.0), &size("master", 1000.0, 500.0, 1.0));
    check(&b, "c2", (220.0, 0.0, 100.0, 100.0));
    let b = boxes(&grid(319.0), &size("master", 1000.0, 500.0, 1.0));
    check(&b, "c1", (164.5, 0.0, 154.5, 100.0));
    check(&b, "c2", (0.0, 110.0, 154.5, 100.0));
}

#[test]
fn a_row_past_the_template_adds_auto_rows() {
    // Row 2 is empty, so 0 tall: row 3 starts after two gaps.
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 100, "gap": 10, "gridTemplateRows": "50px", "children": [
            {"id": "a", "type": "rect", "height": 30, "gridRow": 3}]}]),
    );
    check(
        &boxes(&s, &size("master", 1000.0, 500.0, 1.0)),
        "a",
        (0.0, 70.0, 100.0, 30.0),
    );
}

#[test]
fn percent_tracks_are_shares_of_the_inside_of_the_padding() {
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 520, "height": 100, "padding": 10, "gridTemplateColumns": "25% 75%", "children": [
            {"id": "a", "type": "rect", "height": 50}, {"id": "b", "type": "rect", "height": 50}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "a", (10.0, 10.0, 125.0, 50.0));
    check(&b, "b", (135.0, 10.0, 375.0, 50.0));
}

#[test]
fn a_hugging_grid_keeps_its_fr_ratio_around_its_content() {
    // One fr must hold 30 (1fr) and 20 (40 in 2fr): 30, so 30 and 60.
    let s = scene(
        json!([{"id": "g", "type": "frame", "gridTemplateColumns": "1fr 2fr", "children": [
            {"id": "a", "type": "rect", "width": 30, "height": 10}, {"id": "b", "type": "rect", "width": 40, "height": 10}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "g", (0.0, 0.0, 90.0, 10.0));
    check(&b, "b", (30.0, 0.0, 40.0, 10.0));
}

#[test]
fn an_fr_column_never_gets_narrower_than_a_px_item_in_it() {
    // An even split would be 150; a's 250 holds, b gets the 50 left.
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 300, "gridTemplateColumns": "1fr 1fr", "children": [
            {"id": "a", "type": "rect", "width": 250, "height": 50}, {"id": "b", "type": "rect", "height": 50}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    check(&b, "a", (0.0, 0.0, 250.0, 50.0));
    check(&b, "b", (250.0, 0.0, 50.0, 50.0));
}

#[test]
fn an_fr_column_is_never_narrower_than_its_longest_word() {
    let word = json!({"id": "a", "type": "text", "fontSize": 40, "text": "Unbelievable"});
    let least = keyline_mcp::text::Text::of(&serde_json::from_value(word.clone()).unwrap(), 1.0)
        .unwrap()
        .min_width();
    assert!(least > 150.0, "the word must beat an even split: {least}");
    let s = scene(
        json!([{"id": "g", "type": "frame", "width": 300, "gridTemplateColumns": "1fr 1fr", "children": [
            word, {"id": "b", "type": "rect", "height": 50}]}]),
    );
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    assert!((b["a"].w - least).abs() < 0.5, "{b:?}");
    assert!((b["b"].x - least).abs() < 0.5, "{b:?}");
}
