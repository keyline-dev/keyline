//! Tests of the later checks (covers, grouped and named lines, firstFit
//! reasons), what the layout says of photos, facts and the render report.

use super::tests::scene;
use super::*;
use serde_json::json;

#[test]
fn an_advisory_at_every_size_is_one_line_naming_the_text() {
    // An unnamed dim text at both sizes: one line, its words for a name.
    let s = scene(json!([
        {"id": "text1", "type": "text", "text": "Today at the park", "x": 10, "y": 10, "fontSize": 30, "color": "#DDDDDD"}
    ]));
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
    assert!(
        w.starts_with("all text1 \"Today at the par…\" warn contrast 1.") && w.lines().count() == 1,
        "{w}"
    );
    // Defects stay one line per size: their numbers differ.
    let s = scene(json!([
        {"id": "text1", "type": "text", "text": "Much too long for its box", "width": 60, "height": 20, "fontSize": 30, "minFontSize": 30}
    ]));
    let w = warnings(&s, None).unwrap();
    assert_eq!(w.lines().count(), 2, "{w}");
    assert!(
        w.starts_with("wide text1 \"Much too long fo…\" text "),
        "{w}"
    );
    // A named text keeps its id alone.
    let s = scene(json!([
        {"id": "note", "type": "text", "text": "Today", "x": 10, "y": 10, "fontSize": 30, "color": "#DDDDDD"}
    ]));
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
    assert!(w.starts_with("all note warn contrast"), "{w}");
}

#[test]
fn a_first_fit_says_why_it_skipped_an_option_when_its_choice_has_a_problem() {
    let pick = |over: bool| {
        let mut layers = vec![
            json!({"id": "pick", "type": "firstFit", "width": 400, "height": 60, "children": [
            {"id": "long", "type": "text", "text": "A very long headline that wraps", "width": 400, "fontSize": 30, "maxLines": 1},
            {"id": "short", "type": "text", "text": "Short", "fontSize": 30}]}),
        ];
        if over {
            layers.push(json!({"id": "over", "type": "text", "text": "Over", "fontSize": 30}));
        }
        let s = scene(serde_json::Value::Array(layers));
        describe(&s, Some("wide"), false, None).unwrap()
    };
    let d = pick(true);
    let mut lines = d.lines();
    assert_eq!(
        lines.next().unwrap(),
        "wide pick firstFit 0,0 400×60 → short (long: 2L > maxLines 1)",
        "{d}"
    );
    assert!(
        lines.next().unwrap().contains("short text") && d.contains("!overlaps over"),
        "{d}"
    );
    // Nothing wrong in what it drew: nothing to say.
    assert_eq!(pick(false), "ok");
}

#[test]
fn the_fonts_line_names_each_family_and_weight_drawn() {
    let s = scene(json!([
        {"id": "a", "type": "text", "text": "Plain <b>bold</b>", "fontSize": 20},
        {"id": "b", "type": "text", "text": "Heavy", "fontSize": 20, "fontWeight": 800},
        {"id": "c", "type": "text", "text": "Gone", "fontSize": 20, "fontFamily": "No Such Family"}
    ]));
    // A variable font has every weight; a family with no face falls back.
    assert_eq!(
        fonts_line(&s),
        "fonts: Inter 400/700/800, No Such Family (fallback)"
    );
}

#[test]
fn a_photos_print_resolution_counts_the_page_points() {
    let s =
        scene(json!([{"id": "p", "type": "image", "asset": "img", "width": 100, "height": 100}]));
    // 400 px drawn over 100 px: at 1 pt a px, 288 dpi; on a 300 dpi page
    // (0.24 pt a px) it would be 1200, but it's embedded at 300 at most.
    assert_eq!(image_dpi(&s, &s.sizes[0], 1.0).map(f32::round), Some(288.0));
    assert_eq!(
        image_dpi(&s, &s.sizes[0], 0.24).map(f32::round),
        Some(300.0)
    );
    assert_eq!(image_dpi(&scene(json!([])), &s.sizes[0], 1.0), None);
}

