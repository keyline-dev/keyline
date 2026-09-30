//! Pixel tests: render small scenes and check what landed where.

use super::*;
use serde_json::json;
use skia_safe::Data;

#[test]
fn renders_deterministic_png_of_the_right_size() {
    let scene: Scene = serde_json::from_value(json!({"width": 200,
        "height": 100,
        "background": "#EEEEEE",
        "sizes": [{"id": "a", "width": 200, "height": 100}],
        "layers": [
            {"id": "r", "type": "rect", "x": 10, "y": 10, "width": 50, "height": 50, "borderRadius": 8, "fill": "#D0202E"},
            {"id": "t", "type": "text", "text": "Hi", "x": 80, "y": 20, "fontSize": 30, "fontWeight": 800}]}))
    .unwrap();
    let dir = std::env::temp_dir();
    let a = render_png(&scene, &scene.sizes[0], 1.0, &dir).unwrap();
    let b = render_png(&scene, &scene.sizes[0], 1.0, &dir).unwrap();
    assert_eq!(a, b);
    assert_eq!(raster_size(&a), Some((200.0, 100.0)));
    let preview = render_png(&scene, &scene.sizes[0], 0.5, &dir).unwrap();
    assert_eq!(raster_size(&preview), Some((100.0, 50.0)));
}

pub(super) fn pixels(layers: serde_json::Value) -> impl Fn(i32, i32) -> (u8, u8, u8) {
    pixels_with(layers, &[])
}

/// Renders a 100×100 scene whose `assets` are `(id, PNG bytes)`.
pub(super) fn pixels_with(
    layers: serde_json::Value,
    assets: &[(&str, Vec<u8>)],
) -> impl Fn(i32, i32) -> (u8, u8, u8) + use<> {
    let dir = std::env::temp_dir();
    let mut registry = serde_json::Map::new();
    for (id, png) in assets {
        let sha = crate::store::sha256_hex(png);
        // Tests run in parallel and share assets: write whole files only.
        let tmp = dir.join(format!("{sha}.{:?}.tmp", std::thread::current().id()));
        std::fs::write(&tmp, png).unwrap();
        std::fs::rename(&tmp, dir.join(&sha)).unwrap();
        let (w, h) = raster_size(png).unwrap();
        registry.insert(
            (*id).to_owned(),
            json!({"sha256": sha, "width": w, "height": h}),
        );
    }
    let mut v = json!({
        "width": 100, "height": 100, "background": "#FFFFFF",
        "sizes": [{"id": "a", "width": 100, "height": 100}],
    });
    v["layers"] = layers;
    v["assets"] = serde_json::Value::Object(registry);
    let scene: Scene = serde_json::from_value(v).unwrap();
    let img = render_image(&scene, &scene.sizes[0], 1.0, &dir, false).unwrap();
    move |x, y| {
        let c = img.peek_pixels().unwrap().get_color((x, y));
        (c.r(), c.g(), c.b())
    }
}

/// A `w`×`h` PNG, left half `left`, right half `right`.
pub(super) fn two_tone(w: i32, h: i32, left: skia_safe::Color, right: skia_safe::Color) -> Vec<u8> {
    let mut surface = surfaces::raster_n32_premul((w, h)).unwrap();
    let c = surface.canvas();
    let mut p = Paint::default();
    p.set_color(left);
    c.draw_rect(
        skia_safe::Rect::from_xywh(0.0, 0.0, w as f32 / 2.0, h as f32),
        &p,
    );
    p.set_color(right);
    c.draw_rect(
        skia_safe::Rect::from_xywh(w as f32 / 2.0, 0.0, w as f32 / 2.0, h as f32),
        &p,
    );
    encode_png(&surface.image_snapshot()).unwrap()
}

/// How many pixels of the 100×100 render satisfy `f`.
pub(super) fn count(
    px: &impl Fn(i32, i32) -> (u8, u8, u8),
    f: impl Fn((u8, u8, u8)) -> bool,
) -> usize {
    (0..100)
        .flat_map(|y| (0..100).map(move |x| (x, y)))
        .filter(|&(x, y)| f(px(x, y)))
        .count()
}

