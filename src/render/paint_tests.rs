//! Pixel tests of the paint model: fill stacks, gradient kinds, image
//! fills in shapes, patterns, grain, shadows, blur, radii, strokes, shapes,
//! transforms, masks and image adjustments.

use serde_json::json;
use skia_safe::Color as C;

use super::tests::{BLUE, RED, count, pixels, pixels_with, two_tone};

fn near(a: (u8, u8, u8), b: (u8, u8, u8), tol: i32) -> bool {
    [(a.0, b.0), (a.1, b.1), (a.2, b.2)]
        .iter()
        .all(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() <= tol)
}

const WHITE: (u8, u8, u8) = (255, 255, 255);
const BLACK: (u8, u8, u8) = (0, 0, 0);

#[test]
fn fills_stack_bottom_to_top_with_opacity() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": ["#FF0000", {"color": "#0000FF", "opacity": 0.5}]}]),
    );
    assert!(near(px(50, 50), (128, 0, 127), 2), "{:?}", px(50, 50));
}

#[test]
fn an_empty_fill_list_paints_nothing() {
    let px = pixels(json!([{"type": "rect", "width": 100, "height": 100, "fill": []}]));
    assert_eq!(px(50, 50), WHITE);
}

#[test]
fn radial_gradients_run_from_the_center_to_the_box_edge() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"gradient": {"type": "radial", "stops": ["#FFFFFF", "#000000"]}}}]),
    );
    assert!(near(px(50, 50), WHITE, 8), "{:?}", px(50, 50));
    assert!(near(px(99, 50), BLACK, 8), "{:?}", px(99, 50));
    assert!(near(px(50, 99), BLACK, 8), "{:?}", px(50, 99));
    assert!(
        near(px(99, 99), BLACK, 1),
        "past the radius the last stop holds"
    );
}

#[test]
fn conic_gradients_start_at_the_top_and_turn_clockwise() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"gradient": {"type": "conic", "stops": ["#FF0000", "#0000FF"]}}}]),
    );
    // Just clockwise of 12 o'clock is nearly red; just before it, nearly blue.
    let (r1, _, b1) = px(55, 5);
    let (r2, _, b2) = px(45, 5);
    assert!(r1 > 200 && b1 < 60, "{:?}", px(55, 5));
    assert!(b2 > 200 && r2 < 60, "{:?}", px(45, 5));
}

#[test]
fn css_angles_set_a_linear_gradient_s_direction() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"gradient": {"angle": 180, "stops": ["#000000", "#FFFFFF"]}}}]),
    );
    assert!(
        px(50, 1).0 < 10 && px(50, 98).0 > 245,
        "{:?} {:?}",
        px(50, 1),
        px(50, 98)
    );
    assert_eq!(px(5, 50), px(95, 50), "a vertical gradient is flat across");
}

#[test]
fn image_fills_take_the_shape_of_any_layer() {
    let img = two_tone(100, 100, C::RED, C::BLUE);
    let px = pixels_with(
        json!([{"type": "ellipse", "width": 100, "height": 100, "fill": {"image": "img"}}]),
        &[("img", img)],
    );
    assert_eq!(px(25, 50), RED);
    assert_eq!(px(75, 50), BLUE);
    assert_eq!(px(3, 3), WHITE, "outside the circle");
}

#[test]
fn patterns_cover_part_of_the_box() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"pattern": "stripes", "color": "#000000", "size": 20}}]),
    );
    let dark = count(&px, |c| c.0 < 128);
    assert!((4000..=6000).contains(&dark), "stripes cover half: {dark}");
    let dots = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"pattern": "dots", "color": "#000000", "size": 20}}]),
    );
    let dark = count(&dots, |c| c.0 < 128);
    assert!((500..=2000).contains(&dark), "dots cover a little: {dark}");
}

#[test]
fn noise_is_grain_that_repeats_with_its_seed() {
    let render = |seed: u32| {
        let px = pixels(
            json!([{"type": "rect", "width": 100, "height": 100, "fill": ["#808080", {"noise": 0.5, "seed": seed}]}]),
        );
        (0..20).map(|i| px(i * 5, 50)).collect::<Vec<_>>()
    };
    let a = render(1);
    assert_eq!(a, render(1));
    assert_ne!(a, render(2));
    let distinct: std::collections::HashSet<_> = a.iter().collect();
    assert!(distinct.len() > 5, "grain varies: {a:?}");
}

