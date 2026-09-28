//! Tests of the listing, defects, advisories, facts and text report.

use super::*;
use serde_json::json;

fn scene(layers: serde_json::Value) -> Scene {
    let mut v = json!({
        "width": 400, "height": 200,
        "sizes": [{"id": "wide", "width": 400, "height": 200}, {"id": "small", "width": 200, "height": 200, "scale": 0.5}],
        "assets": {"img": {"sha256": "x", "width": 400, "height": 400}},
    });
    v["layers"] = layers;
    serde_json::from_value(v).unwrap()
}

#[test]
fn one_line_per_layer_per_size() {
    let s = scene(json!([
        {"id": "photo", "type": "image", "asset": "img", "width": 400, "height": 200},
        {"id": "bar", "type": "frame", "y": 150, "width": 400, "height": 50, "children": [
            {"id": "r", "type": "rect", "width": 400, "height": 50, "color": "#FF0000"}
        ]}
    ]));
    let d = describe(&s, None, true, None).unwrap();
    assert_eq!(d.lines().count(), 9, "{d}");
    assert!(
        d.starts_with(
            "assets img 400×400\nwide 400×200\n photo image 0,0 400×200 fill crop 50%h\n"
        ),
        "{d}"
    );
    assert!(d.contains("small 200×200\n"), "{d}");
    assert!(d.contains("  r rect 0,150 400×50\n"), "{d}");
    assert_eq!(
        describe(&s, Some("small"), true, None)
            .unwrap()
            .lines()
            .count(),
        5
    );
    assert!(describe(&s, Some("nope"), true, None).is_err());
    // A clean layout costs two characters by default.
    assert_eq!(describe(&s, None, false, None).unwrap(), "ok");
    assert_eq!(warnings(&s, None), None);
}

#[test]
fn warns_on_overflow_clipping_and_hidden_layers() {
    let s = scene(json!([
        {"id": "long", "type": "text", "text": "a very long headline that will not fit", "fontSize": 30, "resize": "fixed", "width": 100, "height": 30},
        {"id": "edge", "type": "text", "text": "edge", "x": 380, "fontSize": 30},
        {"id": "gone", "type": "rect", "x": 500, "width": 10, "height": 10}
    ]));
    let d = describe(&s, Some("wide"), true, None).unwrap();
    assert!(
        d.contains("long text 0,0 100×30 30px") && d.contains("!overflow needs 100×"),
        "{d}"
    );
    assert!(
        d.lines()
            .any(|l| l.starts_with(" edge") && l.contains("!clipped by canvas: right")),
        "{d}"
    );
    assert!(
        d.lines()
            .any(|l| l.starts_with(" gone") && l.ends_with("!hidden")),
        "{d}"
    );
    // By default only the warning lines, each tagged with its size.
    let w = warnings(&s, None).unwrap();
    assert_eq!(w.lines().count(), 6, "{w}");
    assert!(
        w.lines().all(|l| l.contains(" !") || l.contains(" warn ")),
        "{w}"
    );
    assert!(w.contains("small gone rect"), "{w}");
}

#[test]
fn clipped_warning_names_the_frame_and_the_cut() {
    let s = scene(json!([
        {"id": "band", "type": "frame", "y": 10, "width": 400, "height": 40, "children": [
            {"id": "title", "type": "text", "text": "Title", "y": 20, "fontSize": 30}
        ]}
    ]));
    let w = warnings(&s, None).unwrap();
    let line = w.lines().find(|l| l.starts_with("wide title")).unwrap();
    assert!(line.contains("!clipped by band: bottom "), "{line}");
}

#[test]
fn overlapping_text_is_a_defect() {
    let s = scene(json!([
        {"id": "a", "type": "text", "text": "Hello", "fontSize": 30},
        {"id": "b", "type": "text", "text": "World", "x": 40, "y": 10, "fontSize": 30}
    ]));
    let w = warnings(&s, None).unwrap();
    assert!(
        w.contains("wide a text") && w.contains("!overlaps b"),
        "{w}"
    );
    assert!(
        w.contains("wide b text") && w.contains("!overlaps a"),
        "{w}"
    );
}

#[test]
fn tightly_set_lines_only_overlap_if_their_glyphs_do() {
    // "Monica" over "JETHANI": the line boxes overlap by a few pixels of
    // ascent and descent space, the letters don't.
    let s = scene(json!([
        {"id": "first", "type": "text", "text": "Monica", "y": 20, "fontSize": 26, "weight": 800},
        {"id": "last", "type": "text", "text": "JETHANI", "y": 50, "fontSize": 44, "weight": 900}
    ]));
    assert_eq!(warnings(&s, None), None);
}