#[test]
fn a_shrunk_text_says_which_side_bound_it() {
    let s = scene(json!([
        // Tall enough for two lines, but held to one: maxLines.
        {"id": "one", "type": "text", "text": "two words", "fontSize": 30, "width": 100, "height": 100, "maxLines": 1},
        // Wide enough, too short: height.
        {"id": "low", "type": "text", "text": "Low", "fontSize": 40, "width": 300, "height": 30, "y": 120}
    ]));
    let r = text_report(&s, &s.sizes[0]);
    assert!(
        r.contains(" one ") && r.contains("(max 30, maxLines)"),
        "{r}"
    );
    assert!(r.contains(" low ") && r.contains("(max 40, height)"), "{r}");
}

#[test]
fn facts_read_every_shot_and_skip_shots_a_size_hides() {
    let shots = |media: serde_json::Value| {
        let mut s2 = json!({"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1}, "children": [
            {"id": "fine", "type": "text", "text": "tickets", "fontSize": 12, "y": 20},
            {"id": "wide-photo", "type": "image", "asset": "img", "y": 100, "width": 400, "height": 50}]});
        if !media.is_null() {
            s2["media"] = media;
        }
        scene(json!([
            {"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1}, "children": [
                {"id": "big", "type": "text", "text": "BIG", "fontSize": 60}]},
            s2
        ]))
        .resolved()
        .into_owned()
    };
    // The second shot's 12 px text is the smallest, though a still shows the first.
    let s = shots(serde_json::Value::Null);
    assert!(
        facts(&s).starts_with("smallest text: wide 12px (fine), small 6px (fine)"),
        "{}",
        facts(&s)
    );
    // Hidden at the small size: neither its text nor its crop counts there.
    let s = shots(json!({"small": {"hidden": true}}));
    assert!(
        facts(&s).starts_with("smallest text: wide 12px (fine), small 30px (big)"),
        "{}",
        facts(&s)
    );
}

#[test]
fn drawn_lines_print_a_no_break_space_as_itself() {
    let s = scene(
        json!([{"id": "t", "type": "text", "text": "Distrito\u{a0}4 \"siga\"", "fontSize": 20, "width": 110}]),
    );
    let r = text_report(&s, &s.sizes[0]);
    assert!(
        r.contains("Distrito\u{a0}4") && !r.contains("\\u{a0}"),
        "{r}"
    );
    assert!(r.contains("\\\"siga\\\""), "quotes are escaped: {r}");
}

#[test]
fn text_under_a_later_layer_or_highlight_is_covered() {
    let one =
        |layers: serde_json::Value| describe(&scene(layers), Some("wide"), false, None).unwrap();
    let pct = json!({"id": "pct", "type": "text", "text": "83%", "x": 10, "y": 10, "fontSize": 40});
    // A toast drawn after the text, over half of it.
    let toast = json!({"id": "toast", "type": "rect", "x": 40, "y": 0, "width": 200, "height": 80, "fill": "#FFFFFF"});
    let d = one(json!([pct.clone(), toast.clone()]));
    assert!(
        d.contains("wide pct text") && d.contains("!covered by toast "),
        "{d}"
    );
    // Under the text: a background, not a cover.
    assert_eq!(one(json!([toast, pct.clone()])), "ok");
    // A frame with no fill draws nothing over it; a layer used as a mask isn't drawn.
    assert_eq!(
        one(json!([pct.clone(), {"id": "box", "type": "frame", "width": 200, "height": 80}])),
        "ok"
    );
    assert_eq!(
        one(
            json!([pct.clone(), {"id": "m", "type": "rect", "width": 200, "height": 80},
            {"id": "masked", "type": "rect", "x": 300, "width": 10, "height": 10, "mask": {"layer": "m"}}])
        ),
        "ok"
    );
    // The next line's highlight reaching over it.
    let d = one(json!([
        {"id": "meet", "type": "text", "text": "MEET", "x": 10, "y": 10, "fontSize": 20},
        {"id": "name", "type": "text", "text": "Winston", "x": 10, "y": 24, "fontSize": 40,
         "highlight": {"color": "#F5C518", "padding": 14}}
    ]));
    assert!(
        d.contains("wide meet text") && d.contains("!covered by name highlight "),
        "{d}"
    );
}

