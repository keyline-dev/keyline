//! Baseline rows: the drawn first baselines line up, whatever the text's
//! padding or trim, and a row that hugs is tall enough for what it aligns.

use crate::*;
use keyline_mcp::layout::Placed;
use serde_json::{Value, json};

/// A hugging row at 0, 0 aligned by baseline.
fn row(children: Value) -> keyline_mcp::scene::Scene {
    let mut f =
        json!({"id": "row", "type": "frame", "flexDirection": "row", "alignItems": "baseline"});
    f["children"] = children;
    scene(Value::Array(vec![f]))
}

/// Where the text of layer `id` draws its first baseline, canvas px.
fn drawn_baseline(s: &keyline_mcp::scene::Scene, id: &str) -> f32 {
    with_placed(s, &size("master", 1000.0, 500.0, 1.0), id, |p: &Placed| {
        let (para, _) = p.text.as_ref().expect("a text layer");
        // The glyphs sit on the first line's baseline.
        p.text_origin().1 + para.get_line_metrics()[0].baseline as f32
    })
    .expect("drawn")
}

#[test]
fn a_hugging_baseline_row_is_tall_enough_for_its_descenders() {
    // The icon sits on the baseline by its bottom, so the text hangs below it.
    let s = row(json!([
        {"id": "icon", "type": "rect", "width": 24, "height": 24},
        {"id": "t", "type": "text", "fontSize": 16, "text": "Label gjpq"}
    ]));
    let b = boxes(&s, &size("master", 1000.0, 500.0, 1.0));
    for id in ["icon", "t"] {
        assert!(
            b[id].bottom() <= b["row"].bottom() + 0.5,
            "{id} fits the row: {b:?}"
        );
    }
    let base = drawn_baseline(&s, "t");
    assert!(
        (base - b["icon"].bottom()).abs() < 0.5,
        "baseline {base}: {b:?}"
    );
}

#[test]
fn padded_text_lines_up_by_its_drawn_baseline() {
    let s = row(json!([
        {"id": "a", "type": "text", "fontSize": 32, "text": "Big"},
        {"id": "b", "type": "text", "fontSize": 32, "padding": 20, "text": "Big"}
    ]));
    let (a, b) = (drawn_baseline(&s, "a"), drawn_baseline(&s, "b"));
    assert!((a - b).abs() < 0.5, "baselines {a} and {b}");
}

#[test]
fn cap_trimmed_text_lines_up_by_its_drawn_baseline() {
    let s = row(json!([
        {"id": "a", "type": "text", "fontSize": 32, "text": "Big"},
        {"id": "b", "type": "text", "fontSize": 20, "trim": "cap", "text": "Pill"}
    ]));
    let (a, b) = (drawn_baseline(&s, "a"), drawn_baseline(&s, "b"));
    assert!((a - b).abs() < 0.5, "baselines {a} and {b}");
}