pub(super) const RED: (u8, u8, u8) = (255, 0, 0);
pub(super) const BLUE: (u8, u8, u8) = (0, 0, 255);

#[test]
fn tiles_repeat_at_the_image_size_times_tile_scale() {
    let tile = two_tone(10, 10, skia_safe::Color::RED, skia_safe::Color::BLUE);
    let px = pixels_with(
        json!([{"type": "image", "asset": "t", "width": 100, "height": 100, "fit": "tile"}]),
        &[("t", tile.clone())],
    );
    assert_eq!(
        [px(2, 5), px(7, 5), px(12, 5), px(17, 5), px(92, 95)],
        [RED, BLUE, RED, BLUE, RED]
    );
    let px = pixels_with(
        json!([{"type": "image", "asset": "t", "width": 100, "height": 100, "fit": "tile", "tileScale": 2}]),
        &[("t", tile)],
    );
    assert_eq!([px(5, 5), px(15, 5), px(25, 5)], [RED, BLUE, RED]);
}

#[test]
fn ellipses_lines_and_icons_draw_where_their_box_says() {
    let px = pixels(json!([{"type": "ellipse", "width": 100, "height": 100, "fill": "#FF0000"}]));
    // Inside the circle, not its bounding box's corner.
    assert_eq!((px(50, 50), px(3, 3)), (RED, (255, 255, 255)));
    let px = pixels(
        json!([{"type": "line", "y": 50, "width": 100, "stroke": {"width": 4, "color": "#0000FF"}}]),
    );
    assert_eq!((px(50, 50), px(50, 45)), (BLUE, (255, 255, 255)));
    // A solid icon fills with its color; its box is contained, centered.
    let px = pixels(json!([
        {"type": "icon", "name": "square", "set": "solid", "color": "#FF0000", "width": 100, "height": 100},
    ]));
    assert_eq!(px(50, 50), RED);
    let red = count(&px, |c| c == RED);
    assert!(red > 5000, "{red}");
}

#[test]
fn masks_fade_a_layer_and_focus_picks_what_a_crop_keeps() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": "#0000FF", "mask": {"from": [0.5, 0], "to": [0.5, 1], "stops": [{"offset": 0, "color": "#00000000"}, {"offset": 1, "color": "#000000"}]}}]),
    );
    // Transparent at the top, opaque at the bottom, half-way between.
    // (Pixel centers sit half a pixel into the gradient, so ±1.)
    let near = |(r, g, b): (u8, u8, u8), (x, y, z): (u8, u8, u8)| {
        r.abs_diff(x) <= 1 && g.abs_diff(y) <= 1 && b.abs_diff(z) <= 1
    };
    assert!(near(px(50, 0), (255, 255, 255)) && near(px(50, 99), BLUE));
    let (r, _, b) = px(50, 50);
    assert!(r > 100 && r < 160 && b == 255, "{:?}", px(50, 50));
    // A 20×10 image, left red, right blue, cover-cropped to a square:
    // the focus decides which half stays.
    let img = two_tone(20, 10, skia_safe::Color::RED, skia_safe::Color::BLUE);
    let layer = |focus: [f32; 2]| json!([{"type": "image", "asset": "i", "width": 100, "height": 100, "focus": focus}]);
    let left = pixels_with(layer([0.0, 0.5]), &[("i", img.clone())]);
    let right = pixels_with(layer([1.0, 0.5]), &[("i", img)]);
    assert_eq!((left(50, 50), right(50, 50)), (RED, BLUE));
}

