//! End-to-end: an agent paints through MCP — fill stacks,
//! gradients, image fills in shapes, patterns, grain, shadows, backdrop
//! blur, radii, dashed strokes with markers, polygons, named shapes, arcs,
//! masks, adjustments and transforms — and the renders match golden PNGs.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::golden::check_golden;
use common::{Mcp, b64, photo_png};
use serde_json::json;

#[tokio::test]
async fn the_paint_kit_renders_at_every_size() {
    let mcp = Mcp::start("paint").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 800, "height": 800, "sizes": [
                {"id": "square", "width": 800, "height": 800},
                {"id": "banner", "width": 1200, "height": 628, "scale": 0.78}
            ]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "photo", "base64": b64(&photo_png())}),
    )
    .await;
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "layers": [
                {"id": "bg", "type": "rect", "width": "fill", "height": "fill", "fill": [{"gradient": {"angle": 135, "stops": ["#1B2A5C", "#D0202E"]}}, {"noise": 0.08, "seed": 3}]},
                {"id": "stripes", "type": "rect", "y": 700, "width": "fill", "height": 100, "constraints": {"vertical": "bottom"}, "fill": {"pattern": "stripes", "color": "#FFFFFF22", "size": 24, "angle": 45}},
                {"id": "card", "type": "frame", "x": 60, "y": 60, "width": 420, "height": 300, "borderRadius": 28, "fill": "#FFFFFF", "shadow": [{"y": 18, "blur": 40, "color": "#00000066"}], "children": [{"id": "avatar", "type": "ellipse", "x": 30, "y": 30, "width": 120, "height": 120, "fill": {"image": "photo", "fit": "cover"}, "stroke": {"width": 6, "color": "#D0202E", "align": "outside"}}, {"id": "gray", "type": "image", "asset": "photo", "x": 180, "y": 30, "width": 210, "height": 120, "borderRadius": [24, 0, 24, 0], "filter": {"grayscale": 1, "contrast": 1.2}}, {"id": "headline", "type": "text", "x": 30, "y": 180, "width": 360, "text": "Bold paint", "fontSize": 56, "fontWeight": 900, "fill": {"gradient": {"stops": ["#D0202E", "#1B2A5C"], "angle": 90}}, "stroke": {"width": 2, "color": "#1B2A5C"}}, {"id": "divider", "type": "line", "x": 30, "y": 270, "width": 330, "stroke": {"width": 3, "color": "#1B2A5C", "dash": [12, 8], "cap": "round", "markerEnd": "triangle"}}]},
                {"id": "glass", "type": "rect", "x": 380, "y": 280, "width": 360, "height": 220, "borderRadius": 32, "backdropBlur": 24, "fill": "#FFFFFF33", "stroke": {"width": 1.5, "color": "#FFFFFF88"}, "shadow": {"blur": 0, "spread": 0, "x": 0, "y": 0, "color": "#FFFFFF55", "inset": true}},
                {"id": "seal", "type": "polygon", "sides": 16, "innerRadius": 0.82, "width": 170, "height": 170, "place": "top-right", "margin": 50, "borderRadius": 6, "shadow": {"y": 6, "blur": 12, "color": "#0000004D"}, "fill": "#FFD700"},
                {"id": "sale", "type": "text", "text": "SALE", "fontSize": 40, "fontWeight": 900, "color": "#1B2A5C", "place": "top-right", "margin": [80, 113], "rotate": -12},
                {"id": "ribbon", "type": "path", "shape": "ribbon", "x": 60, "y": 420, "width": 280, "height": 70, "fill": "#FFD700", "fit": "fill"},
                {"id": "heart", "type": "path", "shape": "heart", "x": 90, "y": 540, "width": 110, "height": 110, "fill": {"gradient": {"type": "radial", "stops": ["#FF7A8A", "#D0202E"]}}, "skew": [-8, 0]},
                {"id": "gauge-track", "type": "ellipse", "x": 250, "y": 530, "width": 140, "height": 140, "arc": {"inner": 0.78}, "fill": "#FFFFFF33"},
                {"id": "gauge", "type": "ellipse", "x": 250, "y": 530, "width": 140, "height": 140, "fill": {"gradient": {"type": "conic", "stops": ["#FFD700", "#D0202E"]}}, "arc": {"start": 0, "end": 250, "inner": 0.78}},
                {"id": "portrait", "type": "image", "asset": "photo", "x": 440, "y": 540, "width": 300, "height": 200, "mask": {"shape": "blob-3"}, "flipX": true}]}))
        .await;
    assert!(
        reply
            .lines()
            .next()
            .unwrap()
            .starts_with("added bg,stripes,card"),
        "{reply}"
    );

    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for (size, path) in common::files(&rendered) {
        check_golden(&format!("paint-{size}.png"), &std::fs::read(path).unwrap());
    }
    mcp.stop().await;
}

