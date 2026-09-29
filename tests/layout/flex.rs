//! Stacks: fill and grow, spacers, stretch and alignSelf, around,
//! padding and gap shorthands, wrap, reverse, absolute children, priority,
//! and baselines.

use crate::*;
use keyline_mcp::text::Text;
use serde_json::{Value, json};

/// A 500 × 100 row at 0, 0 with the given stack fields and children.
fn row(stack: Value, children: Value) -> keyline_mcp::scene::Scene {
    let mut f = json!({"id": "f", "type": "frame", "width": 500, "height": 100,
        "flexDirection": "row", "alignItems": "flex-start"});
    if let Value::Object(m) = stack {
        for (k, v) in m {
            f[k] = v;
        }
    }
    f["children"] = children;
    scene(Value::Array(vec![f]))
}

fn rect(id: &str, w: Value, h: Value) -> Value {
    let mut r = json!({"id": id, "type": "rect"});
    r["width"] = w;
    r["height"] = h;
    r
}

fn at_master(
    s: &keyline_mcp::scene::Scene,
) -> std::collections::BTreeMap<String, keyline_mcp::layout::Rect> {
    boxes(s, &size("s", 1000.0, 500.0, 1.0))
}

#[test]
fn fill_children_share_the_free_space_by_grow() {
    let mut c = rect("c", json!("fill"), json!(20));
    c["flexGrow"] = json!(2);
    let s = row(
        json!({"gap": 10}),
        json!([
            rect("a", json!(100), json!(20)),
            rect("b", json!("fill"), json!(20)),
            c
        ]),
    );
    let b = at_master(&s);
    // 500 − 100 − 2 × 10 = 380 free: b gets 1/3, c 2/3.
    check(&b, "a", (0.0, 0.0, 100.0, 20.0));
    check(&b, "b", (110.0, 0.0, 380.0 / 3.0, 20.0));
    check(&b, "c", (120.0 + 380.0 / 3.0, 0.0, 760.0 / 3.0, 20.0));
}

#[test]
fn fill_stops_at_max_width_and_passes_the_rest_on() {
    let mut b = rect("b", json!("fill"), json!(20));
    b["maxWidth"] = json!(100);
    let s = row(
        json!({"gap": 10}),
        json!([
            rect("a", json!(100), json!(20)),
            b,
            rect("c", json!("fill"), json!(20))
        ]),
    );
    let bx = at_master(&s);
    check(&bx, "b", (110.0, 0.0, 100.0, 20.0));
    check(&bx, "c", (220.0, 0.0, 280.0, 20.0));
}

#[test]
fn a_stretched_row_gives_its_fill_child_the_growth() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 1000, "height": 50, "constraints": {"horizontal": "stretch"}, "flexDirection": "row", "alignItems": "flex-start", "children": [{"id": "logo", "type": "rect", "width": 100, "height": 50}, {"id": "title", "type": "rect", "width": "fill", "height": 50}]}]),
    );
    check(
        &boxes(&s, &size("s", 1000.0, 500.0, 1.0)),
        "title",
        (100.0, 0.0, 900.0, 50.0),
    );
    check(
        &boxes(&s, &size("s", 1600.0, 500.0, 1.0)),
        "title",
        (100.0, 0.0, 1500.0, 50.0),
    );
}

#[test]
fn a_spacer_pushes_the_rest_to_the_end() {
    let s = row(
        json!({"gap": 10}),
        json!([rect("a", json!(100), json!(20)), {"id": "sp", "type": "spacer"}, rect("b", json!(100), json!(20))]),
    );
    let b = at_master(&s);
    check(&b, "b", (400.0, 0.0, 100.0, 20.0));
    check(&b, "sp", (110.0, 0.0, 280.0, 0.0));
}

#[test]
fn a_spacer_keeps_its_min_length() {
    let s = row(
        json!({}),
        json!([rect("a", json!(300), json!(20)), {"id": "sp", "type": "spacer", "minLength": 40}, rect("b", json!(300), json!(20))]),
    );
    check(&at_master(&s), "sp", (300.0, 0.0, 40.0, 0.0));
}

