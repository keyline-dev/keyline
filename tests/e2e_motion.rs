//! End-to-end: motion through MCP — a scene with a duration, split text
//! flying in with `random()` starts, staggered entrances and a looping
//! spin — rendered as an animated PNG and as stills at chosen moments, and
//! left out of the tools with `--no-motion`. Self-drawing strokes and a
//! counting number, checked frame by frame in the animated PNG.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::Mcp;
use common::golden::check_golden;
use serde_json::json;

/// A small "Animate Anything": letters from scattered starts, a subtitle
/// word by word, shapes popping in one after another and spinning.
async fn build(mcp: &Mcp) -> String {
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["500x250"], "duration": 1.5, "fps": 10, "loop": true, "background": "#0E100F"}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "layers": [
                {"id": "title", "type": "text", "text": "Animate Anything", "fontSize": 48, "fontWeight": 900, "color": "#FFFCE1", "place": "top", "margin": [0, 60], "split": "chars", "stagger": 0.03, "animate": {"translate": {"from": ["random(-200, 200)", "random(-120, 120)"]}, "rotate": {"from": "random(-180, 180)"}, "opacity": {"from": 0}, "duration": 0.6, "ease": "back.out"}},
                {"id": "sub", "type": "text", "text": "rendered without a browser", "fontSize": 16, "color": "#9D95FF", "place": "top", "margin": [0, 130], "split": "words", "stagger": 0.1, "enter": {"effect": "fade-up", "delay": 0.6, "duration": 0.3, "distance": 12}},
                {"id": "shapes", "type": "frame", "flexDirection": "row", "gap": 20, "alignItems": "flex-start", "place": "bottom", "margin": 30, "stagger": 0.1, "enter": {"effect": "pop", "delay": 0.8, "duration": 0.3}, "children": [{"type": "rect", "width": 24, "height": 24, "borderRadius": 6, "fill": "#0AE448"}, {"type": "ellipse", "width": 24, "height": 24, "fill": "#FEC5FB"}, {"type": "polygon", "sides": 5, "innerRadius": 0.45, "width": 24, "height": 24, "animate": {"rotate": [0, 360], "duration": 1, "repeat": -1, "ease": "none"}, "fill": "#FF8709"}], "clipsContent": false}]}))
        .await;
    assert!(reply.starts_with("added title,sub,shapes v1 ok"), "{reply}");
    id
}

/// The 4-byte big-endian number at `at`.
fn be(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

#[tokio::test]
async fn a_scene_moves_as_an_animated_png_and_as_stills() {
    let mcp = Mcp::start("motion").await;
    let id = build(&mcp).await;

    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "apng"}))
        .await;
    let path = reply.split_whitespace().nth(1).unwrap();
    assert!(path.ends_with(".anim.png"), "{reply}");
    // The reply says what the file holds: a model sees only its first frame.
    assert!(
        reply.contains(" (500×250, 1.5s, 15 frames at 10 fps, loops, "),
        "{reply}"
    );
    let png = std::fs::read(path).unwrap();
    let actl = png.windows(4).position(|w| w == b"acTL").unwrap();
    assert_eq!(
        (be(&png, actl + 4), be(&png, actl + 8)),
        (15, 0),
        "15 frames, looping"
    );
    // Each frame after the first stores only what changed.
    let frames: Vec<(u32, u32)> = png
        .windows(4)
        .enumerate()
        .filter(|(_, w)| *w == b"fcTL")
        .map(|(i, _)| (be(&png, i + 8), be(&png, i + 12)))
        .collect();
    assert_eq!(frames.len(), 15);
    assert_eq!(frames[0], (500, 250));
    assert!(
        frames[1..].iter().any(|&(w, h)| w * h < 500 * 250),
        "{frames:?}"
    );

    // Stills at a moment, each in its own file.
    for t in [0.3_f32, 1.2] {
        let reply = mcp.ok("render", json!({"sceneId": id, "time": t})).await;
        let path = reply.split_whitespace().nth(1).unwrap();
        assert!(path.ends_with(&format!(".at{t}s.png")), "{reply}");
        assert!(
            reply.contains(" (500×250, ") && reply.contains(" KB)"),
            "{reply}"
        );
        check_golden(&format!("motion-{t}.png"), &std::fs::read(path).unwrap());
    }

    // The same motion as a GIF: plays everywhere, loops like the scene.
    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "gif"}))
        .await;
    let gif = std::fs::read(reply.split_whitespace().nth(1).unwrap()).unwrap();
    assert!(gif.starts_with(b"GIF89a"), "{reply}");
    assert!(reply.contains("15 frames at 10 fps, loops, "), "{reply}");
    assert!(
        gif.windows(11).any(|w| w == b"NETSCAPE2.0"),
        "loops forever"
    );

    let e = mcp
        .call(
            "render",
            json!({"sceneId": id, "format": "apng", "time": 1}),
        )
        .await
        .unwrap_err();
    assert!(e.contains("time renders a still"), "{e}");
    mcp.stop().await;
}

