//! Tests of text runs, sizing modes, fitting and fonts.

use super::*;
use serde_json::json;

fn text_layer(v: serde_json::Value) -> Layer {
    serde_json::from_value(v).unwrap()
}

#[test]
fn color_runs_split_on_character_ranges() {
    let red = Color(0xFFFF_0000);
    let black = Color(0xFF00_0000);
    let ranges = [Range {
        start: 1,
        end: 3,
        color: red,
    }];
    let runs = color_runs("héllo", &ranges, black);
    assert_eq!(runs, vec![("h", black), ("él", red), ("lo", black)]);
    assert_eq!(color_runs("", &ranges, black), vec![]);
}

#[test]
fn auto_width_grows_with_text_and_scale() {
    let short = text_layer(json!({"type": "text", "text": "Hi", "fontSize": 40}));
    let long = text_layer(json!({"type": "text", "text": "Hello world", "fontSize": 40}));
    let (ws, hs) = Text::of(&short, 1.0).unwrap().natural_size(0.0, 0.0);
    let (wl, hl) = Text::of(&long, 1.0).unwrap().natural_size(0.0, 0.0);
    assert!(wl > ws * 2.0, "{wl} vs {ws}");
    assert_eq!(hs, hl);
    let (wh, hh) = Text::of(&long, 0.5).unwrap().natural_size(0.0, 0.0);
    assert!((wh - wl / 2.0).abs() < 2.0 && hh < hl);
}

#[test]
fn auto_height_wraps() {
    let l = text_layer(
        json!({"type": "text", "text": "one two three four five six", "fontSize": 20, "resize": "auto-height", "width": 80}),
    );
    let t = Text::of(&l, 1.0).unwrap();
    let (w, h) = t.natural_size(80.0, 0.0);
    assert_eq!(w, 80.0);
    let (_, fit) = t.layout(w, h);
    assert!(fit.lines > 1 && !fit.overflow);
}

#[test]
fn fixed_reports_overflow_and_truncate_ellipsizes() {
    let text = "one two three four five six seven eight";
    let fixed = text_layer(
        json!({"type": "text", "text": text, "fontSize": 20, "resize": "fixed", "width": 80, "height": 30}),
    );
    let (_, fit) = Text::of(&fixed, 1.0).unwrap().layout(80.0, 30.0);
    assert!(fit.overflow && !fit.truncated);
    // The server says what would fit, so the agent needn't guess.
    assert!(
        fit.need_height > 30.0 && fit.one_line_width > 80.0,
        "{fit:?}"
    );
    let (_, refit) = Text::of(&fixed, 1.0).unwrap().layout(80.0, fit.need_height);
    assert!(!refit.overflow, "{refit:?}");

    let trunc = text_layer(
        json!({"type": "text", "text": text, "fontSize": 20, "resize": "truncate", "width": 80, "height": 30}),
    );
    let (_, fit) = Text::of(&trunc, 1.0).unwrap().layout(80.0, 30.0);
    assert!(fit.truncated && !fit.overflow && fit.lines == 1, "{fit:?}");
}

#[test]
fn max_lines_never_keeps_more_lines_than_fit() {
    let text = "one two three four five six seven eight";
    let trunc = text_layer(
        json!({"type": "text", "text": text, "fontSize": 20, "resize": "truncate", "maxLines": 5, "width": 80, "height": 30}),
    );
    let (_, fit) = Text::of(&trunc, 1.0).unwrap().layout(80.0, 30.0);
    assert!(fit.truncated && !fit.overflow && fit.lines == 1, "{fit:?}");
    let tiny = text_layer(
        json!({"type": "text", "text": text, "fontSize": 40, "minFontScale": 0.9, "maxLines": 5, "width": 200, "height": 50}),
    );
    let (_, fit) = Text::of(&tiny, 1.0).unwrap().layout(200.0, 50.0);
    assert!(fit.truncated && !fit.overflow, "{fit:?}");
}

#[test]
fn fit_shrinks_the_font_like_uilabel() {
    let text = "one two three four five six seven eight";
    // Box given, no resize: fit. Fits at the requested size → unchanged.
    let roomy = text_layer(
        json!({"type": "text", "text": "Hi", "fontSize": 20, "width": 200, "height": 40}),
    );
    let (_, fit) = Text::of(&roomy, 1.0).unwrap().layout(200.0, 40.0);
    assert_eq!(
        (fit.font_size, fit.overflow, fit.truncated),
        (20.0, false, false)
    );

    // Too big → shrinks until it fits, staying above the minimum.
    let tight = text_layer(
        json!({"type": "text", "text": text, "fontSize": 40, "width": 200, "height": 100}),
    );
    let (p, fit) = Text::of(&tight, 1.0).unwrap().layout(200.0, 100.0);
    assert!(fit.font_size < 40.0 && fit.font_size >= 20.0, "{fit:?}");
    assert!(
        !fit.overflow && !fit.truncated && p.height() <= 100.5,
        "{fit:?}"
    );
    // The Scale tool scales the maximum too.
    let (_, half) = Text::of(&tight, 0.5).unwrap().layout(100.0, 50.0);
    assert!(half.font_size <= 20.0, "{half:?}");

    // Still too big at minFontScale → minimum size, then an ellipsis.
    let tiny = text_layer(
        json!({"type": "text", "text": text, "fontSize": 40, "minFontScale": 0.9, "width": 200, "height": 50}),
    );
    let (_, fit) = Text::of(&tiny, 1.0).unwrap().layout(200.0, 50.0);
    assert_eq!(fit.font_size, 36.0);
    assert!(fit.truncated && !fit.overflow, "{fit:?}");
}