#[test]
fn text_can_be_filled_with_an_image_or_a_gradient() {
    let text = |extra: serde_json::Value| {
        let mut t = json!({"type": "text", "text": "III", "fontSize": 90, "fontWeight": 900, "color": "#00FF00"});
        t.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        json!([t])
    };
    let image = two_tone(20, 20, skia_safe::Color::RED, skia_safe::Color::BLUE);
    let px = pixels_with(text(json!({"fill": {"image": "img"}})), &[("img", image)]);
    // The letters show the image's red and blue, never the text color.
    assert!(count(&px, |c| c == RED) > 50 && count(&px, |c| c == BLUE) > 50);
    assert_eq!(count(&px, |c| c.1 > 200 && c.0 < 50), 0);

    let px = pixels(text(
        json!({"fill": {"gradient": {"angle": 90, "stops": ["#FF0000", "#0000FF"]}}}),
    ));
    assert!(
        count(&px, |c| c.0 > 200 && c.2 < 60) > 20 && count(&px, |c| c.2 > 200 && c.0 < 60) > 20
    );
}

#[test]
fn outlined_text_can_be_hollow() {
    let px = pixels(
        json!([{"type": "text", "text": "III", "fontSize": 90, "fontWeight": 900, "color": "#00000000", "stroke": {"width": 4, "color": "#FF0000", "align": "center"}}]),
    );
    assert!(count(&px, |c| c == RED) > 50, "outline drawn");
    assert_eq!(
        count(&px, |c| c.0 < 100 && c.1 < 100 && c.2 < 100),
        0,
        "no dark fill"
    );
    let filled =
        pixels(json!([{"type": "text", "text": "III", "fontSize": 90, "fontWeight": 900}]));
    assert!(count(&filled, |c| c == RED) == 0 && count(&filled, |c| c.0 < 30) > 200);
}

#[test]
fn gradients_run_between_their_points() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"gradient": {"from": [0, 0.5], "to": [1, 0.5], "stops": [{"offset": 0, "color": "#000000"}, {"offset": 1, "color": "#FFFFFF"}]}}}]),
    );
    assert!(px(2, 50).0 < 20 && px(97, 50).0 > 235 && (px(50, 50).0 as i32 - 128).abs() < 10);
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": {"gradient": {"from": [0.5, 0], "to": [0.5, 1], "stops": [{"offset": 0, "color": "#FF0000"}, {"offset": 1, "color": "#0000FF"}]}}}]),
    );
    assert!(px(50, 2).0 > 240 && px(50, 97).2 > 240, "top to bottom");
}

#[test]
fn strokes_sit_inside_center_or_outside_the_edge() {
    let stroke = |align: &str| {
        pixels(
            json!([{"type": "rect", "x": 30, "y": 30, "width": 40, "height": 40,
            "stroke": {"width": 6, "color": "#FF0000", "align": align}}]),
        )
    };
    let red = (255, 0, 0);
    let inside = stroke("inside");
    assert_eq!((inside(32, 50), inside(28, 50)), (red, (255, 255, 255)));
    let outside = stroke("outside");
    assert_eq!((outside(28, 50), outside(32, 50)), (red, (255, 255, 255)));
    let center = stroke("center");
    assert_eq!((center(28, 50), center(32, 50)), (red, red));
}

#[test]
fn rotation_turns_the_layer_and_its_children_about_the_center() {
    // A 60×10 bar through the middle, turned 90°, stands upright.
    let px = pixels(
        json!([{"type": "frame", "x": 20, "y": 45, "width": 60, "height": 10, "rotate": 90, "children": [{"type": "rect", "width": 60, "height": 10, "fill": "#000000"}]}]),
    );
    assert_eq!(px(50, 25), (0, 0, 0));
    assert_eq!(px(25, 50), (255, 255, 255));
}

#[test]
fn blend_modes_composite_with_what_is_below() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "fill": "#FF0000"}, {"type": "rect", "width": 50, "height": 100, "blendMode": "multiply", "fill": "#00FF00"}]),
    );
    assert_eq!(px(25, 50), (0, 0, 0), "red × green = black");
    assert_eq!(px(75, 50), (255, 0, 0));
}

