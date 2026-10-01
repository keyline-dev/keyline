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
    let s = scene(
        json!([{"id": "photo", "type": "image", "asset": "img", "width": 400, "height": 200}, {"id": "bar", "type": "frame", "y": 150, "width": 400, "height": 50, "children": [{"id": "r", "type": "rect", "width": 400, "height": 50, "fill": "#FF0000"}]}]),
    );
    let d = describe(&s, None, true, None).unwrap();
    assert_eq!(d.lines().count(), 9, "{d}");
    assert!(
        d.starts_with(
            "assets img 400×400\nwide 400×200\n photo image 0,0 400×200 cover crop 50%h\n"
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
    let s = scene(
        json!([{"id": "long", "type": "text", "text": "Supercalifragilisticexpialidocious", "fontSize": 30, "width": 100, "height": 30}, {"id": "edge", "type": "text", "text": "edge", "x": 380, "fontSize": 30}, {"id": "gone", "type": "rect", "x": 500, "width": 10, "height": 10}]),
    );
    let d = describe(&s, Some("wide"), true, None).unwrap();
    assert!(
        d.contains("long text 0,0 100×30 15px (max 30, width)")
            && d.contains("!truncated needs 100×"),
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
fn letters_taller_than_their_line_are_clipped_by_the_frame() {
    // Caps at lineHeight 0.6 reach above their own box; the frame sized to
    // that box cuts their tops, though the box itself fits.
    let s = scene(json!([
        {"id": "band", "type": "frame", "y": 20, "width": 400, "height": 60, "children": [
            {"id": "title", "type": "text", "text": "HELLO", "fontSize": 100, "fontWeight": 900, "lineHeight": 0.6}
        ]}
    ]));
    let d = describe(&s, Some("wide"), true, None).unwrap();
    let line = d.lines().find(|l| l.contains("title text")).unwrap();
    assert!(
        line.contains("0,20 ") && line.contains("×60"),
        "the box fits: {line}"
    );
    assert!(line.contains("!clipped by band: top "), "{line}");
    // With room above, nothing is cut.
    let s = scene(json!([
        {"id": "title", "type": "text", "text": "HELLO", "y": 40, "fontSize": 100, "fontWeight": 900, "lineHeight": 0.6}
    ]));
    let d = describe(&s, Some("wide"), false, None).unwrap();
    assert!(!d.contains("!clipped"), "{d}");
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
fn a_line_that_changes_size_measures_its_real_ink() {
    // The smaller run first: its glyphs used to be dropped and the rest
    // shifted right, onto the next text.
    let s = scene(
        json!([{"type": "frame", "flexDirection": "row", "gap": 80, "alignItems": "center", "children": [
        {"id": "price", "type": "text", "fontSize": 60, "fontWeight": 700,
         "text": "<span style=\"font-size:40px\"><s>$2,190</s></span>  $1,890"},
        {"id": "next", "type": "text", "text": "Next thing", "fontSize": 30}]}]),
    );
    let w = warnings(&s, None).unwrap();
    assert!(!w.contains("!overlaps"), "{w}");
}

#[test]
fn text_whose_letters_dont_show_overlaps_nothing() {
    let over = |extra: serde_json::Value| {
        let mut ghost = json!({"id": "ghost", "type": "text", "text": "Hello", "fontSize": 30, "highlight": "#FFD400"});
        for (k, v) in extra.as_object().unwrap() {
            ghost[k] = v.clone();
        }
        let s = scene(json!([{"id": "a", "type": "text", "text": "Hello", "fontSize": 30}, ghost]));
        warnings(&s, None).unwrap_or_default().contains("!overlaps")
    };
    assert!(!over(json!({"fill": []})), "a highlight-only layer");
    assert!(!over(json!({"opacity": 0})), "an invisible layer");
    assert!(
        over(json!({"fill": [], "stroke": {"width": 2, "color": "#000000"}})),
        "outlined letters show"
    );
}

#[test]
fn tightly_set_lines_only_overlap_if_their_glyphs_do() {
    // "Monica" over "JETHANI": the line boxes overlap by a few pixels of
    // ascent and descent space, the letters don't.
    let s = scene(
        json!([{"id": "first", "type": "text", "text": "Monica", "y": 20, "fontSize": 26, "fontWeight": 800}, {"id": "last", "type": "text", "text": "JETHANI", "y": 50, "fontSize": 44, "fontWeight": 900}]),
    );
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
    // The crop of an image this wide is an advisory of its own.
    let w = warnings(&s, None).unwrap();
    assert!(
        w.lines()
            .all(|l| l.contains(" big image ") && l.contains("warn crop")),
        "{w}"
    );
    assert_eq!(
        facts(&s),
        "smallest text: wide 14px (fine), small 7px (fine); upscaled: wide big 2.0x"
    );
    let d = describe(&s, Some("wide"), true, None).unwrap();
    assert!(
        d.contains("big image 0,100 800×100 cover crop 88%h upscaled 2.0x"),
        "{d}"
    );
}

#[test]
fn warns_on_low_contrast_against_what_is_behind() {
    let s: Scene = serde_json::from_value(json!({"width": 400,
        "height": 100,
        "background": "#FFFFFF",
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [{"id": "panel", "type": "rect", "width": 200, "height": 100, "fill": "#1B2A5C"}, // Navy on navy: unreadable. White on navy and navy on white: fine.
            {"id": "dim", "type": "text", "text": "Dim", "x": 10, "y": 10, "fontSize": 16, "color": "#22335F"}, {"id": "lit", "type": "text", "text": "Lit", "x": 10, "y": 50, "fontSize": 16, "color": "#FFFFFF"}, {"id": "ink", "type": "text", "text": "Ink", "x": 250, "y": 10, "fontSize": 16, "color": "#1B2A5C"}]}))
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
fn knockout_text_is_judged_by_what_shows_through_it() {
    // Magenta behind the left frame, white behind the right; both frames
    // magenta. The letters' own color is never drawn: here it would pass
    // on the left and fail on the right.
    let s: Scene = serde_json::from_value(json!({"width": 400,
        "height": 100,
        "background": "#FFFFFF",
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [{"id": "photo", "type": "rect", "width": 200, "height": 100, "fill": "#C0187A"},
            {"id": "over", "type": "frame", "width": 200, "height": 100, "fill": "#C8207F", "children": [
                {"id": "lost", "type": "text", "text": "LOST", "x": 10, "y": 10, "fontSize": 60, "fontWeight": 900, "color": "#FFFFFF", "knockout": true}]},
            {"id": "clear", "type": "frame", "x": 200, "width": 200, "height": 100, "fill": "#C8207F", "children": [
                {"id": "seen", "type": "text", "text": "SEEN", "x": 10, "y": 10, "fontSize": 60, "fontWeight": 900, "color": "#C8207F", "knockout": true}]}]}))
    .unwrap();
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
    assert_eq!(w.lines().count(), 1, "{w}");
    assert!(
        w.starts_with("a lost text") && w.contains(" warn contrast 1.1:1 (WCAG 3)"),
        "{w}"
    );
}

#[test]
fn highlighted_text_is_judged_against_its_highlight() {
    // Navy on a yellow highlight, over navy: fine, for a whole text and for
    // one highlighted word judged over its own letters (the white word
    // beside it is fine on the navy).
    let s: Scene = serde_json::from_value(json!({"width": 400, "height": 100,
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [{"id": "panel", "type": "rect", "width": 400, "height": 100, "fill": "#1B2A5C"},
            {"id": "whole", "type": "text", "text": "Marked", "x": 10, "y": 10, "fontSize": 20,
             "color": "#1B2A5C", "highlight": "#FFD400"},
            {"id": "part", "type": "text", "text": "Plain <span style=\"background-color:#FFD400;color:#1B2A5C\">marked</span>",
             "x": 10, "y": 50, "fontSize": 20, "color": "#FFFFFF"}]}))
    .unwrap();
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap_or_default();
    assert!(!w.contains("contrast"), "{w}");
}

#[test]
fn checks_look_where_a_transformed_text_is_drawn() {
    // White text laid out over white, but moved onto the navy half: fine.
    // "B" laid out clear of "A", but scaled up over it: overlapping.
    let s: Scene = serde_json::from_value(json!({"width": 400, "height": 100, "background": "#FFFFFF",
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [{"id": "panel", "type": "rect", "width": 200, "height": 100, "fill": "#1B2A5C"},
            {"id": "moved", "type": "text", "text": "Moved", "x": 250, "y": 10, "fontSize": 20,
             "color": "#FFFFFF", "translate": [-230, 0]},
            {"id": "a", "type": "text", "text": "AAAA", "x": 250, "y": 60, "fontSize": 16, "color": "#000000"},
            {"id": "b", "type": "text", "text": "BB", "x": 310, "y": 60, "fontSize": 16, "color": "#000000", "scale": 4}]}))
    .unwrap();
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap_or_default();
    assert!(
        !w.lines()
            .any(|l| l.starts_with("a moved") && l.contains("contrast")),
        "{w}"
    );
    assert!(
        w.contains("!overlaps b") && w.contains("!overlaps a"),
        "{w}"
    );
}

#[test]
fn a_clipping_frame_that_cuts_a_shadow_says_so() {
    let card = |padding: f32| {
        scene(
            json!([{"id": "copy", "type": "frame", "flexDirection": "column", "padding": padding, "children": [
            {"id": "cta", "type": "rect", "width": 120, "height": 40, "fill": "#D0202E",
             "shadow": {"y": 8, "blur": 16, "color": "#00000055"}}]}]),
        )
    };
    let w = warnings(&card(0.0), None).unwrap_or_default();
    assert!(w == "all cta warn shadow clipped by copy\n", "{w}");
    let w = warnings(&card(24.0), None).unwrap_or_default();
    assert!(!w.contains("shadow clipped"), "room for it: {w}");
}

#[test]
fn leader_lines_whose_parts_meet_are_a_defect() {
    let s = scene(
        json!([{"id": "menu", "type": "text", "width": 200, "fontSize": 16, "leader": ".",
        "text": "Soup\t$9\nCharred leeks with brown butter\t$14"}]),
    );
    let w = warnings(&s, None).unwrap_or_default();
    assert!(
        w.contains("wide menu text")
            && w.contains(r#"!leader "Charred leeks with b…" meets "$14""#),
        "{w}"
    );
    assert!(!w.contains(r#""Soup""#), "{w}");
}

#[test]
fn faint_text_is_judged_as_drawn() {
    let s: Scene = serde_json::from_value(json!({"width": 400,
        "height": 100,
        "sizes": [{"id": "a", "width": 400, "height": 100}],
        "layers": [{"id": "panel", "type": "rect", "width": 400, "height": 100, "fill": "#1B2A5C"}, // White, but at 1/8 alpha, or inside a 10% frame: barely visible.
            {"id": "alpha", "type": "text", "text": "Faint", "x": 10, "y": 10, "color": "#FFFFFF20"}, {"id": "box", "type": "frame", "x": 200, "width": 200, "height": 100, "opacity": 0.1, "children": [{"id": "nested", "type": "text", "text": "Faint", "x": 10, "y": 10, "color": "#FFFFFF"}]}, {"id": "solid", "type": "text", "text": "Clear", "x": 10, "y": 50, "color": "#FFFFFF"}]}))
    .unwrap();
    let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
    assert_eq!(w.lines().count(), 2, "{w}");
    assert!(
        w.contains("a alpha text") && w.contains("a nested text"),
        "{w}"
    );
}

#[test]
fn a_word_wider_than_its_box_is_a_defect() {
    let s = scene(json!([
        {"id": "name", "type": "text", "text": "Sir Winston", "fontSize": 60, "width": 150},
        {"id": "fine", "type": "text", "x": 250, "text": "one two three", "fontSize": 20, "width": 60}]));
    let w = warnings(&s, None).unwrap_or_default();
    let line = w.lines().find(|l| l.starts_with("wide name")).unwrap_or("");
    assert!(line.contains(r#"!breaks "Winston" (needs "#), "{w}");
    assert!(
        !w.lines()
            .any(|l| l.contains("fine text") && l.contains("!breaks")),
        "breaks between words only: {w}"
    );
}

#[test]
fn text_cut_at_max_lines_says_a_height_would_shrink_it() {
    let text = |extra: serde_json::Value| {
        let mut t = json!({"id": "name", "type": "text", "text": "Winston Churchill", "fontSize": 40,
            "width": 200, "maxLines": 1});
        for (k, v) in extra.as_object().unwrap() {
            t[k] = v.clone();
        }
        warnings(&scene(json!([t])), None).unwrap_or_default()
    };
    let w = text(json!({}));
    assert!(
        w.contains("!truncated at maxLines 1 (a height lets it shrink instead)"),
        "{w}"
    );
    // And it does: with a height, it shrinks onto one line.
    let w = text(json!({"height": 50}));
    assert!(!w.contains("wide name"), "{w}");
}

#[test]
fn a_print_size_for_a_screen_master_hints_at_a_scale() {
    let s: Scene = serde_json::from_value(json!({"width": 1080, "height": 1350,
        "sizes": [{"id": "a4-portrait", "width": 2480, "height": 3508},
            {"id": "a4-scaled", "width": 2480, "height": 3508, "scale": 2.3},
            {"id": "post", "width": 1080, "height": 1350}], "layers": []}))
    .unwrap();
    assert_eq!(
        super::scale_hints(&s),
        [
            r#"hint: a4-portrait is 2.3× the master's width; give it "scale": 2.3 to keep the layout's proportions"#
        ]
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
        r.contains(" fit ") && r.contains("(max 40, height): \"SHRINK ME\""),
        "{r}"
    );
}

#[test]
fn every_shot_is_checked_not_only_the_one_shown_at_rest() {
    let s = scene(
        json!([{"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1}, "children": [{"id": "a", "type": "text", "text": "fine", "fontSize": 30}]}, {"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 1}, "children": [{"id": "b", "type": "text", "text": "a very long headline that will not fit", "fontSize": 30, "width": 100, "height": 30}]}, {"id": "logo", "type": "text", "text": "logo", "x": 380, "fontSize": 30}]),
    );
    let w = warnings(&s.resolved(), None).unwrap();
    assert!(w.contains("wide b ") && w.contains("!truncated"), "{w}");
    assert_eq!(w.matches("logo").count(), 2, "listed once per size: {w}");
}

#[test]
fn a_video_is_cropped_and_scaled_like_an_image() {
    let mut s = scene(
        json!([{"id": "clip", "type": "video", "asset": "img", "width": 400, "height": 200}]),
    );
    s.assets.get_mut("img").unwrap().clip =
        serde_json::from_value(json!({"duration": 2, "fps": 30, "audio": true})).unwrap();
    let d = describe(&s, Some("wide"), true, None).unwrap();
    assert!(d.contains("assets img 400×400 2s sound"), "{d}");
    assert!(d.contains(" clip video 0,0 400×200 cover crop 50%h"), "{d}");
}

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
        "wide band image 0,0 1200×238 cover crop 70%h warn crop cuts the image's middle (focus 50%,50%): a box 400 tall shows half\n"
    );
    // Less than half hidden (crop 48%h): the subject fits.
    assert_eq!(band(420, json!({})), "");
    assert!(
        band(238, json!({"focus": [0.3, 0.2]}))
            .contains("warn crop cuts the area around its focus (focus 30%,20%): a box 400 tall"),
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
            "crop 75%w warn crop cuts the image's middle (focus 50%,50%): a box at most 200 tall shows half"
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
