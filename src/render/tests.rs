//! Pixel tests: render small scenes and check what landed where.

use super::*;
use serde_json::json;
use skia_safe::Data;

#[test]
fn renders_deterministic_png_of_the_right_size() {
    let scene: Scene = serde_json::from_value(json!({
        "width": 200, "height": 100, "background": "#EEEEEE",
        "sizes": [{"id": "a", "width": 200, "height": 100}],
        "layers": [
            {"id": "r", "type": "rect", "x": 10, "y": 10, "width": 50, "height": 50, "color": "#D0202E", "cornerRadius": 8},
            {"id": "t", "type": "text", "text": "Hi", "x": 80, "y": 20, "fontSize": 30, "weight": 800}
        ]
    }))
    .unwrap();
    let dir = std::env::temp_dir();
    let a = render_png(&scene, &scene.sizes[0], 1.0, &dir).unwrap();
    let b = render_png(&scene, &scene.sizes[0], 1.0, &dir).unwrap();
    assert_eq!(a, b);
    assert_eq!(raster_size(&a), Some((200.0, 100.0)));
    let preview = render_png(&scene, &scene.sizes[0], 0.5, &dir).unwrap();
    assert_eq!(raster_size(&preview), Some((100.0, 50.0)));
}

fn pixels(layers: serde_json::Value) -> impl Fn(i32, i32) -> (u8, u8, u8) {
    pixels_with(layers, &[])
}

/// Renders a 100×100 scene whose `assets` are `(id, PNG bytes)`.
fn pixels_with(
    layers: serde_json::Value,
    assets: &[(&str, Vec<u8>)],
) -> impl Fn(i32, i32) -> (u8, u8, u8) + use<> {
    let dir = std::env::temp_dir();
    let mut registry = serde_json::Map::new();
    for (id, png) in assets {
        let sha = crate::store::sha256_hex(png);
        std::fs::write(dir.join(&sha), png).unwrap();
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
fn two_tone(w: i32, h: i32, left: skia_safe::Color, right: skia_safe::Color) -> Vec<u8> {
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
fn count(px: &impl Fn(i32, i32) -> (u8, u8, u8), f: impl Fn((u8, u8, u8)) -> bool) -> usize {
    (0..100)
        .flat_map(|y| (0..100).map(move |x| (x, y)))
        .filter(|&(x, y)| f(px(x, y)))
        .count()
}

const RED: (u8, u8, u8) = (255, 0, 0);
const BLUE: (u8, u8, u8) = (0, 0, 255);

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
    let px = pixels(json!([
        {"type": "ellipse", "width": 100, "height": 100, "color": "#FF0000"},
    ]));
    // Inside the circle, not its bounding box's corner.
    assert_eq!((px(50, 50), px(3, 3)), (RED, (255, 255, 255)));
    let px = pixels(json!([
        {"type": "line", "y": 50, "width": 100, "color": "#0000FF", "strokeWidth": 4},
    ]));
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
    let px = pixels(json!([
        {"type": "rect", "width": 100, "height": 100, "color": "#0000FF",
         "mask": {"from": [0.5, 0], "to": [0.5, 1], "stops": [{"at": 0, "color": "#00000000"}, {"at": 1, "color": "#000000"}]}},
    ]));
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
        let mut t = json!({"type": "text", "text": "III", "fontSize": 90, "weight": 900, "color": "#00FF00"});
        t.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        json!([t])
    };
    let image = two_tone(20, 20, skia_safe::Color::RED, skia_safe::Color::BLUE);
    let px = pixels_with(text(json!({"fill": {"asset": "img"}})), &[("img", image)]);
    // The letters show the image's red and blue, never the text color.
    assert!(count(&px, |c| c == RED) > 50 && count(&px, |c| c == BLUE) > 50);
    assert_eq!(count(&px, |c| c.1 > 200 && c.0 < 50), 0);

    let px = pixels(text(
        json!({"gradient": {"stops": [{"at": 0, "color": "#FF0000"}, {"at": 1, "color": "#0000FF"}]}}),
    ));
    assert!(
        count(&px, |c| c.0 > 200 && c.2 < 60) > 20 && count(&px, |c| c.2 > 200 && c.0 < 60) > 20
    );
}

#[test]
fn outlined_text_can_be_hollow() {
    let px = pixels(
        json!([{"type": "text", "text": "III", "fontSize": 90, "weight": 900,
        "color": "#00000000", "outline": {"width": 4, "color": "#FF0000"}}]),
    );
    assert!(count(&px, |c| c == RED) > 50, "outline drawn");
    assert_eq!(
        count(&px, |c| c.0 < 100 && c.1 < 100 && c.2 < 100),
        0,
        "no dark fill"
    );
    let filled = pixels(json!([{"type": "text", "text": "III", "fontSize": 90, "weight": 900}]));
    assert!(count(&filled, |c| c == RED) == 0 && count(&filled, |c| c.0 < 30) > 200);
}

#[test]
fn gradients_run_between_their_points() {
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "gradient":
        {"from": [0, 0.5], "to": [1, 0.5], "stops": [{"at": 0, "color": "#000000"}, {"at": 1, "color": "#FFFFFF"}]}}]),
    );
    assert!(px(2, 50).0 < 20 && px(97, 50).0 > 235 && (px(50, 50).0 as i32 - 128).abs() < 10);
    let px = pixels(
        json!([{"type": "rect", "width": 100, "height": 100, "gradient":
        {"from": [0.5, 0], "to": [0.5, 1], "stops": [{"at": 0, "color": "#FF0000"}, {"at": 1, "color": "#0000FF"}]}}]),
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
        json!([{"type": "frame", "x": 20, "y": 45, "width": 60, "height": 10, "rotation": 90,
        "children": [{"type": "rect", "width": 60, "height": 10, "color": "#000000"}]}]),
    );
    assert_eq!(px(50, 25), (0, 0, 0));
    assert_eq!(px(25, 50), (255, 255, 255));
}

#[test]
fn blend_modes_composite_with_what_is_below() {
    let px = pixels(json!([
        {"type": "rect", "width": 100, "height": 100, "color": "#FF0000"},
        {"type": "rect", "width": 50, "height": 100, "color": "#00FF00", "blendMode": "multiply"}
    ]));
    assert_eq!(px(25, 50), (0, 0, 0), "red × green = black");
    assert_eq!(px(75, 50), (255, 0, 0));
}

#[test]
fn gpu_renders_match_the_cpu_closely() {
    if !gpu::available() {
        eprintln!("no GPU on this machine; the CPU fallback covers renders");
        return;
    }
    let scene: Scene = serde_json::from_value(json!({
        "width": 200, "height": 100, "background": "#EEEEEE",
        "sizes": [{"id": "a", "width": 200, "height": 100}],
        "layers": [
            {"id": "r", "type": "rect", "x": 10, "y": 10, "width": 80, "height": 80, "cornerRadius": 12,
             "gradient": {"stops": [{"at": 0, "color": "#D0202E"}, {"at": 1, "color": "#1B2A5C"}]}, "rotation": 10},
            {"id": "t", "type": "text", "text": "GPU", "x": 110, "y": 20, "fontSize": 40, "weight": 800,
             "outline": {"width": 2, "color": "#000000"}, "color": "#FFFFFF"}
        ]
    }))
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