#[test]
fn gpu_renders_match_the_cpu_closely() {
    if !gpu::available() {
        eprintln!("no GPU on this machine; the CPU fallback covers renders");
        return;
    }
    let scene: Scene = serde_json::from_value(json!({"width": 200,
        "height": 100,
        "background": "#EEEEEE",
        "sizes": [{"id": "a", "width": 200, "height": 100}],
        "layers": [
            {"id": "r", "type": "rect", "x": 10, "y": 10, "width": 80, "height": 80, "borderRadius": 12, "rotate": 10, "fill": {"gradient": {"stops": [{"offset": 0, "color": "#D0202E"}, {"offset": 1, "color": "#1B2A5C"}], "angle": 90}}},
            {"id": "t", "type": "text", "text": "GPU", "x": 110, "y": 20, "fontSize": 40, "fontWeight": 800, "color": "#FFFFFF", "stroke": {"width": 2, "color": "#000000", "align": "center"}}]}))
    .unwrap();
    let dir = std::env::temp_dir();
    let cpu = render_png_on(&scene, &scene.sizes[0], &dir, Backend::Cpu).unwrap();
    let gpu = render_png_on(&scene, &scene.sizes[0], &dir, Backend::Gpu).unwrap();
    assert_eq!(raster_size(&gpu), Some((200.0, 100.0)));
    let decode = |png: &[u8]| {
        Image::from_encoded(Data::new_copy(png))
            .and_then(|i| i.make_raster_image(None, None))
            .unwrap()
    };
    let (a, b) = (decode(&cpu), decode(&gpu));
    let (pa, pb) = (a.peek_pixels().unwrap(), b.peek_pixels().unwrap());
    // Same drawing, different rasterizer: edges antialias differently,
    // so compare the average difference, not a count of pixels.
    let mut total = 0u64;
    for y in 0..100 {
        for x in 0..200 {
            let (ca, cb) = (pa.get_color((x, y)), pb.get_color((x, y)));
            total += u64::from(ca.r().abs_diff(cb.r()))
                + u64::from(ca.g().abs_diff(cb.g()))
                + u64::from(ca.b().abs_diff(cb.b()));
        }
    }
    let mean = total as f64 / (200.0 * 100.0 * 3.0);
    assert!(
        mean < 2.0,
        "GPU and CPU differ by {mean:.2} levels per channel on average"
    );
}

/// Renders a 100×100, 2 s scene of `layers` `t` seconds in.
fn pixels_at(layers: serde_json::Value, t: f32) -> impl Fn(i32, i32) -> (u8, u8, u8) {
    pixels_in(
        layers,
        json!([{"id": "a", "width": 100, "height": 100}]),
        0,
        t,
    )
}

/// Renders size `n` of a 100×100, 2 s scene of `layers` with `sizes`,
/// `t` seconds in.
fn pixels_in(
    layers: serde_json::Value,
    sizes: serde_json::Value,
    n: usize,
    t: f32,
) -> impl Fn(i32, i32) -> (u8, u8, u8) {
    let mut v = json!({"width": 100, "height": 100, "background": "#FFFFFF", "duration": 2});
    v["layers"] = layers;
    v["sizes"] = sizes;
    let scene: Scene = serde_json::from_value(v).unwrap();
    scene.validate().unwrap();
    let scene = scene.resolved().into_owned();
    let size = &scene.sizes[n];
    let at = crate::anim::at_time(&scene, t, size);
    let img = render_image(&at, size, 1.0, &std::env::temp_dir(), false).unwrap();
    move |x, y| {
        let c = img.peek_pixels().unwrap().get_color((x, y));
        (c.r(), c.g(), c.b())
    }
}