#[test]
fn small_text_and_upscaling_are_facts_not_warnings() {
    let s = scene(json!([
        // 14 px at master is 7 px in the half-scale size.
        {"id": "fine", "type": "text", "text": "Fine print", "y": 150, "fontSize": 14},
        {"id": "head", "type": "text", "text": "Head", "fontSize": 30},
        // A 400 px image covering 800 px; at half scale it's drawn 1:1.
        {"id": "big", "type": "image", "asset": "img", "y": 100, "width": 800, "height": 100}
    ]));
    assert_eq!(warnings(&s, None), None);
    assert_eq!(
        facts(&s),
        "smallest text: wide 14px (fine), small 7px (fine); upscaled: wide big 2.0x"
    );
    let d = describe(&s, Some("wide"), true, None).unwrap();
    assert!(
        d.contains("big image 0,100 800×100 fill crop 88%h upscaled 2.0x"),
        "{d}"
    );
}

#[test]
fn warns_on_low_contrast_against_what_is_behind() {
    let s: Scene = serde_json::from_value(json!({
        "width": 400, "height": 100, "background": "#FFFFFF",
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [
            {"id": "panel", "type": "rect", "width": 200, "height": 100, "color": "#1B2A5C"},
            // Navy on navy: unreadable. White on navy and navy on white: fine.
            {"id": "dim", "type": "text", "text": "Dim", "x": 10, "y": 10, "fontSize": 16, "color": "#22335F"},
            {"id": "lit", "type": "text", "text": "Lit", "x": 10, "y": 50, "fontSize": 16, "color": "#FFFFFF"},
            {"id": "ink", "type": "text", "text": "Ink", "x": 250, "y": 10, "fontSize": 16, "color": "#1B2A5C"}
        ]
    }))
    .unwrap();
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
    assert_eq!(w.lines().count(), 1, "{w}");
    assert!(
        w.starts_with("a dim text") && w.contains(" warn contrast 1.") && w.contains("(WCAG 4.5)"),
        "{w}"
    );
    // Without an asset store the check is skipped.
    assert_eq!(warnings(&s, None), None);
}

#[test]
fn faint_text_is_judged_as_drawn() {
    let s: Scene = serde_json::from_value(json!({
        "width": 400, "height": 100,
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [
            {"id": "panel", "type": "rect", "width": 400, "height": 100, "color": "#1B2A5C"},
            // White, but at 1/8 alpha, or inside a 10% frame: barely visible.
            {"id": "alpha", "type": "text", "text": "Faint", "x": 10, "y": 10, "color": "#FFFFFF20"},
            {"id": "box", "type": "frame", "x": 200, "width": 200, "height": 100, "opacity": 0.1, "children": [
                {"id": "nested", "type": "text", "text": "Faint", "x": 10, "y": 10, "color": "#FFFFFF"}
            ]},
            {"id": "solid", "type": "text", "text": "Clear", "x": 10, "y": 50, "color": "#FFFFFF"}
        ]
    }))
    .unwrap();
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
    assert_eq!(w.lines().count(), 2, "{w}");
    assert!(
        w.contains("a alpha text") && w.contains("a nested text"),
        "{w}"
    );
}

#[test]
fn text_report_shows_only_wrapped_shrunk_or_cut_text() {
    let s = scene(json!([
        {"id": "plain", "type": "text", "text": "Short", "fontSize": 20},
        {"id": "wrap", "type": "text", "text": "one two three four", "fontSize": 20, "width": 80},
        {"id": "fit", "type": "text", "text": "SHRINK ME", "fontSize": 40, "width": 150, "height": 30, "y": 120}
    ]));
    let r = text_report(&s, &s.sizes[0]);
    assert!(!r.contains("plain"), "{r}");
    assert!(r.contains(" wrap 20px: \"one two\" / "), "{r}");
    assert!(
        r.contains(" fit ") && r.contains("(max 40): \"SHRINK ME\""),
        "{r}"
    );
}

#[test]
fn every_shot_is_checked_not_only_the_one_shown_at_rest() {
    let s = scene(json!([
        {"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1},
         "children": [{"id": "a", "type": "text", "text": "fine", "fontSize": 30}]},
        {"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1},
         "children": [{"id": "b", "type": "text", "text": "a very long headline that will not fit",
                       "fontSize": 30, "resize": "fixed", "width": 100, "height": 30}]},
        {"id": "logo", "type": "text", "text": "logo", "x": 380, "fontSize": 30}
    ]));
    let w = warnings(&s.resolved(), None).unwrap();
    assert!(w.contains("wide b ") && w.contains("!overflow"), "{w}");
    assert_eq!(w.matches("logo").count(), 2, "listed once per size: {w}");
}
