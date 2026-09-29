//! Tests of constraints, the Scale tool, stacks and text boxes.

use super::*;
use serde_json::json;

fn scene(layers: serde_json::Value) -> Scene {
    let mut v = json!({
        "width": 1000, "height": 500,
        "sizes": [{"id": "a", "width": 1000, "height": 500}],
        "assets": {"img": {"sha256": "x", "width": 400, "height": 200}},
    });
    v["layers"] = layers;
    serde_json::from_value(v).unwrap()
}

fn size(w: f32, h: f32, scale: f32) -> Size {
    Size {
        id: "t".into(),
        width: w,
        height: h,
        scale,
        safe: [0.0; 4],
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

#[test]
fn axis_follows_constraints() {
    // Parent grows 1000 → 1200 around a 100-wide child at x = 800.
    assert_eq!(
        axis(Pin::Start, 800.0, 100.0, 1000.0, 1200.0),
        (800.0, 100.0)
    );
    assert_eq!(
        axis(Pin::End, 800.0, 100.0, 1000.0, 1200.0),
        (1000.0, 100.0)
    );
    assert_eq!(
        axis(Pin::Center, 800.0, 100.0, 1000.0, 1200.0),
        (900.0, 100.0)
    );
    assert_eq!(
        axis(Pin::Stretch, 800.0, 100.0, 1000.0, 1200.0),
        (800.0, 300.0)
    );
    assert_eq!(
        axis(Pin::Scale, 800.0, 100.0, 1000.0, 1200.0),
        (960.0, 120.0)
    );
    assert_eq!(axis(Pin::Stretch, 0.0, 100.0, 1000.0, 800.0), (0.0, 0.0));
}

#[test]
fn constraints_apply_on_resize() {
    let s = scene(
        json!([{"id": "a", "type": "rect", "x": 900, "y": 400, "width": 100, "height": 100, "constraints": {"horizontal": "right", "vertical": "bottom"}}, {"id": "b", "type": "rect", "x": 0, "y": 0, "width": 1000, "height": 50, "constraints": {"horizontal": "stretch"}}]),
    );
    let p = layout(&s, &size(600.0, 600.0, 1.0));
    assert_eq!(p[0].rect, rect(500.0, 500.0, 100.0, 100.0));
    assert_eq!(p[1].rect, rect(0.0, 0.0, 600.0, 50.0));
}

#[test]
fn scale_applies_before_constraints() {
    let s = scene(
        json!([{"id": "a", "type": "rect", "x": 900, "y": 400, "width": 100, "height": 100, "constraints": {"horizontal": "right", "vertical": "bottom"}}]),
    );
    // Master scaled to 500 × 250, then resized to 300 × 600.
    let p = layout(&s, &size(300.0, 600.0, 0.5));
    assert_eq!(p[0].rect, rect(250.0, 550.0, 50.0, 50.0));
}

#[test]
fn frames_constrain_children_relative_to_themselves() {
    let s = scene(
        json!([{"id": "f", "type": "frame", "x": 0, "y": 400, "width": 1000, "height": 100, "constraints": {"horizontal": "stretch", "vertical": "bottom"}, "children": [{"id": "c", "type": "rect", "x": 450, "y": 25, "width": 100, "height": 50, "constraints": {"horizontal": "center", "vertical": "center"}}]}]),
    );
    let p = layout(&s, &size(1200.0, 600.0, 1.0));
    assert_eq!(p[0].rect, rect(0.0, 500.0, 1200.0, 100.0));
    assert_eq!(p[0].children[0].rect, rect(550.0, 525.0, 100.0, 50.0));
}

#[test]
fn a_column_stack_hugs_and_centers_its_children() {
    let s = scene(
        json!([{"id": "col", "type": "frame", "x": 100, "y": 50, "flexDirection": "column", "gap": 10, "padding": 5, "alignItems": "center", "children": [{"id": "a", "type": "rect", "x": 999, "y": 999, "width": 200, "height": 40}, {"id": "b", "type": "rect", "width": 100, "height": 20}]}]),
    );
    let p = layout(&s, &size(1000.0, 500.0, 1.0));
    // Hugs: widest child + padding, heights + gap + padding.
    assert_eq!(p[0].rect, rect(100.0, 50.0, 210.0, 80.0));
    // x and y of children are ignored.
    assert_eq!(p[0].children[0].rect, rect(105.0, 55.0, 200.0, 40.0));
    assert_eq!(p[0].children[1].rect, rect(155.0, 105.0, 100.0, 20.0));
}

#[test]
fn a_row_stack_spreads_children_and_follows_its_frame() {
    let s = scene(
        json!([{"id": "row", "type": "frame", "width": 1000, "height": 100, "constraints": {"horizontal": "stretch"}, "flexDirection": "row", "alignItems": "flex-end", "justifyContent": "space-evenly", "children": [{"type": "rect", "width": 100, "height": 50}, {"type": "rect", "width": 100, "height": 100}, {"type": "rect", "width": 100, "height": 50}]}]),
    );
    let xs = |p: &[Placed]| {
        p[0].children
            .iter()
            .map(|c| (c.rect.x, c.rect.y))
            .collect::<Vec<_>>()
    };
    // 700 px free, shared by 4 gaps of 175.
    let p = layout(&s, &size(1000.0, 500.0, 1.0));
    assert_eq!(xs(&p), [(175.0, 50.0), (450.0, 0.0), (725.0, 50.0)]);
    // The frame stretches to 1200, so the gaps grow to 225.
    let p = layout(&s, &size(1200.0, 500.0, 1.0));
    assert_eq!(xs(&p), [(225.0, 50.0), (550.0, 0.0), (875.0, 50.0)]);
    let between = scene(
        json!([{"type": "frame", "width": 1000, "height": 100, "flexDirection": "row", "alignItems": "flex-start", "justifyContent": "space-between", "children": [{"type": "rect", "width": 100, "height": 10}, {"type": "rect", "width": 100, "height": 10}]}]),
    );
    let p = layout(&between, &size(1000.0, 500.0, 1.0));
    assert_eq!(p[0].children[1].rect.x, 900.0);
}

#[test]
fn stacks_measure_text_and_scale_gaps() {
    let s = scene(
        json!([{"type": "frame", "flexDirection": "column", "gap": 20, "alignItems": "flex-start", "children": [{"id": "t", "type": "text", "text": "Hi", "fontSize": 40}, {"type": "rect", "width": 10, "height": 10}]}]),
    );
    let p = layout(&s, &size(500.0, 250.0, 0.5));
    let t = p[0].children[0].rect;
    assert!(t.w > 0.0 && t.h > 0.0);
    // The gap scales with the size: 20 × 0.5.
    assert_eq!(p[0].children[1].rect.y, t.h + 10.0);
}

#[test]
fn images_default_to_intrinsic_size_and_keep_aspect() {
    let s = scene(json!([
        {"id": "a", "type": "image", "asset": "img"},
        {"id": "b", "type": "image", "asset": "img", "width": 100}
    ]));
    let p = layout(&s, &size(1000.0, 500.0, 1.0));
    assert_eq!(p[0].rect, rect(0.0, 0.0, 400.0, 200.0));
    assert_eq!(p[1].rect, rect(0.0, 0.0, 100.0, 50.0));
}

#[test]
fn auto_width_text_keeps_width_when_stretched() {
    let s = scene(
        json!([{"id": "t", "type": "text", "text": "Hello", "x": 100, "fontSize": 40, "constraints": {"horizontal": "stretch"}}]),
    );
    let a = layout(&s, &size(1000.0, 500.0, 1.0));
    let b = layout(&s, &size(1400.0, 500.0, 1.0));
    assert_eq!(a[0].rect.w, b[0].rect.w);
    assert_eq!(b[0].rect.x, 300.0);
}

#[test]
fn auto_height_text_rewraps_when_narrowed() {
    let s = scene(
        json!([{"id": "t", "type": "text", "text": "one two three four five six seven", "fontSize": 30, "width": 1000, "constraints": {"horizontal": "stretch"}}]),
    );
    let wide = layout(&s, &size(1000.0, 500.0, 1.0));
    let narrow = layout(&s, &size(300.0, 500.0, 1.0));
    assert_eq!(narrow[0].rect.w, 300.0);
    assert!(
        narrow[0].rect.h >= wide[0].rect.h * 2.0,
        "{} vs {}",
        narrow[0].rect.h,
        wide[0].rect.h
    );
}

#[test]
fn boxed_text_is_centered_vertically_like_uilabel() {
    let s = scene(json!([
        {"id": "fit", "type": "text", "text": "Hi", "fontSize": 20, "width": 200, "height": 100},
        {"id": "auto", "type": "text", "text": "Hi", "fontSize": 20, "y": 300}
    ]));
    let p = layout(&s, &size(1000.0, 500.0, 1.0));
    let h = p[0].text.as_ref().unwrap().0.height();
    assert_eq!(p[0].text_top(), (100.0 - h) / 2.0);
    assert_eq!(p[1].text_top(), 300.0);
}