#[test]
fn a_draw_track_draws_that_share_of_the_stroke() {
    let line = json!([{"type": "line", "x": 10, "y": 50, "width": 80, "height": 0,
        "stroke": {"width": 6, "color": "#000000"}, "animate": {"draw": [0, 1], "ease": "none"}}]);
    let half = pixels_at(line.clone(), 0.5);
    assert_eq!(
        (half(20, 50), half(80, 50)),
        ((0, 0, 0), (255, 255, 255)),
        "half drawn"
    );
    let rest = pixels_at(line, 0.0);
    assert_eq!(rest(20, 50), (255, 255, 255), "nothing before it starts");
    // A ring draws clockwise from the top: a quarter covers the right-top only.
    let ring = json!([{"type": "ellipse", "x": 20, "y": 20, "width": 60, "height": 60,
        "stroke": {"width": 6, "color": "#FF0000"}, "animate": {"draw": {"from": 0, "to": 0.25}, "delay": 1, "duration": 0.01}}]);
    let q = pixels_at(ring, 1.5);
    assert!(q(65, 24).1 < 100, "top right: {:?}", q(65, 24));
    assert_eq!(q(35, 76), (255, 255, 255), "bottom left not yet");
    assert_eq!(q(24, 35), (255, 255, 255), "left not yet");
}

#[test]
fn a_count_track_draws_the_number_of_the_moment() {
    let text = json!([{"id": "n", "type": "text", "text": "{{n}}", "fontSize": 40, "color": "#000000",
        "animate": {"count": [0, 1000], "ease": "none"}}]);
    let ink = |px: &dyn Fn(i32, i32) -> (u8, u8, u8)| {
        (0..100)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| px(x, y).0 < 128)
            .count()
    };
    let start = pixels_at(text.clone(), 0.0);
    let end = pixels_at(text, 2.0);
    assert!(ink(&start) * 3 < ink(&end), "one digit, then four");
}

/// Dark pixels in a `w`×`h` area from the top left.
fn dark(px: &dyn Fn(i32, i32) -> (u8, u8, u8), w: i32, h: i32) -> usize {
    (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|&(x, y)| px(x, y).0 < 128)
        .count()
}

#[test]
fn media_keeps_split_text_and_tracks_moving() {
    let sizes =
        json!([{"id": "a", "width": 100, "height": 100}, {"id": "b", "width": 100, "height": 100}]);
    let split = json!({"id": "t", "type": "text", "text": "Hello", "fontSize": 30, "color": "#000000",
        "split": "chars", "enter": {"effect": "fade", "duration": 0.5}});
    let mut on_text = split.clone();
    on_text["media"] = json!({"b": {"color": "#000010"}});
    let on_parent = json!({"type": "frame", "width": 100, "height": 100,
        "media": {"b": {"opacity": 1}}, "children": [split]});
    for layers in [json!([on_text]), json!([on_parent])] {
        let start = pixels_in(layers.clone(), sizes.clone(), 1, 0.0);
        let end = pixels_in(layers.clone(), sizes.clone(), 1, 1.0);
        assert_eq!(dark(&start, 100, 100), 0, "nothing yet: {layers}");
        assert!(dark(&end, 100, 100) > 50, "all there: {layers}");
    }
    // Self-drawing strokes and counting numbers too.
    let line = json!([{"type": "line", "x": 10, "y": 50, "width": 80, "height": 0,
        "stroke": {"width": 6, "color": "#000000"}, "animate": {"draw": [0, 1], "ease": "none"},
        "media": {"b": {"opacity": 1}}}]);
    let half = pixels_in(line, sizes.clone(), 1, 0.5);
    assert_eq!(half(80, 50), (255, 255, 255), "half drawn");
    let count = json!([{"type": "text", "text": "{{n}}", "fontSize": 40, "color": "#000000",
        "animate": {"count": [0, 1000], "ease": "none"}, "media": {"b": {"opacity": 1}}}]);
    let start = pixels_in(count.clone(), sizes.clone(), 1, 0.0);
    let end = pixels_in(count, sizes, 1, 2.0);
    assert!(
        dark(&start, 100, 60) * 3 < dark(&end, 100, 60),
        "one digit, then four"
    );
}