#[test]
fn stretch_fills_the_cross_axis_unless_the_size_is_fixed() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "height": 400, "flexDirection": "column", "padding": 10, "gap": 10, "alignItems": "stretch", "children": [{"id": "a", "type": "rect", "height": 50}, {"id": "b", "type": "rect", "width": 100, "height": 50}, {"id": "t", "type": "text", "text": "Stretched text that wraps inside", "fontSize": 30}]}]),
    );
    let b = at_master(&s);
    check(&b, "a", (10.0, 10.0, 280.0, 50.0));
    check(&b, "b", (10.0, 70.0, 100.0, 50.0));
    let t = b["t"];
    assert_eq!((t.x, t.y, t.w), (10.0, 130.0, 280.0));
    assert!(t.h > 60.0, "the text wraps onto 2+ lines: {t:?}");
}

#[test]
fn fill_on_the_cross_axis_is_the_inside() {
    let s = row(
        json!({"padding": [10, 20]}),
        json!([rect("a", json!(50), json!("fill"))]),
    );
    check(&at_master(&s), "a", (20.0, 10.0, 50.0, 80.0));
}

#[test]
fn percentages_in_a_stack_use_its_inside() {
    let s = row(
        json!({"padding": 50}),
        json!([rect("a", json!("25%"), json!("50%"))]),
    );
    check(&at_master(&s), "a", (50.0, 50.0, 100.0, 0.0));
}

#[test]
fn align_self_overrides_the_stack() {
    let mut c = rect("c", json!(50), json!(20));
    c["alignSelf"] = json!("flex-end");
    let s = row(
        json!({"alignItems": "flex-start"}),
        json!([rect("a", json!(50), json!(20)), c]),
    );
    let b = at_master(&s);
    check(&b, "a", (0.0, 0.0, 50.0, 20.0));
    check(&b, "c", (50.0, 80.0, 50.0, 20.0));
}

#[test]
fn justify_around_puts_half_spaces_at_the_edges() {
    let s = row(
        json!({"justifyContent": "space-around"}),
        json!([
            rect("a", json!(100), json!(20)),
            rect("b", json!(100), json!(20)),
            rect("c", json!(100), json!(20))
        ]),
    );
    let b = at_master(&s);
    // 200 free: 200 / 6 at each edge, 200 / 3 between.
    let xs = ["a", "b", "c"].map(|id| b[id].x);
    for (got, want) in xs.iter().zip([200.0 / 6.0, 200.0, 1100.0 / 3.0]) {
        assert!((got - want).abs() < 0.01, "{xs:?}");
    }
}

#[test]
fn padding_takes_one_two_or_four_values() {
    let four = scene(
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "padding": [1, 2, 3, 4], "alignItems": "flex-start", "children": [rect("a", json!(10), json!(10))]}]),
    );
    let b = at_master(&four);
    check(&b, "f", (0.0, 0.0, 16.0, 14.0));
    check(&b, "a", (4.0, 1.0, 10.0, 10.0));
}

#[test]
fn wrap_breaks_lines_and_uses_the_row_gap_between_them() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 250, "flexDirection": "row", "gap": [20, 10], "alignItems": "flex-start", "flexWrap": "wrap", "children": [rect("a", json!(100), json!(50)), rect("b", json!(100), json!(50)), rect("c", json!(100), json!(50))]}]),
    );
    let b = at_master(&s);
    check(&b, "a", (0.0, 0.0, 100.0, 50.0));
    check(&b, "b", (110.0, 0.0, 100.0, 50.0));
    check(&b, "c", (0.0, 70.0, 100.0, 50.0));
    check(&b, "f", (0.0, 0.0, 250.0, 120.0));
}

#[test]
fn wrapped_lines_justify_on_their_own() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "flexDirection": "row", "alignItems": "flex-start", "justifyContent": "center", "flexWrap": "wrap", "children": [rect("a", json!(200), json!(20)), rect("b", json!(200), json!(20))]}]),
    );
    let b = at_master(&s);
    check(&b, "a", (50.0, 0.0, 200.0, 20.0));
    check(&b, "b", (50.0, 20.0, 200.0, 20.0));
}

#[test]
fn reverse_directions_place_the_last_child_first() {
    let s = row(
        json!({"flexDirection": "row-reverse"}),
        json!([
            rect("a", json!(100), json!(20)),
            rect("b", json!(100), json!(20))
        ]),
    );
    let b = at_master(&s);
    check(&b, "b", (0.0, 0.0, 100.0, 20.0));
    check(&b, "a", (100.0, 0.0, 100.0, 20.0));
    let col = scene(
        json!([{"id": "f", "type": "frame", "flexDirection": "column-reverse", "gap": 5, "alignItems": "flex-start", "children": [rect("a", json!(10), json!(10)), rect("b", json!(10), json!(20))]}]),
    );
    let c = at_master(&col);
    check(&c, "b", (0.0, 0.0, 10.0, 20.0));
    check(&c, "a", (0.0, 25.0, 10.0, 10.0));
}