#[test]
fn an_overflow_names_the_sibling_that_grew_and_hides_what_follows_from_it() {
    let row = |grow: bool| {
        let mut photo = json!({"id": "photoBox", "type": "rect", "width": "52%", "height": "fill", "fill": "#333333"});
        if grow {
            photo["flexGrow"] = json!(1);
        }
        let s = scene(
            json!([{"id": "row", "type": "frame", "width": 400, "height": 200, "flexDirection": "row", "children": [
            photo,
            {"id": "content", "type": "frame", "width": "fill", "height": "fill", "flexDirection": "column", "children": [
                {"id": "spec", "type": "rect", "width": 150, "height": 40, "fill": "#000000"},
                {"id": "word", "type": "text", "text": "Electrified", "fontSize": 30, "width": 100}]}]}]),
        );
        describe(&s, Some("wide"), false, None).unwrap()
    };
    let d = row(true);
    assert!(
        d.contains("content frame")
            && d.contains("!overflow needs 150×")
            && d.contains("(photoBox grows past its width 52%: flexGrow 1)"),
        "{d}"
    );
    // The word breaking inside the squeezed column follows from it: not listed.
    assert!(!d.contains("!breaks"), "{d}");
    // Without the grow, the content gets its room.
    assert!(!row(false).contains("photoBox grows"), "{}", row(false));
}

#[test]
fn the_render_report_names_the_option_a_first_fit_drew() {
    let s = scene(
        json!([{"id": "head", "type": "firstFit", "width": 400, "height": 60, "children": [
        {"id": "long", "type": "text", "text": "A very long headline that wraps", "width": 400, "fontSize": 30, "maxLines": 1},
        {"id": "short", "type": "text", "text": "Short", "fontSize": 30}]}]),
    );
    let r = text_report(&s, &s.sizes[0]);
    assert!(r.contains(" head → short (long: 2L > maxLines 1)\n"), "{r}");
}

#[test]
fn the_full_listing_says_what_moves_when() {
    let s = scene(json!([
        {"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 2}, "children": [
            {"id": "pct", "type": "text", "text": "{{n}}%", "fontSize": 40,
             "enter": {"effect": "fade-up", "delay": 0.4, "duration": 0.6},
             "animate": {"count": [0, 83], "delay": 1.4, "duration": 1.6}}]},
        {"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1.5}, "children": [
            {"id": "ring", "type": "ellipse", "width": 50, "height": 50, "animate": {"scale": [1, 1.1, 1], "duration": 1, "repeat": -1},
             "exit": {"effect": "fade"}}]}
    ]));
    let d = describe(&s.resolved(), Some("wide"), true, None).unwrap();
    assert!(d.contains(" s1 frame 0,0 400×200 shot 0–2s"), "{d}");
    assert!(
        d.contains("pct text") && d.contains(" enter fade-up 0.4–1s count 1.4–3s"),
        "{d}"
    );
    assert!(
        d.contains("ring ellipse") && d.contains(" scale 0–1s ×∞ exit fade at the end"),
        "{d}"
    );
    // Not in the problem lines.
    assert!(
        !describe(&s.resolved(), Some("wide"), false, None)
            .unwrap()
            .contains("enter ")
    );
}

#[test]
fn a_text_fill_that_hides_its_color_is_hinted() {
    let hint = |extra: serde_json::Value| {
        let mut t = json!({"id": "chip", "type": "text", "text": "Calm"});
        t.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        fill_hints(&scene(json!([t])))
    };
    assert_eq!(
        hint(json!({"color": "#FFFFFF", "fill": "#2E7D5B"})),
        [
            "hint: chip fill paints the letters (its color is unused); a box behind text is a frame with a fill"
        ]
    );
    assert!(
        hint(json!({"fill": "#2E7D5B"})).is_empty(),
        "fill alone is meant"
    );
    assert!(hint(json!({"color": "#2E7D5B", "fill": "#2E7D5B"})).is_empty());
}

