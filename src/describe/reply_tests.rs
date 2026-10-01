//! Tests of the later checks (crops, covers, tight ink, grouped and named
//! lines, firstFit reasons), facts and the render report.

use super::tests::scene;
use super::*;
use serde_json::json;

#[test]
fn a_cover_crop_that_hides_over_half_the_image_is_an_advisory() {
    let band = |h: u32, extra: serde_json::Value| {
        let mut l =
            json!({"id": "band", "type": "image", "asset": "photo", "width": 1200, "height": h});
        l.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let mut s = scene(json!([l]));
        s.width = 1200.0;
        s.height = 800.0;
        s.sizes.truncate(1);
        s.sizes[0].width = 1200.0;
        s.sizes[0].height = 800.0;
        s.assets.insert(
            "photo".into(),
            serde_json::from_value(json!({"sha256": "x", "width": 1200, "height": 800})).unwrap(),
        );
        warnings(&s, None).unwrap_or_default()
    };
    // A 1200×800 photo in a 1200×238 band: 70% of its height hidden.
    assert_eq!(
        band(238, json!({})),
        "wide band image 0,0 1200×238 cover crop 70%h warn crop cuts the image's middle (focus 50%,50%): height 400 shows half\n"
    );
    // Less than half hidden (crop 48%h): the subject fits.
    assert_eq!(band(420, json!({})), "");
    assert!(
        band(238, json!({"focus": [0.3, 0.2]}))
            .contains("warn crop cuts the area around its focus (focus 30%,20%): height 400"),
    );
    // Contain never crops; a crop picks its part on purpose.
    assert_eq!(band(238, json!({"fit": "contain"})), "");
    assert_eq!(
        band(
            238,
            json!({"crop": {"x": 0, "y": 0.3, "width": 1, "height": 0.3}})
        ),
        ""
    );
}

#[test]
fn a_crop_at_the_sides_names_the_height_that_shows_half() {
    let s = scene(
        json!([{"id": "strip", "type": "image", "asset": "img", "width": 100, "height": 400}]),
    );
    let w = describe(&s, Some("wide"), false, None).unwrap();
    assert!(
        w.contains(
            "crop 75%w warn crop cuts the image's middle (focus 50%,50%): height at most 200 shows half"
        ),
        "{w}"
    );
}