#[test]
fn shapes_cast_box_shadows_and_inset_shadows_sit_inside() {
    let px = pixels(
        json!([{"type": "rect", "x": 20, "y": 20, "width": 40, "height": 40, "shadow": [{"y": 20, "color": "#000000"}, {"x": 10, "color": "#FF0000", "inset": true}], "fill": "#FFFFFF"}]),
    );
    assert_eq!(px(40, 70), BLACK, "the drop shadow shows below");
    assert_eq!(px(40, 10), WHITE, "nothing above");
    assert_eq!(px(23, 40), RED, "the inset shadow hugs the left inner edge");
    assert_eq!(px(55, 40), WHITE);
}

#[test]
fn shadow_spread_grows_the_shadow() {
    let px = pixels(
        json!([{"type": "rect", "x": 40, "y": 40, "width": 20, "height": 20, "shadow": {"spread": 10, "color": "#000000"}, "fill": "#FFFFFF"}]),
    );
    assert_eq!(px(35, 50), BLACK);
    assert_eq!(px(25, 50), WHITE);
}

#[test]
fn text_and_cutouts_cast_the_shadow_of_their_own_alpha() {
    let px = pixels(
        json!([{"type": "text", "text": "I", "x": 40, "y": 0, "fontSize": 90, "fontWeight": 900, "color": "#FFFFFF", "shadow": {"x": 12, "color": "#000000"}}]),
    );
    let dark = count(&px, |c| c == BLACK);
    assert!(dark > 300, "the glyph's shadow shows: {dark}");
    assert!(dark < 3000, "only the glyph's shape, not its box: {dark}");
}

#[test]
fn blur_softens_edges() {
    let sharp = pixels(json!([{"type": "rect", "width": 50, "height": 100, "fill": "#000000"}]));
    let soft = pixels(
        json!([{"type": "rect", "width": 50, "height": 100, "blur": 10, "fill": "#000000"}]),
    );
    assert_eq!(sharp(52, 50), WHITE);
    let (g, _, _) = soft(52, 50);
    assert!(g > 30 && g < 225, "{:?}", soft(52, 50));
}

#[test]
fn backdrop_blur_frosts_what_is_behind() {
    let px = pixels(
        json!([{"type": "rect", "width": 50, "height": 100, "fill": "#000000"}, {"type": "rect", "x": 25, "y": 25, "width": 50, "height": 50, "backdropBlur": 16, "fill": "#FFFFFF00"}]),
    );
    let (g, _, _) = px(50, 50);
    assert!(
        g > 40 && g < 215,
        "the edge is blurred inside the panel: {:?}",
        px(50, 50)
    );
    assert_eq!(px(50, 10), WHITE, "and sharp outside it");
    assert_eq!(px(48, 10), BLACK);
}

#[test]
fn radius_full_makes_a_capsule_and_corners_can_differ() {
    let pill = pixels(
        json!([{"type": "rect", "width": 100, "height": 40, "borderRadius": "full", "fill": "#000000"}]),
    );
    assert_eq!(pill(2, 2), WHITE);
    assert_eq!(pill(50, 20), BLACK);
    let tab = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "borderRadius": [40, 0, 0, 0], "fill": "#000000"}]),
    );
    assert_eq!(tab(3, 3), WHITE, "top-left rounded");
    assert_eq!(tab(97, 3), BLACK, "top-right square");
}

#[test]
fn dashed_strokes_leave_gaps() {
    let px = pixels(
        json!([{"type": "line", "x": 0, "y": 50, "width": 100, "stroke": {"width": 4, "color": "#000000", "dash": [10, 10]}}]),
    );
    let on = (0..100).filter(|x| px(*x, 50) == BLACK).count();
    assert!((40..=60).contains(&on), "half the line is drawn: {on}");
}