#[tokio::test]
async fn a_still_scene_has_no_animation_to_render() {
    let mcp = Mcp::start("motion-still").await;
    let id = mcp
        .ok("scene_create", json!({"sizes": ["200x100"]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "layer_add",
        json!({"sceneId": id, "layers": [{"type": "rect", "width": 50, "height": 50}]}),
    )
    .await;
    let e = mcp
        .call("render", json!({"sceneId": id, "format": "apng"}))
        .await
        .unwrap_err();
    assert!(e.contains("give it a duration"), "{e}");
    let e = mcp
        .call(
            "layer_add",
            json!({"sceneId": id, "layers": [
            {"type": "rect", "animate": {"fontSize": [10, 20]}}]}),
        )
        .await
        .unwrap_err();
    assert!(e.contains("can't animate fontSize"), "{e}");
    mcp.stop().await;
}

#[tokio::test]
async fn no_motion_leaves_time_out_of_the_tools() {
    let with = Mcp::start("motion-tools").await;
    let without = Mcp::start_args("motion-off", &["--no-motion"]).await;
    let text = |tools: Vec<rmcp::model::Tool>| serde_json::to_string(&tools).unwrap();
    let (on, off) = (text(with.tools().await), text(without.tools().await));
    for word in ["stagger", "\"duration\"", "\"time\""] {
        assert!(on.contains(word), "{word} missing with motion on");
        assert!(!off.contains(word), "{word} still there with --no-motion");
    }
    println!(
        "tools/list: {} chars with motion, {} without",
        on.len(),
        off.len()
    );
    with.stop().await;
    without.stop().await;
}

/// Every frame of an animated PNG as whole RGBA images: each stored frame
/// holds only the box that changed, drawn over the frame before.
fn apng_frames(bytes: &[u8]) -> (usize, Vec<Vec<u8>>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let w = reader.info().width as usize;
    let mut canvas = vec![0u8; w * reader.info().height as usize * 4];
    let mut frames = Vec::new();
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    while let Ok(out) = reader.next_frame(&mut buf) {
        let fc = *reader.info().frame_control().unwrap();
        let (x, y) = (fc.x_offset as usize, fc.y_offset as usize);
        for row in 0..out.height as usize {
            let src = &buf[row * out.line_size..][..out.width as usize * 4];
            canvas[((y + row) * w + x) * 4..][..src.len()].copy_from_slice(src);
        }
        frames.push(canvas.clone());
    }
    (w, frames)
}

/// Dark pixels in an RGBA frame: ink on a white background.
fn ink(frame: &[u8]) -> usize {
    frame.chunks(4).filter(|p| p[0] < 128).count()
}

#[tokio::test]
async fn strokes_draw_themselves_frame_by_frame() {
    let mcp = Mcp::start("motion-draw").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["200x100"], "duration": 1, "fps": 10}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id, "layers": [
            {"id": "rule", "type": "line", "x": 20, "y": 50, "width": 160, "height": 0,
             "stroke": {"width": 8, "color": "#000000"}, "animate": {"drawSVG": [0, 1], "duration": 0.9, "ease": "none"}}]}))
        .await;
    assert!(reply.starts_with("added rule v1 ok"), "{reply}");
    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "apng"}))
        .await;
    let (w, frames) =
        apng_frames(&std::fs::read(reply.split_whitespace().nth(1).unwrap()).unwrap());
    assert_eq!(frames.len(), 10, "{reply}");
    let dark = |f: &[u8], x: usize| f[(50 * w + x) * 4] < 128;
    assert_eq!(ink(&frames[0]), 0, "nothing drawn at the start");
    assert!(
        dark(&frames[5], 60) && !dark(&frames[5], 150),
        "half drawn, left to right"
    );
    assert!(dark(&frames[9], 175), "all drawn at the end");
    let inks: Vec<usize> = frames.iter().map(|f| ink(f)).collect();
    assert!(inks.windows(2).all(|p| p[0] <= p[1]), "{inks:?}");
    mcp.stop().await;
}