#[test]
fn a_halftone_on_a_dark_page_is_hinted() {
    let page = |bg: &str, layers: serde_json::Value| {
        let mut s = scene(layers);
        s.background = crate::scene::Color::parse(bg).unwrap();
        halftone_hints(&s)
    };
    let dots = json!({"id": "photo", "type": "image", "asset": "img", "width": 100, "height": 100, "filter": {"halftone": 6}});
    assert_eq!(
        page("#141414", json!([dots.clone()])),
        [
            "hint: photo halftone draws black dots; on #141414 it barely shows (a light fill behind it, or duotone instead)"
        ]
    );
    assert!(page("#FFFFFF", json!([dots.clone()])).is_empty());
    // A light frame between the photo and the dark page.
    assert!(
        page("#141414", json!([{"type": "frame", "width": 200, "height": 200, "fill": "#F4EFE6", "children": [dots]}]))
            .is_empty()
    );
}

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
        "wide pick firstFit 0,0 400×60 → short (long: long cut at maxLines 1)",
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
    // (0.24 pt a px), 1200.
    assert_eq!(image_dpi(&s, &s.sizes[0], 1.0).map(f32::round), Some(288.0));
    assert_eq!(
        image_dpi(&s, &s.sizes[0], 0.24).map(f32::round),
        Some(1200.0)
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
fn a_photo_sized_by_its_stack_is_told_its_min_height() {
    // A column gives the photo what its fixed siblings leave: 100 px of a
    // 400×400 photo 400 wide. The fix is a minHeight, in master px (the
    // half-scale size shows half at 100 px, a minHeight of 200).
    let s = scene(
        json!([{"id": "col", "type": "frame", "width": 400, "height": 200, "flexDirection": "column", "children": [
        {"id": "photo", "type": "image", "asset": "img", "width": "fill", "height": "fill"},
        {"id": "copy", "type": "rect", "width": "fill", "height": 100, "fill": "#000000"}]}]),
    );
    let w = warnings(&s, None).unwrap();
    assert!(
        w.contains("wide photo image 0,0 400×100 cover crop 75%h warn crop cuts the image's middle (focus 50%,50%): minHeight 200 shows half"),
        "{w}"
    );
}

#[test]
fn the_crop_fix_is_the_field_value_in_master_px() {
    // A 400×400 photo in a band 100 tall: at the half-scale size the box
    // is 50 tall, yet the height to write is 200 at both sizes.
    let s = scene(
        json!([{"id": "band", "type": "image", "asset": "img", "width": 400, "height": 100}]),
    );
    let w = warnings(&s, None).unwrap();
    assert!(w.contains("wide band image 0,0 400×100 cover crop 75%h warn crop cuts the image's middle (focus 50%,50%): height 200 shows half"), "{w}");
    assert!(w.contains("small band image 0,0 200×50 cover crop 75%h warn crop cuts the image's middle (focus 50%,50%): height 200 shows half"), "{w}");
}

#[test]
fn a_squeezed_photo_is_told_the_min_height_that_stops_the_squeeze() {
    // A stretch constraint shrinks the band with a shorter size: setting
    // its height again does nothing; a minHeight holds it.
    let band = |extra: serde_json::Value| {
        let mut l = json!({"id": "band", "type": "image", "asset": "img", "width": 400, "height": 200,
            "constraints": {"horizontal": "stretch", "vertical": "stretch"}});
        l.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let mut s = scene(json!([l]));
        s.sizes[0].height = 100.0;
        describe(&s, Some("wide"), false, None).unwrap()
    };
    let w = band(json!({}));
    assert!(w.contains("400×100 cover crop 75%h warn crop cuts the image's middle (focus 50%,50%): squeezed from 200 to 100: minHeight 200 shows half"), "{w}");
    let w = band(json!({"minHeight": 200}));
    assert!(!w.contains("warn crop"), "following it fixes it: {w}");
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
    assert!(
        warnings(&s, None)
            .unwrap()
            .contains("wide wide-photo image")
    );
    // Hidden at the small size: neither its text nor its crop counts there.
    let s = shots(json!({"small": {"hidden": true}}));
    assert!(
        facts(&s).starts_with("smallest text: wide 12px (fine), small 30px (big)"),
        "{}",
        facts(&s)
    );
    let w = warnings(&s, None).unwrap();
    assert!(
        w.contains("wide wide-photo") && !w.contains("small wide-photo"),
        "{w}"
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
fn ink_that_nearly_touches_a_neighbour_is_an_advisory() {
    let pair = |gap_y: f32| {
        let s = scene(json!([
            {"id": "eyebrow", "type": "text", "text": "LINEUP", "x": 10, "y": 10, "fontSize": 40, "fontWeight": 900},
            {"id": "head", "type": "text", "text": "VELVET", "x": 10, "y": gap_y, "fontSize": 60, "fontWeight": 900, "lineHeight": 0.8}
        ]));
        describe(&s, Some("wide"), false, None).unwrap()
    };
    // Caps 60 px tall at lineHeight 0.8 overshoot their box: a few px of ink
    // between the two lines.
    let d = pair(58.0);
    assert!(
        d.contains("wide eyebrow text") && d.contains(" warn ink 3px from head"),
        "{d}"
    );
    // Comfortably apart: nothing.
    assert_eq!(pair(70.0), "ok");
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
    assert!(
        r.contains(" head → short (long: long cut at maxLines 1)\n"),
        "{r}"
    );
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
fn a_crop_says_which_side_it_takes_most_from() {
    let band = |focus: serde_json::Value| {
        let s = scene(
            json!([{"id": "p", "type": "image", "asset": "img", "width": 400, "height": 100, "focus": focus}]),
        );
        describe(&s, Some("wide"), true, None).unwrap()
    };
    // Focus near the top: most of the crop comes off the bottom.
    assert!(
        band(json!([0.5, 0.1])).contains("cover crop 75%h bottom "),
        "{}",
        band(json!([0.5, 0.1]))
    );
    assert!(band(json!([0.5, 0.9])).contains("cover crop 75%h top "));
    // Centered: both sides alike, no side named.
    assert!(band(json!([0.5, 0.5])).contains("cover crop 75%h warn"));
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