#[test]
fn a_column_taller_than_the_canvas_says_so_once_on_itself() {
    let col = |hero: f32, height: serde_json::Value| {
        let mut c = json!({"id": "col", "type": "frame", "width": "fill", "flexDirection": "column", "children": [
            {"id": "hero", "type": "rect", "width": "fill", "height": hero, "fill": "#333333"},
            {"id": "card", "type": "frame", "width": "fill", "flexDirection": "column", "fill": "#EEEEEE", "children": [
                {"id": "line", "type": "text", "text": "The card's line", "fontSize": 30}]}]});
        if !height.is_null() {
            c["height"] = height;
        }
        describe(&scene(json!([c])), Some("wide"), false, None).unwrap()
    };
    assert_eq!(col(150.0, json!(null)), "ok", "it fits");
    // Sized by its content, 16 px taller than the canvas: said on the
    // column, with what to cut, not on the text inside it.
    let d = col(180.0, json!(null));
    assert!(
        d.starts_with("wide col frame 0,0 400×216 !clipped by canvas: bottom 16px"),
        "{d}"
    );
    assert!(
        !d.contains("line text"),
        "the text inside is that one problem: {d}"
    );
    // A column with its own height may run off the canvas on purpose; the
    // card sizing to its content inside it is what loses its content.
    assert_eq!(
        col(180.0, json!(260)),
        "wide card frame 0,180 400×36 !clipped by canvas: bottom 16px\n"
    );
}

#[test]
fn a_clipped_text_names_the_edge_that_cuts_it() {
    // The frame clips, but it reaches past the canvas: the canvas cuts the text.
    let s = scene(
        json!([{"id": "band", "type": "frame", "y": 100, "width": 400, "height": 200, "children": [
        {"id": "t", "type": "text", "text": "Low", "y": 80, "fontSize": 40}]}]),
    );
    let d = describe(&s, Some("wide"), false, None).unwrap();
    assert!(
        d.contains("t text") && d.contains("!clipped by canvas: bottom "),
        "{d}"
    );
    // Inside the canvas, its frame is what cuts it.
    let s = scene(
        json!([{"id": "band", "type": "frame", "y": 10, "width": 400, "height": 40, "children": [
        {"id": "t", "type": "text", "text": "Low", "y": 20, "fontSize": 40}]}]),
    );
    let d = describe(&s, Some("wide"), false, None).unwrap();
    assert!(d.contains("!clipped by band: bottom "), "{d}");
}

#[test]
fn a_rotated_layer_is_checked_where_it_is_drawn() {
    // Upright, the sticker fits by 10 px; turned 30° its corners leave the canvas.
    let sticker = |rotate: f32| {
        let s = scene(
            json!([{"id": "sale", "type": "text", "text": "SALE NOW", "x": 230, "y": 10, "fontSize": 30, "rotate": rotate}]),
        );
        describe(&s, Some("wide"), false, None).unwrap()
    };
    assert_eq!(sticker(0.0), "ok");
    assert!(
        sticker(30.0).contains("sale text") && sticker(30.0).contains("!clipped by canvas: "),
        "{}",
        sticker(30.0)
    );
}

#[test]
fn a_tilted_line_near_the_safe_area_is_checked_as_drawn() {
    // A long line just above a story's bottom bar: tilted, one end dips in.
    let line = |rotate: f32| {
        let mut s = scene(
            json!([{"id": "cta", "type": "text", "text": "Adopt today at the shelter",
            "x": 20, "y": 150, "fontSize": 30, "rotate": rotate}]),
        );
        s.sizes.truncate(1);
        s.sizes[0].safe = [0.0, 0.0, 15.0, 0.0];
        describe(&s, None, false, None).unwrap()
    };
    assert_eq!(line(0.0), "ok");
    assert!(
        line(-4.0).contains("cta text") && line(-4.0).contains("!unsafe"),
        "{}",
        line(-4.0)
    );
}