#[tokio::test]
async fn bad_paint_gets_one_line_errors() {
    let mcp = Mcp::start("paint-errors").await;
    let id = mcp
        .ok("scene_create", json!({"sizes": ["600x600"]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let err = |layer: serde_json::Value| {
        let mcp = &mcp;
        let id = id.clone();
        async move {
            let e = mcp
                .call("layer_add", json!({"sceneId": id, "layers": [layer]}))
                .await
                .unwrap_err();
            assert_eq!(e.lines().count(), 1, "{e}");
            e
        }
    };
    let e = err(json!({"type": "rect", "fill": {"colour": "#000"}})).await;
    assert!(e.contains("a fill is a color, or an object with"), "{e}");
    let e = err(json!({"type": "rect", "fill": {"image": "nope"}})).await;
    assert!(e.contains("unknown asset nope"), "{e}");
    let e = err(json!({"type": "path", "shape": "unicorn"})).await;
    assert!(
        e.contains("unknown shape unicorn") && e.contains("ribbon"),
        "{e}"
    );
    let e = err(json!({"type": "rect", "mask": {"layer": "ghost"}})).await;
    assert!(e.contains("mask layer ghost not found"), "{e}");
    let e = err(json!({"type": "polygon", "sides": 2})).await;
    assert!(e.contains("polygon sides must be >= 3"), "{e}");
    let e = err(json!({"type": "rect", "borderRadius": "round"})).await;
    assert!(e.contains("\"full\""), "{e}");
    mcp.stop().await;
}

#[tokio::test]
async fn torn_edges_rough_strokes_and_halftone_look_hand_made() {
    let mcp = Mcp::start("hand").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 600, "height": 400, "sizes": [{"id": "card", "width": 600, "height": 400}]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "photo", "base64": b64(&photo_png())}),
    )
    .await;
    mcp.ok("layer_add", json!({"sceneId": id,
        "layers": [
            {"type": "rect", "width": "fill", "height": "fill", "fill": "#F4F1EA"},
            {"id": "photo", "type": "image", "asset": "photo", "x": 40, "y": 40, "width": 320, "height": 220, "edges": {"sides": ["bottom", "right"], "depth": 14, "seed": 2}},
            {"id": "label", "type": "rect", "x": 60, "y": 290, "width": 280, "height": 60, "edges": {"depth": 6, "seed": 5}, "fill": "#D0202E"},
            {"id": "ring", "type": "ellipse", "x": 400, "y": 60, "width": 160, "height": 120, "stroke": {"width": 5, "color": "#1B2A5C", "roughness": 3, "seed": 1, "align": "center"}},
            {"id": "arrow", "type": "line", "x": 400, "y": 240, "width": 150, "height": 80, "stroke": {"width": 4, "color": "#1B2A5C", "roughness": 2, "cap": "round", "markerEnd": "arrow"}},
            {"id": "dots", "type": "image", "asset": "photo", "x": 400, "y": 200, "width": 60, "height": 40, "filter": {"halftone": 5, "tint": "#D0202E"}}]}))
        .await;
    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    let path = common::file_of(rendered.lines().next().unwrap()).unwrap().1;
    check_golden("hand-card.png", &std::fs::read(path).unwrap());
    let e = mcp
        .call(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"id": "label"}, "set": {"edges": {"sides": ["up"]}}}]}),
        )
        .await
        .unwrap_err();
    assert!(e.contains("unknown variant `up`"), "{e}");
    mcp.stop().await;
}

#[tokio::test]
async fn a_band_too_short_for_its_photo_is_a_defect() {
    let mcp = Mcp::start("crop-warn").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 1200, "height": 1000, "sizes": [{"id": "wide", "width": 1200, "height": 1000}]}),
        )
        .await
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "photo", "base64": b64(&photo_png())}),
    )
    .await;
    // A 1600×900 photo in a 1200×238 band: cover hides 65% of its height.
    let reply = mcp
        .ok(
            "layer_add",
            json!({"sceneId": id, "layers": [{"id": "photo-band", "type": "image", "asset": "photo", "y": 235, "width": 1200, "height": 238}]}),
        )
        .await;
    assert!(
        reply.contains("wide photo-band image 0,235 1200×238 cover crop 65%h !crop cuts the image's middle (focus 50%,50%): needs 100px more here (height 338 shows half, 675 all)"),
        "{reply}"
    );
    // A taller band keeps most of it: the edit is clean.
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"id": "photo-band"}, "set": {"height": 420}}]}),
        )
        .await;
    assert!(
        reply.starts_with("changed photo-band v") && !reply.contains("warn"),
        "{reply}"
    );
    mcp.stop().await;
}

#[tokio::test]
async fn a_css_radial_glow_sits_where_css_puts_it() {
    let mcp = Mcp::start("glow").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 400, "height": 400, "sizes": [{"id": "s", "width": 400, "height": 400}]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    // A corner glow, written as agents write it in CSS.
    mcp.ok(
        "layer_add",
        json!({"sceneId": id, "layers": [
        {"type": "rect", "width": "fill", "height": "fill",
         "fill": "radial-gradient(40% 40% at 90% 10%, #ffffff 0%, #000000 100%)"}]}),
    )
    .await;
    let reply = mcp.ok("render", json!({"sceneId": id})).await;
    let (_, path) = common::file_of(reply.lines().next().unwrap()).unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
        .read_info()
        .unwrap();
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let n = info.color_type.samples();
    let at = |x: usize, y: usize| buf[y * info.line_size + x * n];
    assert!(at(360, 40) > 240, "bright at its center: {}", at(360, 40));
    assert!(
        at(200, 200) < 40,
        "dark mid-box, past its radius: {}",
        at(200, 200)
    );
    assert!(at(40, 360) < 10, "dark in the far corner: {}", at(40, 360));
    mcp.stop().await;
}
