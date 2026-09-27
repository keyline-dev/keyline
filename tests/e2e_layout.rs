//! End-to-end: an agent builds an adaptive layout through MCP — `fill`,
//! `place`, a direction list, `firstFit`, `at` by aspect class — and
//! `scene_describe` reports every size's boxes and choices without a render.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Mcp, b64, photo_png};
use serde_json::json;

/// The line for layer `id` at size `size` in a full `scene_describe`.
fn line<'a>(d: &'a str, size: &str, id: &str) -> &'a str {
    d.split(&format!("\n{size} ")).nth(1).map_or("", |s| {
        s.lines()
            .find(|l| l.trim_start().starts_with(&format!("{id} ")))
            .unwrap_or("")
    })
}

/// `x,y w×h` from a describe line.
fn bbox(line: &str) -> (i64, i64, i64, i64) {
    let mut words = line.split_whitespace().skip(2);
    let (x, y) = words.next().unwrap().split_once(',').unwrap();
    let (w, h) = words.next().unwrap().split_once('×').unwrap();
    let n = |s: &str| s.parse::<i64>().unwrap();
    (n(x), n(y), n(w), n(h))
}

#[tokio::test]
async fn one_master_adapts_to_every_size_without_at() {
    let mcp = Mcp::start("adaptive").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 1350, "sizes": [
                {"id": "portrait", "width": 1080, "height": 1350},
                {"id": "banner", "width": 1200, "height": 628, "scale": 0.6},
                {"id": "sky", "width": 300, "height": 600, "scale": 0.5}
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
    let candidate = |name: &str| {
        json!({"type": "frame", "stack": {"dir": "column", "align": "center"}, "children": [
            {"type": "text", "text": name, "fontSize": 40, "weight": 700}]})
    };
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id, "layers": [
            {"id": "page", "type": "frame", "width": "fill", "height": "fill",
             "stack": {"dir": "column", "gap": 24, "padding": 40}, "children": [
                {"id": "headline", "type": "firstFit", "width": "fill", "children": [
                    {"id": "long", "type": "text", "text": "Results for Willowmere", "fontSize": 64, "weight": 800},
                    {"id": "short", "type": "text", "text": "Results", "fontSize": 64, "weight": 800}]},
                {"id": "photo", "type": "image", "asset": "photo", "width": "fill", "height": "fill", "minHeight": 80},
                {"id": "cands", "type": "frame", "width": "fill", "stack": {"dir": ["row", "column"], "justify": "evenly", "gap": 16},
                 "children": [candidate("Dana Levi"), candidate("Omar Haddad"), candidate("Ruth Cohen")]},
                {"id": "footer", "type": "text", "text": "Paid for by Willowmere Forward", "fontSize": 24,
                 "at": {"tall": {"hidden": true}}}]},
            {"id": "badge", "type": "ellipse", "width": 120, "height": 120, "color": "#D0202E",
             "place": "top-right", "inset": 24}
        ]}))
        .await;
    assert!(
        reply
            .lines()
            .next()
            .unwrap()
            .starts_with("added page,badge"),
        "{reply}"
    );

    let d = format!(
        "\n{}",
        mcp.ok("scene_describe", json!({"sceneId": id, "full": true}))
            .await
    );
    // The long headline fits the portrait width; the skyscraper gets the short one.
    assert!(line(&d, "portrait", "headline").ends_with("→ long"), "{d}");
    assert!(line(&d, "sky", "headline").ends_with("→ short"), "{d}");
    // Candidates sit in a row where three fit, a column where they don't.
    assert!(line(&d, "portrait", "cands").ends_with("→ row"), "{d}");
    assert!(line(&d, "sky", "cands").ends_with("→ column"), "{d}");
    // The photo takes the height that's left at each size.
    let (_, _, pw, ph) = bbox(line(&d, "portrait", "photo"));
    let (_, _, bw, bh) = bbox(line(&d, "banner", "photo"));
    assert_eq!(pw, 1000);
    assert!(ph > 600 && bh < ph && bw > pw, "{d}");
    // The badge stays in the top-right corner, inset by 24 × scale.
    assert_eq!(bbox(line(&d, "portrait", "badge")), (936, 24, 120, 120));
    assert_eq!(bbox(line(&d, "sky", "badge")), (228, 12, 60, 60));
    // `at` by aspect class: the skyscraper drops the footer.
    assert!(!line(&d, "portrait", "footer").is_empty());
    assert!(line(&d, "sky", "footer").is_empty(), "{d}");

    // Nothing needs fixing at any size.
    let problems = mcp.ok("scene_describe", json!({"sceneId": id})).await;
    assert_eq!(problems, "ok");
    mcp.stop().await;
}

#[tokio::test]
async fn bad_layout_values_get_one_line_errors() {
    let mcp = Mcp::start("layout-errors").await;
    let id = mcp
        .ok("scene_create", json!({"width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let err = |layer: serde_json::Value| {
        let mcp = &mcp;
        let id = id.clone();
        async move {
            mcp.call("layer_add", json!({"sceneId": id, "layers": [layer]}))
                .await
                .unwrap_err()
        }
    };
    let e = err(json!({"type": "rect", "width": "wide"})).await;
    assert!(e.contains("\"hug\", \"fill\" or \"40%\""), "{e}");
    assert_eq!(e.lines().count(), 1, "{e}");
    let e = err(json!({"type": "rect", "place": "middle"})).await;
    assert!(e.contains("expected one of `top-left`"), "{e}");
    let e = err(json!({"type": "rect", "at": {"tiny": {"width": 5}}})).await;
    assert!(e.contains("no size or aspect class tiny"), "{e}");
    let e = err(json!({"type": "spacer", "minLenght": 5})).await;
    assert!(
        e.contains("minLenght → minLength") || e.contains("allowed: minLength"),
        "{e}"
    );
    mcp.stop().await;
}