#[test]
fn per_side_borders_draw_only_their_sides() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "stroke": {"width": [0, 0, 6, 0], "color": "#000000"}}]),
    );
    assert_eq!(px(50, 97), BLACK);
    assert_eq!(px(50, 2), WHITE);
    assert_eq!(px(2, 50), WHITE);
}

#[test]
fn line_markers_draw_arrowheads() {
    let plain = pixels(
        json!([{"type": "line", "x": 10, "y": 50, "width": 80, "stroke": {"width": 2, "color": "#000000"}}]),
    );
    let arrow = pixels(
        json!([{"type": "line", "x": 10, "y": 50, "width": 80, "stroke": {"width": 2, "color": "#000000", "markerEnd": "triangle"}}]),
    );
    let dark = |px: &dyn Fn(i32, i32) -> (u8, u8, u8)| {
        (0..100)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .filter(|&(x, y)| px(x, y).0 < 100)
            .count()
    };
    assert!(dark(&arrow) > dark(&plain) + 10);
}

#[test]
fn polygons_and_stars_fill_their_outline() {
    let star = pixels(
        json!([{"type": "polygon", "width": 100, "height": 100, "sides": 5, "innerRadius": 0.4, "fill": "#000000"}]),
    );
    assert_eq!(star(50, 50), BLACK);
    assert_eq!(star(50, 3), BLACK, "a point at the top");
    assert_eq!(star(20, 20), WHITE, "between points");
    let hexagon = pixels(
        json!([{"type": "polygon", "width": 100, "height": 100, "sides": 6, "fill": "#000000"}]),
    );
    assert_eq!(hexagon(50, 50), BLACK);
    assert_eq!(hexagon(2, 2), WHITE);
}

#[test]
fn named_shapes_and_svg_paths_fit_the_box() {
    let heart = pixels(
        json!([{"type": "path", "shape": "heart", "width": 100, "height": 100, "fill": "#000000"}]),
    );
    assert_eq!(heart(50, 50), BLACK);
    assert_eq!(heart(50, 3), WHITE, "the dip at the top");
    let tri = pixels(
        json!([{"type": "path", "d": "M0 100 L50 0 L100 100 Z", "width": 100, "height": 100, "fill": "#000000"}]),
    );
    assert_eq!(tri(50, 80), BLACK);
    assert_eq!(tri(5, 20), WHITE);
}

#[test]
fn arcs_draw_rings_and_pies() {
    let ring = pixels(
        json!([{"type": "ellipse", "width": 100, "height": 100, "arc": {"inner": 0.6}, "fill": "#000000"}]),
    );
    assert_eq!(ring(50, 50), WHITE, "the hole");
    assert_eq!(ring(50, 5), BLACK, "the ring");
    let half = pixels(
        json!([{"type": "ellipse", "width": 100, "height": 100, "arc": {"start": 0, "end": 180}, "fill": "#000000"}]),
    );
    assert_eq!(half(75, 50), BLACK, "the right half");
    assert_eq!(half(25, 50), WHITE);
}

#[test]
fn flips_offsets_and_scale_transform_after_layout() {
    let img = two_tone(100, 100, C::RED, C::BLUE);
    let flipped = pixels_with(
        json!([{"type": "image", "asset": "img", "width": 100, "height": 100, "flipX": true}]),
        &[("img", img.clone())],
    );
    assert_eq!(flipped(25, 50), BLUE);
    let moved = pixels(
        json!([{"type": "rect", "width": 20, "height": 20, "translate": [50, 0], "fill": "#000000"}]),
    );
    assert_eq!(moved(60, 10), BLACK);
    assert_eq!(moved(10, 10), WHITE);
    let small = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "scale": 0.5, "fill": "#000000"}]),
    );
    assert_eq!(small(10, 10), WHITE);
    assert_eq!(small(50, 50), BLACK);
}