#[test]
fn absolute_children_sit_on_the_frame_outside_the_flow() {
    let mut badge = rect("badge", json!(20), json!(20));
    badge["position"] = json!("absolute");
    badge["place"] = json!("top-right");
    let s = row(
        json!({}),
        json!([
            rect("a", json!(100), json!(100)),
            badge,
            rect("b", json!(100), json!(100))
        ]),
    );
    let b = at_master(&s);
    check(&b, "a", (0.0, 0.0, 100.0, 100.0));
    check(&b, "b", (100.0, 0.0, 100.0, 100.0));
    check(&b, "badge", (480.0, 0.0, 20.0, 20.0));
}

#[test]
fn hidden_children_take_no_space() {
    let mut h = rect("h", json!(100), json!(20));
    h["hidden"] = json!(true);
    let s = row(
        json!({"gap": 10}),
        json!([
            rect("a", json!(100), json!(20)),
            h,
            rect("b", json!(100), json!(20))
        ]),
    );
    let b = at_master(&s);
    check(&b, "b", (110.0, 0.0, 100.0, 20.0));
    assert!(!b.contains_key("h"));
}

#[test]
fn low_priority_gives_way_first() {
    let chip = |id: &str, priority: f32| {
        json!({"id": id,
            "type": "frame",
            "layoutPriority": priority,
            "flexDirection": "row",
            "alignItems": "flex-start",
            "children": [rect(&format!("{id}-in"), json!(200), json!(20))]})
    };
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "height": 20, "flexDirection": "row", "alignItems": "flex-start", "children": [chip("keep", 1.0), chip("give", 0.0)]}]),
    );
    let b = at_master(&s);
    check(&b, "keep", (0.0, 0.0, 200.0, 20.0));
    check(&b, "give", (200.0, 0.0, 100.0, 20.0));
}

#[test]
fn text_in_a_tight_row_wraps_instead_of_overflowing() {
    let s = row(
        json!({}),
        json!([{"id": "t", "type": "text", "text": "Hello layout", "fontSize": 40}, rect("r", json!(340), json!(20))]),
    );
    let b = at_master(&s);
    let t = b["t"];
    assert!((t.w - 160.0).abs() < 0.01, "{t:?}");
    assert!(t.h > 80.0, "two lines: {t:?}");
    check(&b, "r", (160.0, 0.0, 340.0, 20.0));
}

#[test]
fn text_gives_way_only_down_to_its_longest_word() {
    let s = row(
        json!({}),
        json!([{"id": "t", "type": "text", "text": "Hello layout", "fontSize": 40}, rect("r", json!(490), json!(20))]),
    );
    let t = at_master(&s)["t"];
    assert!(t.w > 60.0, "never narrower than a word: {t:?}");
}

#[test]
fn baseline_alignment_lines_up_first_baselines() {
    let s = row(
        json!({"alignItems": "baseline"}),
        json!([{"id": "big", "type": "text", "text": "Big", "fontSize": 60},
               {"id": "small", "type": "text", "text": "small", "fontSize": 20}]),
    );
    let b = at_master(&s);
    let base = |id: &str| {
        let layer: keyline_mcp::scene::Layer = serde_json::from_value(if id == "big" {
            json!({"type": "text", "text": "Big", "fontSize": 60})
        } else {
            json!({"type": "text", "text": "small", "fontSize": 20})
        })
        .unwrap();
        b[id].y + Text::of(&layer, 1.0).unwrap().first_baseline(b[id].w)
    };
    assert!((base("big") - base("small")).abs() < 0.01, "{b:?}");
    assert!(b["small"].y > b["big"].y);
}

#[test]
fn text_filling_a_column_wraps_at_the_column_width() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 300, "flexDirection": "column", "alignItems": "flex-start", "children": [{"id": "t", "type": "text", "text": "A sentence long enough to wrap in this column", "fontSize": 30, "width": "fill"}]}]),
    );
    let b = at_master(&s);
    let t = b["t"];
    assert_eq!(t.w, 300.0);
    assert!(t.h > 70.0, "{t:?}");
    assert!(
        (b["f"].h - t.h).abs() < 0.01,
        "the column hugs the wrapped text: {b:?}"
    );
}