#[test]
fn an_outline_covers_text_only_where_its_stroke_runs() {
    let d = |tx: f32, ty: f32| {
        describe(
            &scene(json!([
                {"id": "label", "type": "text", "text": "KINDLEPAW", "x": tx, "y": ty, "fontSize": 20},
                {"id": "blobLine", "type": "ellipse", "x": 50, "y": 10, "width": 300, "height": 180,
                 "fill": [], "stroke": {"width": 4, "color": "#D0202E"}}
            ])),
            Some("wide"),
            false,
            None,
        )
        .unwrap()
    };
    // Inside the ring, clear of its stroke: nothing covers it.
    assert_eq!(d(150.0, 90.0), "ok");
    // Across the stroke at the ring's left edge: covered.
    assert!(
        d(30.0, 90.0).contains("!covered by blobLine "),
        "{}",
        d(30.0, 90.0)
    );
}

#[test]
fn a_cut_only_while_it_moves_says_when() {
    let card = |child: serde_json::Value| {
        let s = scene(
            json!([{"id": "info", "type": "frame", "x": 20, "y": 20, "width": 300, "height": 120, "children": [child]}]),
        );
        describe(&s, Some("wide"), false, None).unwrap()
    };
    // Whole at rest, its pop overshoots the frame that clips it.
    let d = card(
        json!({"id": "ticket", "type": "rect", "x": 1, "y": 1, "width": 298, "height": 118, "fill": "#000000",
        "enter": {"effect": "pop", "delay": 2.1, "duration": 0.6}}),
    );
    assert!(
        d.contains("ticket rect")
            && d.contains("!clipped by info: ")
            && d.contains(" during enter pop 2.1–2.7s"),
        "{d}"
    );
    // A pulse does too.
    let d = card(
        json!({"id": "cta", "type": "rect", "x": 10, "y": 10, "width": 280, "height": 100, "fill": "#000000",
        "animate": {"scale": [1, 1.1, 1], "duration": 1, "repeat": -1}}),
    );
    assert!(d.contains(" during scale 0–1s"), "{d}");
    // A photo filling the frame and zooming slowly means to run past it.
    assert_eq!(
        card(
            json!({"id": "photo", "type": "image", "asset": "img", "width": 300, "height": 120,
            "animate": {"scale": [1, 1.1], "duration": 6}})
        ),
        "ok"
    );
}

#[test]
fn a_first_fit_counts_the_lines_a_balanced_text_needs_uncut() {
    // Builder B's council headline (ES row, facebook-feed), Inter for
    // Archivo: `textWrap: balance` narrows a cut text's lines, and its
    // uncut count is taken across the box, not at that narrower width.
    let head = |id: &str, text: &str, max_lines: Option<u32>| {
        let mut t = json!({"id": id, "type": "text", "text": text, "width": "fill", "fontSize": 56,
            "fontWeight": 800, "lineHeight": 1.08, "letterSpacing": -1, "textWrap": "balance"});
        if let Some(m) = max_lines {
            t["maxLines"] = json!(m);
        }
        t
    };
    let long = "Un Distrito 4 que funciona para las familias.";
    let s = scene(
        json!([{"id": "head", "type": "firstFit", "width": 360, "children": [
        head("long", long, Some(3)), head("short", "Las familias primero.", Some(3))]}]),
    );
    let r = text_report(&s, &s.sizes[0]);
    // What the long headline takes uncut, alone in the same box.
    let alone = scene(
        json!([{"id": "box", "type": "frame", "width": 360, "flexDirection": "column",
        "children": [head("long", long, None)]}]),
    );
    let lines = text_report(&alone, &alone.sizes[0])
        .lines()
        .find(|l| l.starts_with(" long "))
        .map_or(0, |l| l.matches(" / ").count() + 1);
    assert!(lines > 3, "it needs more than 3 lines: {lines}");
    assert!(
        r.contains(&format!(" head → short (long: {lines}L > maxLines 3)")),
        "{r}"
    );
}