#[test]
fn masks_clip_to_shapes_paths_and_other_layers() {
    let circle = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "mask": "ellipse", "fill": "#000000"}]),
    );
    assert_eq!(circle(3, 3), WHITE);
    assert_eq!(circle(50, 50), BLACK);
    let inverted = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "mask": {"shape": "ellipse", "invert": true}, "fill": "#000000"}]),
    );
    assert_eq!(inverted(3, 3), BLACK);
    assert_eq!(inverted(50, 50), WHITE);
    let by_layer = pixels(
        json!([{"id": "m", "type": "rect", "x": 60, "y": 0, "width": 40, "height": 100, "fill": "#000000"}, {"type": "rect", "width": 100, "height": 100, "mask": {"layer": "m"}, "fill": "#FF0000"}]),
    );
    assert_eq!(by_layer(80, 50), RED, "shown where the mask layer is");
    assert_eq!(
        by_layer(20, 50),
        WHITE,
        "hidden elsewhere, and the mask layer isn't drawn"
    );
}

#[test]
fn adjustments_recolor_images() {
    let img = two_tone(100, 100, C::RED, C::BLUE);
    let gray = pixels_with(
        json!([{"type": "image", "asset": "img", "width": 100, "height": 100, "filter": {"grayscale": 1}}]),
        &[("img", img.clone())],
    );
    let (r, g, b) = gray(25, 50);
    assert!(
        r == g && g == b && r > 30 && r < 90,
        "red to gray: {:?}",
        gray(25, 50)
    );
    let tinted = pixels_with(
        json!([{"type": "image", "asset": "img", "width": 100, "height": 100, "filter": {"tint": "#00FF00"}}]),
        &[("img", img.clone())],
    );
    assert_eq!(tinted(25, 50), (0, 255, 0));
    assert_eq!(tinted(75, 50), (0, 255, 0));
    let duo = pixels_with(
        json!([{"type": "image", "asset": "img", "width": 100, "height": 100, "filter": {"duotone": ["#000000", "#FFFF00"]}}]),
        &[("img", img)],
    );
    let (r, g, b) = duo(25, 50);
    assert!(
        r == g && b == 0 && r > 20,
        "red maps between black and yellow: {:?}",
        duo(25, 50)
    );
}

#[test]
fn text_takes_paint_stacks_and_stroke_lists() {
    let hollow = pixels(
        json!([{"type": "text", "text": "O", "x": 10, "y": 0, "fontSize": 90, "fontWeight": 900, "fill": [], "stroke": {"width": 4, "color": "#000000"}}]),
    );
    let dark = count(&hollow, |c| c.0 < 100);
    assert!(dark > 200, "the outline shows: {dark}");
    let solid = pixels(
        json!([{"type": "text", "text": "O", "x": 10, "y": 0, "fontSize": 90, "fontWeight": 900, "fill": "#000000"}]),
    );
    assert!(
        count(&solid, |c| c.0 < 100) > dark,
        "a filled O has more ink than its outline"
    );
}

#[test]
fn halftone_redraws_images_as_dots_sized_by_darkness() {
    let img = two_tone(100, 100, C::RED, C::WHITE);
    let dots = pixels_with(
        json!([{"type": "image", "asset": "img", "width": 100, "height": 100, "filter": {"halftone": 10}}]),
        &[("img", img)],
    );
    // Red is fairly dark: a black dot at each cell's center, gaps at its corners.
    assert_eq!(dots(25, 45), (0, 0, 0));
    assert!(dots(20, 40).0 > 200, "{:?}", dots(20, 40));
    // White has no dots at all.
    assert_eq!(dots(75, 45), (255, 255, 255));
}

#[test]
fn a_line_written_by_its_ends_runs_between_them() {
    // Down-left from (90, 10) to (10, 90), as agents write it: read as a
    // box plus a mirror, it must still ink the top-right, not the top-left.
    let mut line = json!({"type": "line",
        "x1": 90,
        "y1": 10,
        "x2": 10,
        "y2": 90,
        "stroke": {"width": 6, "color": "#000000", "align": "center"}});
    crate::ops::normalize(&mut line);
    let px = pixels(json!([line]));
    assert_eq!(px(88, 12), (0, 0, 0), "starts top-right");
    assert_eq!(px(12, 88), (0, 0, 0), "ends bottom-left");
    assert_eq!(px(12, 12), (255, 255, 255), "not the other diagonal");
}