#[test]
fn drawn_lines_show_breaks_and_cuts() {
    let text = "héllo wörld 🎉 one two three four";
    let wrap = text_layer(json!({"type": "text", "text": text, "fontSize": 20, "width": 120}));
    let t = Text::of(&wrap, 1.0).unwrap();
    let (w, h) = t.natural_size(120.0, 0.0);
    let (p, fit) = t.layout(w, h);
    let lines = t.drawn_lines(&p, &fit);
    assert!(lines.len() > 1, "{lines:?}");
    assert_eq!(lines.join(" "), text);

    let cut = text_layer(
        json!({"type": "text", "text": text, "fontSize": 20, "width": 120, "height": 24, "resize": "truncate"}),
    );
    let t = Text::of(&cut, 1.0).unwrap();
    let (p, fit) = t.layout(120.0, 24.0);
    let lines = t.drawn_lines(&p, &fit);
    assert!(lines.len() == 1 && lines[0].ends_with('…'), "{lines:?}");

    let clip = text_layer(
        json!({"type": "text", "text": text, "fontSize": 20, "width": 120, "height": 24, "resize": "truncate", "ellipsis": false}),
    );
    let t = Text::of(&clip, 1.0).unwrap();
    let (p, fit) = t.layout(120.0, 24.0);
    assert!(fit.truncated && !t.drawn_lines(&p, &fit)[0].ends_with('…'));
}

#[test]
fn letter_spacing_case_and_line_height_change_the_layout() {
    let plain = text_layer(json!({"type": "text", "text": "Hello", "fontSize": 20}));
    let spaced =
        text_layer(json!({"type": "text", "text": "Hello", "fontSize": 20, "letterSpacing": 10}));
    let (w0, h0) = Text::of(&plain, 1.0).unwrap().natural_size(0.0, 0.0);
    let (w1, _) = Text::of(&spaced, 1.0).unwrap().natural_size(0.0, 0.0);
    assert!(w1 >= w0 + 40.0, "{w0} → {w1}");

    let tall =
        text_layer(json!({"type": "text", "text": "Hello", "fontSize": 20, "lineHeight": 3}));
    let (_, h2) = Text::of(&tall, 1.0).unwrap().natural_size(0.0, 0.0);
    assert!((h2 - 60.0).abs() <= 1.0 && h2 > h0 * 2.0, "{h0} → {h2}");

    // Ranges count characters of the text as written, even when upper
    // case changes its length.
    let upper = text_layer(
        json!({"type": "text", "text": "straße sale", "textCase": "upper",
        "ranges": [{"start": 7, "end": 11, "color": "#FF0000"}]}),
    );
    let t = Text::of(&upper, 1.0).unwrap();
    assert_eq!(t.display, "STRASSE SALE");
    assert_eq!(&t.display[t.runs[1].0.clone()], "SALE");
}

#[test]
fn unknown_font_families_are_rejected_with_the_list() {
    let s: crate::scene::Scene = serde_json::from_value(json!({
        "width": 10, "height": 10, "sizes": [{"id": "a", "width": 10, "height": 10}],
        "layers": [{"id": "t", "type": "text", "text": "x", "fontFamily": "Comic Sans"}]
    }))
    .unwrap();
    let err = s.validate().unwrap_err();
    // Other tests may register fonts too, so check the list includes Inter.
    assert!(
        err.contains("unknown fontFamily Comic Sans; available: "),
        "{err}"
    );
    assert!(err.contains("Inter"), "{err}");
    assert!(families().contains(&"Inter".to_string()));
}

#[test]
fn every_weight_from_one_variable_font() {
    // Heavier weights draw wider glyphs, all the way to 900.
    let width = |w: u16| {
        let l = text_layer(json!({"type": "text", "text": "Weight", "fontSize": 40, "weight": w}));
        Text::of(&l, 1.0).unwrap().natural_size(0.0, 0.0).0
    };
    let widths: Vec<f32> = [100, 400, 700, 900].into_iter().map(width).collect();
    assert!(widths.windows(2).all(|p| p[1] > p[0]), "{widths:?}");
}
