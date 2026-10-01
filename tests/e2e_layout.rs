//! End-to-end: an agent builds an adaptive layout through MCP — `fill`,
//! `place`, a direction list, `firstFit`, `at` by aspect class — and
//! `scene_describe` reports every size's boxes and choices without a render.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::golden::check_golden;
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
        json!({"type": "frame",
            "flexDirection": "column",
            "alignItems": "center",
            "children": [{"type": "text", "text": name, "fontSize": 40, "fontWeight": 700}]})
    };
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "layers": [
                {"id": "page", "type": "frame", "width": "fill", "height": "fill", "flexDirection": "column", "gap": 24, "padding": 40, "alignItems": "flex-start", "children": [{"id": "headline", "type": "firstFit", "width": "fill", "children": [{"id": "long", "type": "text", "text": "Results for Willowmere", "fontSize": 64, "fontWeight": 800}, {"id": "short", "type": "text", "text": "Results", "fontSize": 64, "fontWeight": 800}]}, {"id": "photo", "type": "image", "asset": "photo", "width": "fill", "height": "fill", "minHeight": 80}, {"id": "cands", "type": "frame", "width": "fill", "flexDirection": ["row", "column"], "gap": 16, "alignItems": "flex-start", "justifyContent": "space-evenly", "children": [candidate("Dana Levi"), candidate("Omar Haddad"), candidate("Ruth Cohen")]}, {"id": "footer", "type": "text", "text": "Paid for by Willowmere Forward", "fontSize": 24, "media": {"tall": {"hidden": true}}}]},
                {"id": "badge", "type": "ellipse", "width": 120, "height": 120, "fill": "#D0202E", "place": "top-right", "margin": 24}]}))
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
    assert!(
        line(&d, "sky", "headline").ends_with("→ short (long: needs 368×39)"),
        "{d}"
    );
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

    // Nothing needs fixing at any size; the skyscraper's photo, as tall as
    // what's left, is cropped past half: an advisory.
    let problems = mcp.ok("scene_describe", json!({"sceneId": id})).await;
    assert_eq!(
        problems,
        "sky photo image 20,71 260×409 cover crop 64%w warn crop cuts the image's middle (focus 50%,50%): maxHeight 585 shows half\n"
    );
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
    let e = err(json!({"type": "rect", "place": "somewhere"})).await;
    assert!(e.contains("expected one of `top-left`"), "{e}");
    let e = err(json!({"type": "rect", "media": {"tiny": {"width": 5}}})).await;
    assert!(e.contains("no size or aspect class tiny"), "{e}");
    let e = err(json!({"type": "spacer", "minLenght": 5})).await;
    assert!(
        e.contains("minLenght → minLength") || e.contains("allowed: minLength"),
        "{e}"
    );
    mcp.stop().await;
}

#[tokio::test]
async fn presets_name_sizes_and_story_safe_zones_are_checked() {
    let mcp = Mcp::start("presets").await;
    // No master size: the first size is the master.
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["instagram-story", "1200x628"]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok("layer_add", json!({"sceneId": id,
        "layers": [
            {"id": "title", "type": "text", "text": "Under the story bar", "fontSize": 48, "place": "top", "margin": [0, 100]},
            {"id": "cta", "type": "text", "text": "Safe", "fontSize": 48, "place": "center"}]}))
    .await;
    let d = format!(
        "\n{}",
        mcp.ok("scene_describe", json!({"sceneId": id, "full": true}))
            .await
    );
    assert!(d.contains("\ninstagram-story 1080×1920\n"), "{d}");
    assert!(d.contains("\n1200x628 1200×628\n"), "{d}");
    assert!(
        line(&d, "instagram-story", "title").contains("!unsafe"),
        "{d}"
    );
    assert!(
        !line(&d, "instagram-story", "cta").contains("!unsafe"),
        "{d}"
    );
    assert!(!line(&d, "1200x628", "title").contains("!unsafe"), "{d}");
    // The default reply lists only that problem.
    let problems = mcp.ok("scene_describe", json!({"sceneId": id})).await;
    assert!(
        problems.starts_with("instagram-story title text") && problems.contains("!unsafe"),
        "{problems}"
    );
    let e = mcp
        .call("scene_create", json!({"sizes": ["tiktok"]}))
        .await
        .unwrap_err();
    assert!(e.contains("unknown size tiktok"), "{e}");
    mcp.stop().await;
}

#[tokio::test]
async fn a_grid_collage_rearranges_per_aspect_with_one_at() {
    let mcp = Mcp::start("grid").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 1080, "sizes": [
                {"id": "square", "width": 1080, "height": 1080, "scale": 0.5},
                {"id": "banner", "width": 1200, "height": 400, "scale": 0.4}
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
    let tile = |area: &str, color: &str| {
        json!({"id": area,
            "type": "frame",
            "gridArea": area,
            "fill": color,
            "borderRadius": 16,
            "flexDirection": "column",
            "alignItems": "center",
            "justifyContent": "center",
            "children": [
                {"type": "text", "text": area.to_uppercase(), "fontSize": 56, "fontWeight": 800, "color": "#FFFFFF"}]})
    };
    mcp.ok(
        "layer_add",
        json!({"sceneId": id,
            "layers": [
                {"id": "grid", "type": "frame", "width": "fill", "height": "fill", "fill": "#F4F1EA", "gridTemplateColumns": "2fr 1fr", "gridTemplateRows": "2fr 1fr", "gridTemplateAreas": ["photo side", "cta side"], "gap": 24, "padding": 24, "media": {"wide": {"gridTemplateColumns": "2fr 1fr 1fr", "gridTemplateRows": "1fr", "gridTemplateAreas": ["photo cta side"], "gap": 24, "padding": 24}}, "children": [{"id": "photo", "type": "image", "asset": "photo", "gridArea": "photo", "borderRadius": 16}, tile("cta", "#D0202E"), tile("side", "#1B2A5C")]}]}),
    )
    .await;
    let d = format!(
        "\n{}",
        mcp.ok("scene_describe", json!({"sceneId": id, "full": true}))
            .await
    );
    // Square: photo top-left over the CTA, side column on the right.
    // Padding and gap scale to 12: 1080 - 36 = 1044 → 696 + 348 both ways.
    assert_eq!(bbox(line(&d, "square", "photo")), (12, 12, 696, 696));
    assert_eq!(bbox(line(&d, "square", "side")), (720, 12, 348, 1056));
    // Banner (wide): the same children in one row.
    let (_, py, _, ph) = bbox(line(&d, "banner", "photo"));
    let (cx, cy, _, ch) = bbox(line(&d, "banner", "cta"));
    assert_eq!((py, cy, ph, ch), (10, 10, 381, 381), "{d}");
    // 1200 - 2 × 9.6 - 2 × 9.6 = 1161.6 → 580.8 + 290.4 + 290.4.
    assert_eq!(cx, 600, "{d}");
    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for (size, path) in common::files(&rendered) {
        check_golden(&format!("grid-{size}.png"), &std::fs::read(path).unwrap());
    }
    mcp.stop().await;
}