#[test]
fn a_pulse_cut_by_the_frame_that_sizes_to_it_says_when() {
    // The button fills the row that hugs it; the row clips, so the pulse
    // is cut on every side.
    let s = scene(
        json!([{"id": "buy", "type": "frame", "x": 20, "y": 20, "flexDirection": "row", "children": [
        {"id": "cta", "type": "frame", "width": 244, "height": 80, "fill": "#D0202E",
         "animate": {"scale": [1, 1.07, 1], "delay": 1, "duration": 0.7}}]}]),
    );
    let d = describe(&s, Some("wide"), false, None).unwrap();
    assert!(
        d.contains("cta frame")
            && d.contains("!clipped by buy: ")
            && d.contains(" during scale 1–1.7s"),
        "{d}"
    );
}

#[test]
fn a_stroke_crossing_text_covers_only_what_it_crosses() {
    // A rotated 5 px blob outline crossing the end of a 40 px line covers a
    // sliver of it, not the box around where the two meet.
    let mut s = scene(json!([
        {"id": "label", "type": "text", "text": "KINDLEPAW RESCUE", "x": 350, "y": 330, "fontSize": 40},
        {"id": "blobLine", "type": "path", "shape": "blob-3", "x": 300, "y": 300, "width": 700, "height": 700,
         "rotate": 5, "fill": [], "stroke": {"width": 5, "color": "#D0202E"}}
    ]));
    s.sizes.truncate(1);
    (s.sizes[0].width, s.sizes[0].height) = (1080.0, 1350.0);
    (s.width, s.height) = (1080.0, 1350.0);
    let d = describe(&s, None, false, None).unwrap();
    let pct: f32 = d
        .split("!covered by blobLine ")
        .nth(1)
        .and_then(|r| r.split('%').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(100.0);
    assert!(pct < 20.0, "{d}");
}

#[test]
fn a_broken_word_says_the_width_the_whole_word_needs() {
    // The width it needs is the word's own, at its size, not its widest
    // broken piece.
    let s = scene(json!([
        {"id": "name", "type": "text", "text": "Winston", "width": 150, "fontSize": 80, "fontWeight": 800},
        {"id": "whole", "type": "text", "text": "Winston", "y": 100, "fontSize": 80, "fontWeight": 800}
    ]));
    let d = describe(&s, Some("wide"), true, None).unwrap();
    let whole: f32 = d
        .lines()
        .find(|l| l.contains("whole text"))
        .and_then(|l| l.split_whitespace().nth(3))
        .and_then(|wh| wh.split('×').next())
        .and_then(|w| w.parse().ok())
        .unwrap();
    let needs: f32 = d
        .split("(needs ")
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap();
    assert!(
        (needs - whole).abs() <= 2.0,
        "needs {needs}, the word is {whole} wide: {d}"
    );
}

#[test]
fn an_outline_with_no_fill_given_covers_only_by_its_stroke() {
    // Builder B's rescue: a blob outline with a stroke and no `fill`,
    // turned, scaled and moved in its frame, its stroke clear of the chip.
    let s = scene(json!([
        {"id": "chip", "type": "text", "text": "calm", "x": 150, "y": 85, "fontSize": 24},
        {"id": "pic", "type": "frame", "x": 100, "y": 20, "width": 160, "height": 160, "clipsContent": false, "children": [
            {"id": "blobLine", "type": "path", "shape": "blob-3", "width": "100%", "height": "100%",
             "stroke": {"width": 6, "color": "#E4572E"}, "rotate": 4, "scale": 0.98, "translate": [18, 16]}]}
    ]));
    // The outline is drawn over the chip, which sits inside its ring, clear
    // of the stroke.
    let d = describe(&s, Some("wide"), false, None).unwrap();
    assert!(!d.contains("!covered"), "{d}");
}

#[test]
fn a_first_fit_in_a_crowded_column_keeps_its_shortest_option() {
    let s = scene(
        json!([{"id": "col", "type": "frame", "width": 300, "height": 200, "flexDirection": "column", "children": [
        {"id": "photo", "type": "rect", "width": "fill", "height": 120, "fill": "#333333"},
        {"id": "head", "type": "firstFit", "width": "fill", "children": [
            {"id": "long", "type": "text", "text": "A headline long enough to take three lines", "width": "fill", "fontSize": 30, "maxLines": 3},
            {"id": "short", "type": "text", "text": "Short", "width": "fill", "fontSize": 30}]},
        {"id": "cta", "type": "rect", "width": "fill", "height": 60, "fill": "#D0202E"}]}]),
    );
    let d = describe(&s, Some("wide"), true, None).unwrap();
    let head = d.lines().find(|l| l.contains("head firstFit")).unwrap();
    // Not squeezed to nothing: as tall as its short option, and the column
    // says what it needs.
    let h: f32 = head
        .split_whitespace()
        .nth(3)
        .and_then(|wh| wh.split('×').nth(1))
        .and_then(|h| h.parse().ok())
        .unwrap();
    assert!(h >= 30.0, "{d}");
    assert!(head.contains("→ short"), "{d}");
    assert!(
        d.contains("col frame") && d.contains("!overflow needs 300×"),
        "{d}"
    );
}

#[test]
fn drawn_lines_cover_every_shot() {
    // The lineup in shot 2 wraps; a still shows shot 1, the video both.
    let s = scene(json!([
        {"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1}, "children": [
            {"id": "title", "type": "text", "text": "Nightline", "fontSize": 40}]},
        {"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1}, "children": [
            {"id": "acts", "type": "text", "text": "Deep Orchard", "width": 200, "fontSize": 40}]}
    ]))
    .resolved()
    .into_owned();
    let r = text_report(&s, &s.sizes[0]);
    assert!(r.contains(r#" acts 40px: "Deep" / "Orchard""#), "{r}");
}

#[test]
fn only_an_opaque_layer_covers_text() {
    // The speaker-card agent's glow: a violet frame at 22% over the name,
    // blurred. Reported as covering, it was moved, faded, then deleted.
    let over = |glow: serde_json::Value| {
        let mut layers = vec![
            json!({"id": "name", "type": "text", "text": "Maya Okonkwo", "x": 40, "y": 40, "fontSize": 40}),
        ];
        let mut g =
            json!({"id": "glow", "type": "frame", "x": 0, "y": 0, "width": 400, "height": 200});
        g.as_object_mut()
            .unwrap()
            .extend(glow.as_object().unwrap().clone());
        layers.push(g);
        describe(&scene(json!(layers)), Some("wide"), false, None).unwrap()
    };
    // Made transparent at all, by its opacity, its fill or a parent's: the
    // agent's choice, for the contrast check to judge.
    for glow in [
        json!({"fill": "#7C5CFF", "opacity": 0.22, "blur": 200}),
        json!({"fill": "#7C5CFF", "opacity": 0.9}),
        json!({"fill": "#7C5CFF80"}),
        json!({"fill": "radial-gradient(#7C5CFFCC, #7C5CFF00)"}),
    ] {
        let d = over(glow.clone());
        assert!(!d.contains("!covered"), "{glow}: {d}");
    }
    // Opaque, it hides the name.
    let d = over(json!({"fill": "#7C5CFF"}));
    assert!(
        d.contains("name text") && d.contains("!covered by glow"),
        "{d}"
    );
}

#[test]
fn the_layout_says_which_part_of_a_photo_is_drawn() {
    // A 1200×800 photo in bands and boxes: the region drawn, in its own px,
    // and no verdict: a crop is the agent's choice.
    let photo = |layer: serde_json::Value| {
        let mut s = scene(json!([layer]));
        s.assets.insert(
            "photo".into(),
            serde_json::from_value(json!({"sha256": "x", "width": 1200, "height": 800})).unwrap(),
        );
        let d = describe(&s, Some("wide"), true, None).unwrap();
        assert!(!d.contains(" !") && !d.contains("warn"), "{d}");
        d
    };
    let band = json!({"id": "band", "type": "image", "asset": "photo", "width": 400, "height": 80});
    // Cover takes the middle by default; `focus` moves the window.
    assert!(
        photo(band.clone())
            .contains("band image 0,0 400×80 cover shows 1200×240 from 0,280 of 1200×800")
    );
    let mut top = band.clone();
    top["focus"] = json!([0.5, 0.1]);
    assert!(photo(top).contains("cover shows 1200×240 from 0,56 of 1200×800"));
    // A tall box crops the sides.
    let tall =
        json!({"id": "tall", "type": "image", "asset": "photo", "width": 100, "height": 200});
    assert!(photo(tall).contains("cover shows 400×800 from 400,0 of 1200×800"));
    // A crop by hand: the region it keeps, then what cover takes of that.
    let mut hand = band.clone();
    hand["crop"] = json!({"x": 0, "y": 0.25, "width": 1, "height": 0.5});
    assert!(photo(hand).contains("cover shows 1200×240 from 0,280 of 1200×800"));
    // Contain draws the whole photo: nothing to say.
    let mut contain = band;
    contain["fit"] = json!("contain");
    assert!(!photo(contain).contains("shows"));
}

#[test]
fn ink_on_ink_is_a_defect_and_ink_apart_is_not_judged() {
    let pair = |y: f32| {
        let s = scene(json!([
            {"id": "eyebrow", "type": "text", "text": "HEADLINING", "x": 40, "y": y, "fontSize": 24},
            {"id": "caps", "type": "text", "text": "VOLTA", "x": 20, "y": 100, "fontSize": 100, "fontWeight": 900, "lineHeight": 0.7}
        ]));
        describe(&s, Some("wide"), false, None).unwrap()
    };
    // Well apart, or a hair apart: how close is the design's call.
    assert_eq!(pair(20.0), "ok");
    assert!(!pair(55.0).contains("warn"), "{}", pair(55.0));
    // Overlapping ink is always wrong.
    assert!(pair(80.0).contains("!overlaps"), "{}", pair(80.0));
}

#[test]
fn unsafe_text_says_how_far_it_reaches_under_the_bars() {
    // A story's bottom 340 px are the platform's: a button at 1600 reaches
    // 32 px under them (its letters, as drawn).
    let mut s = scene(json!([
        {"id": "cta", "type": "text", "text": "Book your place", "x": 40, "y": 1560, "fontSize": 40}
    ]));
    s.width = 1080.0;
    s.height = 1920.0;
    s.sizes.truncate(1);
    s.sizes[0].width = 1080.0;
    s.sizes[0].height = 1920.0;
    s.sizes[0].safe = [250.0, 0.0, 340.0, 0.0];
    let d = describe(&s, Some("wide"), false, None).unwrap();
    let px: f32 = d
        .split("!unsafe bottom ")
        .nth(1)
        .and_then(|r| r.split("px").next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{d}"));
    assert!((1.0..60.0).contains(&px), "{d}");
}

#[test]
fn a_marked_subject_steers_the_crop_and_says_how_much_shows() {
    let band = |subject: serde_json::Value, h: u32| {
        let mut l =
            json!({"id": "band", "type": "image", "asset": "photo", "width": 400, "height": h});
        if !subject.is_null() {
            l["subject"] = subject;
        }
        let mut s = scene(json!([l]));
        s.assets.insert(
            "photo".into(),
            serde_json::from_value(json!({"sha256": "x", "width": 1200, "height": 800})).unwrap(),
        );
        describe(&s, Some("wide"), true, None).unwrap()
    };
    // The barn sits high: the band moves up to it, all of it in view.
    let d = band(json!([0.1, 0.1, 0.4, 0.2]), 80);
    assert!(
        d.contains("cover shows 1200×240 from 0,40 of 1200×800 (subject 100%)"),
        "{d}"
    );
    // Taller than the band can show: what fits, said as a share.
    let d = band(json!([0.0, 0.2, 1.0, 0.6]), 80);
    assert!(d.contains("(subject 50%)"), "{d}");
    // No subject marked: nothing said of one.
    assert!(!band(serde_json::Value::Null, 80).contains("subject"));
}