#[test]
fn fill_in_a_hugging_stack_sizes_to_its_content() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "alignItems": "flex-start", "children": [{"id": "a", "type": "rect", "width": "fill", "height": 20}]}]),
    );
    check(&at_master(&s), "f", (0.0, 0.0, 100.0, 20.0));
}

#[test]
fn flex_grow_alone_takes_the_free_space_as_in_css() {
    let mut b = rect("b", json!(null), json!(20));
    b.as_object_mut().unwrap().remove("width");
    b["flexGrow"] = json!(1);
    let s = row(json!({}), json!([rect("a", json!(100), json!(20)), b]));
    check(&at_master(&s), "b", (100.0, 0.0, 400.0, 20.0));
}

#[test]
fn column_text_keeps_its_height_and_the_stack_reports_it() {
    let text = |id: &str| json!({"id": id, "type": "text", "text": "one two three four five six seven", "fontSize": 20});
    let s = scene(
        json!([{"id": "f", "type": "frame", "width": 100, "height": 60,
        "flexDirection": "column", "children": [text("a"), text("b")]}]),
    );
    let d = keyline_mcp::describe::describe(&s, None, true, None).unwrap();
    let line = |id: &str| {
        d.lines()
            .find(|l| l.trim_start().starts_with(&format!("{id} ")))
            .unwrap()
            .to_owned()
    };
    // Neither text is squeezed below its lines, as in CSS; the column says it overflows.
    assert!(!line("a").contains("!overlaps"), "{d}");
    assert!(line("f").contains("!overflow needs 100×"), "{d}");
    // The children it pushes out are that one problem, not one line each.
    assert!(
        !line("b").contains("!clipped") && !line("b").contains("!hidden"),
        "{d}"
    );
}

#[test]
fn an_empty_spacer_is_not_a_hidden_layer() {
    let s = row(
        json!({}),
        json!([rect("a", json!(500), json!(20)), {"id": "sp", "type": "spacer"}]),
    );
    let d = keyline_mcp::describe::describe(&s, None, false, None).unwrap();
    assert_eq!(d, "ok");
}

#[test]
fn a_hugging_row_makes_room_for_a_spacer_minimum() {
    // B5: 10 + 50 + 10, not an over-full 20.
    let s = scene(
        json!([{"id": "f", "type": "frame", "flexDirection": "row", "children": [
        {"id": "a", "type": "rect", "width": 10, "height": 10},
        {"id": "sp", "type": "spacer", "minLength": 50},
        {"id": "b", "type": "rect", "width": 10, "height": 10}]}]),
    );
    let b = at_master(&s);
    check(&b, "f", (0.0, 0.0, 70.0, 10.0));
    check(&b, "b", (60.0, 0.0, 10.0, 10.0));
}

#[test]
fn an_empty_fill_frame_in_a_column_gives_way() {
    let s = scene(
        json!([{"id": "col", "type": "frame", "width": 200, "height": 300, "flexDirection": "column", "children": [
        {"id": "head", "type": "rect", "height": 250},
        {"id": "rest", "type": "frame", "height": "fill"}]}]),
    );
    let b = boxes(&s, &size("s", 1000.0, 600.0, 1.0));
    check(&b, "rest", (0.0, 250.0, 200.0, 50.0));
    let d = keyline_mcp::describe::describe(&s, None, true, None).unwrap();
    assert!(!d.contains("!overflow"), "{d}");
}

#[test]
fn an_overflowing_stack_still_reports_its_absolute_children() {
    let s = scene(
        json!([{"id": "col", "type": "frame", "width": 200, "height": 100, "flexDirection": "column", "children": [
        {"id": "a", "type": "rect", "height": 80},
        {"id": "b", "type": "rect", "height": 80},
        {"id": "off", "type": "text", "text": "gone", "position": "absolute", "x": 5000}]}]),
    );
    let d = keyline_mcp::describe::describe(&s, None, true, None).unwrap();
    assert!(d.contains("!overflow"), "{d}");
    let off = d
        .lines()
        .find(|l| l.trim_start().starts_with("off "))
        .unwrap();
    assert!(off.contains("!hidden"), "{d}");
}
