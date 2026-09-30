//! End-to-end: motion through MCP — a scene with a duration, split text
//! flying in with `random()` starts, staggered entrances and a looping
//! spin — rendered as an animated PNG and as stills at chosen moments, and
//! left out of the tools with `--no-motion`.

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