#[test]
fn media_can_give_a_size_its_own_motion() {
    let sizes =
        json!([{"id": "a", "width": 100, "height": 100}, {"id": "b", "width": 100, "height": 100}]);
    let layers = json!([{"type": "rect", "width": 100, "height": 100, "fill": "#000000",
        "media": {"b": {"enter": {"effect": "fade", "delay": 1}}}}]);
    let a = pixels_in(layers.clone(), sizes.clone(), 0, 0.5);
    let b = pixels_in(layers, sizes, 1, 0.5);
    assert_eq!((a(50, 50), b(50, 50)), ((0, 0, 0), (255, 255, 255)));
}

#[test]
fn a_translate_track_moves_by_the_size_scale() {
    let sizes = json!([{"id": "a", "width": 100, "height": 100},
        {"id": "s", "width": 75, "height": 75, "scale": 0.75}]);
    // A 20 px square moved 40 px right by the end: 30 px at scale 0.75.
    let layers = json!([{"type": "rect", "x": 0, "y": 0, "width": 20, "height": 20, "fill": "#000000",
        "animate": {"translate": {"from": [0, 0], "to": [40, 0]}, "duration": 0.1}}]);
    let px = pixels_in(layers, sizes, 1, 1.0);
    let left = (0..75).find(|&x| px(x, 7).0 < 128).unwrap();
    assert_eq!(left, 30, "moved 40 × 0.75");
}

#[test]
fn a_stroked_line_that_changes_size_keeps_every_run() {
    let text = json!([{"type": "text", "x": 2, "y": 20, "fontSize": 30, "color": "#0000",
        "text": "<span style=\"font-size:16px\">MM</span> MM", "stroke": {"color": "#000000", "width": 2}}]);
    let px = pixels_at(text, 0.0);
    let left = (0..20)
        .flat_map(|x| (20..60).map(move |y| (x, y)))
        .filter(|&(x, y)| px(x, y).0 < 128)
        .count();
    assert!(
        left > 20,
        "the small run is outlined where it's set: {left}"
    );
}

#[test]
fn a_gradient_mask_hides_everything_outside_its_box() {
    // A fade to nothing whose bottom edge falls on a half pixel, and a child
    // hanging below the masked frame.
    let layers = json!([
        {"id": "a", "type": "rect", "x": 10, "y": 10, "width": 30, "height": 50.5, "fill": "#000000",
         "mask": {"angle": 180, "stops": ["#000000", "#00000000"]}},
        {"id": "b", "type": "frame", "x": 50, "y": 10, "width": 30, "height": 50.5, "fill": "#000000",
         "clipsContent": false, "mask": {"angle": 180, "stops": ["#000000", "#00000000"]},
         "children": [{"id": "c", "type": "rect", "y": 60, "width": 30, "height": 10, "fill": "#FF0000"}]}]);
    let px = pixels_at(layers, 0.0);
    assert!(px(25, 12).0 < 30, "masked in at the top: {:?}", px(25, 12));
    assert!(px(25, 60).0 > 245, "edge row faded out: {:?}", px(25, 60));
    assert_eq!(px(65, 75), (255, 255, 255), "outside the mask's box");
}

#[test]
fn dashes_on_an_ellipse_start_at_the_top() {
    let ring = json!([{"type": "ellipse", "x": 20, "y": 20, "width": 60, "height": 60,
        "stroke": {"width": 4, "color": "#000000", "dash": [12, 400]}}]);
    let px = pixels_at(ring, 0.0);
    assert!(
        px(55, 22).0 < 100,
        "the dash starts at 12 o'clock: {:?}",
        px(55, 22)
    );
    assert_eq!(px(78, 50), (255, 255, 255), "nothing at 3 o'clock");
    assert_eq!(px(45, 22), (255, 255, 255), "clockwise, not back");
}