#[tokio::test]
async fn a_number_counts_up_frame_by_frame_in_a_box_that_holds_still() {
    let mcp = Mcp::start("motion-count").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["300x100"], "duration": 1, "fps": 10}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok("layer_add", json!({"sceneId": id, "layers": [
        {"id": "stat", "type": "text", "text": "{{n}}+", "fontSize": 40, "color": "#000000", "x": 20, "y": 20,
         "animate": {"count": [0, 1250], "separator": ",", "duration": 0.9, "ease": "none"}}]}))
        .await;
    // Measured with its final value: the box is the width of "1,250+".
    let described = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "apng"}))
        .await;
    let (_, frames) = apng_frames(&std::fs::read(file_path(&reply)).unwrap());
    let inks: Vec<usize> = frames.iter().map(|f| ink(f)).collect();
    assert!(inks[9] > inks[0] * 2, "\"0+\", then \"1,250+\": {inks:?}");
    let again = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    assert_eq!(described, again);
    assert!(described.contains("stat text 20,20 "), "{described}");
    mcp.stop().await;
}

/// The file a render reply names.
fn file_path(reply: &str) -> &str {
    reply.split_whitespace().nth(1).unwrap()
}

#[tokio::test]
async fn a_moving_preview_shows_moments_and_a_held_last_shot_is_stated() {
    let mcp = Mcp::start("motion-preview").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["400x200", "200x200"], "duration": 3, "fps": 10}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "layer_add",
        json!({"sceneId": id, "layers": [
        {"id": "s1", "type": "shot", "duration": 1, "children": [
            {"type": "rect", "width": "fill", "height": "fill", "fill": "#D0202E"}]},
        {"id": "s2", "type": "shot", "duration": 1.5, "transition": "fade", "children": [
            {"type": "rect", "width": "fill", "height": "fill", "fill": "#1B2A5C"}]}]}),
    )
    .await;
    let r = mcp
        .call_raw("render", json!({"sceneId": id, "preview": true}))
        .await;
    let text: String = r
        .content
        .iter()
        .filter_map(|c| c.as_text())
        .map(|t| t.text.clone())
        .collect();
    assert!(text.contains("preview at 0.5 1 1.5 2 2.5 3s"), "{text}");
    let image = r.content.iter().find_map(|c| c.as_image()).unwrap();
    let png =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &image.data).unwrap();
    // Six moments a row, a row per size, 192 px tall, but no wider than
    // 2000 px: the 400×200 row shrinks to 162 px tall to fit.
    assert_eq!(
        keyline_mcp::render::raster_size(&png),
        Some((2000.0, 162.0 + 192.0 + 3.0 * 8.0))
    );
    // The first moment is the red shot, the last the navy one.
    let mut reader = png::Decoder::new(std::io::Cursor::new(&png))
        .read_info()
        .unwrap();
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let px = |x: usize, y: usize| {
        let at = y * info.line_size + x * info.color_type.samples();
        (buf[at], buf[at + 2])
    };
    let (r, b) = px(20, 20);
    assert!(r > 150 && b < 100, "first moment: {r},{b}");
    let (r, b) = px(1980, 20);
    assert!(b > r, "last moment: {r},{b}");

    // The scene outlasts its shots: the last holds, and the reply says so.
    let reply = mcp
        .ok(
            "render",
            json!({"sceneId": id, "format": "gif", "sizes": ["200x200"]}),
        )
        .await;
    // Shots end at 2 s (the fade overlaps them by 0.5 s); the scene lasts 3.
    assert!(reply.contains("last shot held 1s"), "{reply}");
    mcp.stop().await;
}