#[test]
fn split_text_draws_its_highlight_whole_and_with_its_first_letter() {
    let text = json!([{"id": "t", "type": "text", "x": 5, "y": 30, "fontSize": 24, "color": "#000000",
        "text": "Hi you", "highlight": "#FFD400", "split": "chars",
        "enter": {"effect": "fade", "duration": 0.5}}]);
    let start = pixels_at(text.clone(), 0.0);
    assert!(
        (0..100).all(|x| start(x, 45) == (255, 255, 255)),
        "no highlight before its letters"
    );
    let end = pixels_at(text, 1.0);
    // Across the line, spaces and letter joins included: highlight or ink.
    let row: Vec<_> = (0..100).map(|x| end(x, 45)).collect();
    let first = row.iter().position(|&c| c != (255, 255, 255)).unwrap();
    let last = row.iter().rposition(|&c| c != (255, 255, 255)).unwrap();
    assert!(last - first > 40, "{first}..{last}");
    let seams: Vec<_> = (first + 2..last - 2)
        .filter(|&x| row[x].2 > 60 && row[x].0 > 200)
        .collect();
    assert!(seams.is_empty(), "unhighlighted at {seams:?}");
}

#[test]
fn leader_lines_keep_their_markup_on_one_baseline() {
    let menu = json!([{"id": "m", "type": "text", "x": 0, "y": 20, "width": 100, "fontSize": 14,
        "color": "#000000", "leader": ".",
        "text": "<b>Soda</b>\t<span style=\"color:#FF0000;font-size:22px\">90</span>"}]);
    let px = pixels_at(menu, 0.0);
    let red = |(r, g, b): (u8, u8, u8)| r > 200 && g < 80 && b < 80;
    let dark = |(r, g, b): (u8, u8, u8)| r < 100 && g < 100 && b < 100;
    let rows = |x0: i32, x1: i32, f: &dyn Fn((u8, u8, u8)) -> bool| {
        (0..100)
            .filter(|&y| (x0..x1).any(|x| f(px(x, y))))
            .collect::<Vec<_>>()
    };
    let price = rows(70, 100, &red);
    assert!(!price.is_empty(), "the price keeps its colour");
    let soda = rows(0, 30, &dark);
    // "90" at 22 px stands taller than "Soda" at 14 px, on the same baseline.
    assert!(price.len() > soda.len() + 3, "the price keeps its size");
    let (sb, pb) = (*soda.last().unwrap(), *price.last().unwrap());
    assert!(sb.abs_diff(pb) <= 1, "one baseline: {sb} vs {pb}");
}

#[test]
fn a_still_shows_where_a_draw_track_comes_to_rest() {
    let v = json!({"width": 100, "height": 100, "background": "#FFFFFF",
        "sizes": [{"id": "a", "width": 100, "height": 100}],
        "layers": [{"id": "ring", "type": "ellipse", "x": 20, "y": 20, "width": 60, "height": 60,
            "stroke": {"width": 6, "color": "#000000"}, "animate": {"draw": [0, 0.75]}}]});
    let scene: Scene = serde_json::from_value(v).unwrap();
    scene.validate().unwrap();
    let img = render_image(&scene, &scene.sizes[0], 1.0, &std::env::temp_dir(), false).unwrap();
    let px = |x, y| img.peek_pixels().unwrap().get_color((x, y)).r();
    // Clockwise from the top: right, bottom and left drawn; the last quarter not.
    assert!(px(77, 50) < 100 && px(50, 77) < 100 && px(23, 50) < 100);
    assert!(px(35, 25) > 200, "top left still open: {}", px(35, 25));
}

#[test]
fn a_stroke_only_arc_is_open() {
    // The right half, top to bottom: no radii back to the center.
    let arc = json!([{"id": "a", "type": "ellipse", "x": 20, "y": 20, "width": 60, "height": 60,
        "arc": {"start": 0, "end": 180}, "fill": [], "stroke": {"width": 4, "color": "#000000"}}]);
    let px = pixels_at(arc, 0.0);
    assert!(px(77, 50).0 < 100, "the arc: {:?}", px(77, 50));
    for y in 25..76 {
        assert_eq!(px(50, y), (255, 255, 255), "a radius at y {y}");
    }
}
