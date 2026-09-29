//! End-to-end: the spec's reference ad written with reuse — tokens,
//! styles, components placed with `each`, an adaptive layout — in one
//! `layer_add`, with a token changed afterwards. Checks the layout and the
//! renders, and compares the tool traffic with the same ad written out
//! layer by layer.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::golden::check_golden;
use common::{CHECK_SVG, Mcp, ad_sizes, b64, build_reference_ad, photo_png};
use serde_json::json;

/// The reference ad with reuse: the same content as `build_reference_ad`.
async fn build_reused_ad(mcp: &Mcp) -> String {
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 1350, "sizes": ad_sizes()}),
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
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "check", "base64": b64(CHECK_SVG.as_bytes())}),
    )
    .await;
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "tokens": {"navy": "#1B2A5C", "red": "#D0202E", "grey": "#6B7280"},
            "styles": {"accent": {"color": "$red"}, "name": {"fontSize": 36, "fontWeight": 700, "color": "$navy", "textAlign": "center"}, "office": {"fontSize": 30, "fontWeight": 500, "color": "$grey", "textAlign": "center"}},
            "components": {"candidate": {"type": "frame", "flexDirection": "column", "gap": 4, "alignItems": "center", "children": [{"type": "text", "role": "name", "text": "{{name}}", "style": "name"}, {"type": "text", "role": "office", "text": "{{office}}", "style": "office"}]}, "step": {"type": "frame", "width": "fill", "flexDirection": "row", "gap": 16, "alignItems": "center", "children": [{"type": "image", "asset": "check", "width": 40, "height": 40}, {"type": "text", "role": "step", "text": "{{text}}", "fontSize": 32, "fontWeight": 500, "color": "$navy", "width": "fill"}]}},
            "layers": [
                {"id": "page", "type": "frame", "width": "fill", "height": "fill", "flexDirection": "column", "gap": 28, "padding": [48, 0], "alignItems": "flex-start", "children": [{"id": "headline", "type": "text", "width": "fill", "padding": [0, 40], "fontSize": 64, "fontWeight": 800, "color": "$navy", "textAlign": "center", "textWrap": "balance", "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"}, {"id": "photo", "type": "image", "asset": "photo", "width": "fill", "height": "fill", "minHeight": 120}, {"id": "cands", "type": "frame", "width": "fill", "flexDirection": ["row", "column"], "gap": 12, "alignItems": "flex-start", "justifyContent": "space-evenly", "children": [{"id": "c", "type": "use", "component": "candidate", "each": [{"name": "Dana Levi", "office": "Mayor"}, {"name": "Omar Haddad", "office": "Council"}, {"name": "Ruth Cohen", "office": "Council"}]}]}, {"id": "cta", "type": "frame", "width": "fill", "fill": "$red", "flexDirection": "row", "gap": 16, "padding": 22, "alignItems": "center", "justifyContent": "center", "children": [{"type": "icon", "name": "mail", "color": "#FFFFFF", "width": 48, "height": 48}, {"type": "text", "text": "VOTE BY MAIL", "fontSize": 48, "fontWeight": 800, "color": "#FFFFFF"}]}, {"id": "steps", "type": "frame", "width": "fill", "flexDirection": "column", "gap": 12, "padding": [0, 60], "alignItems": "flex-start", "children": [{"id": "s", "type": "use", "component": "step", "each": [{"text": "Request your ballot by October 20"}, {"text": "Fill it out at home"}, {"text": "Mail it back by November 3"}]}]}, {"id": "footer", "type": "text", "width": "fill", "text": "Paid for by Willowmere Forward · willowmereforward.org", "fontSize": 20, "color": "$grey", "textAlign": "center", "media": {"tall": {"hidden": true}}}]}]}))
        .await;
    assert!(
        reply.lines().next().unwrap().starts_with("added page"),
        "{reply}"
    );
    id
}

/// The line for layer `id` at size `size` in a full `scene_describe`.
fn line<'a>(d: &'a str, size: &str, id: &str) -> &'a str {
    d.split(&format!("\n{size} ")).nth(1).map_or("", |s| {
        s.lines()
            .find(|l| l.trim_start().starts_with(&format!("{id} ")))
            .unwrap_or("")
    })
}

#[tokio::test]
async fn the_reference_ad_with_components_tokens_and_adaptive_layout() {
    let mcp = Mcp::start("reuse").await;
    let id = build_reused_ad(&mcp).await;
    let reused_traffic = mcp.traffic.get();
    let d = format!(
        "\n{}",
        mcp.ok("scene_describe", json!({"sceneId": id, "full": true}))
            .await
    );
    // Instances are addressed by the use's id, then number, then role.
    for size in ["portrait", "wide", "sky"] {
        assert!(!line(&d, size, "c.2.name").is_empty(), "{size}: {d}");
        assert!(!line(&d, size, "s.1.step").is_empty(), "{size}: {d}");
    }
    assert!(line(&d, "portrait", "cands").ends_with("→ row"), "{d}");
    // The skyscraper renders at scale 0.28, where three names still fit a row.
    assert!(line(&d, "sky", "cands").ends_with("→ row"), "{d}");
    assert!(
        line(&d, "sky", "footer").is_empty(),
        "the tall size drops the footer: {d}"
    );

    // One token edit recolors the headline accent, the CTA and the names.
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "tokens": {"red": "#B0101C"}, "ops": []}),
        )
        .await;
    assert!(
        reply.starts_with("changed $red v") && reply.contains("ok"),
        "{reply}"
    );
    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for l in rendered.lines().filter(|l| !l.starts_with(' ')) {
        let (size, path) = l.split_once(' ').unwrap();
        check_golden(&format!("reuse-{size}.png"), &std::fs::read(path).unwrap());
    }
    mcp.stop().await;

    // The same ad written out layer by layer, for scale: reuse should cost no more.
    let plain = Mcp::start("reuse-plain").await;
    build_reference_ad(&plain).await;
    let plain_traffic = plain.traffic.get();
    plain.stop().await;
    println!(
        "tool traffic: with reuse {reused_traffic} chars, layer by layer {plain_traffic} chars"
    );
    assert!(
        reused_traffic <= plain_traffic * 11 / 10,
        "with reuse {reused_traffic} vs layer by layer {plain_traffic}"
    );
}

#[tokio::test]
async fn components_are_edited_once_for_every_instance() {
    let mcp = Mcp::start("reuse-edit").await;
    let id = build_reused_ad(&mcp).await;
    // Bigger names everywhere, through the component's style.
    mcp.ok(
        "layer_update",
        json!({"sceneId": id, "ops": [{"target": {"style": "name"}, "set": {"fontSize": 44}}]}),
    )
    .await;
    let d = mcp
        .ok(
            "scene_describe",
            json!({"sceneId": id, "size": "portrait", "full": true}),
        )
        .await;
    assert!(
        d.lines()
            .filter(|l| l.contains(".name text") && l.contains(" 44px"))
            .count()
            == 3,
        "{d}"
    );
    // An instance's parts can't be edited alone: change the component, or detach.
    let e = mcp.call("layer_update", json!({"sceneId": id, "ops": [{"target": {"id": "c.1.name"}, "set": {"fontSize": 50}}]})).await.unwrap_err();
    assert!(e.contains("no layer with id c.1.name"), "{e}");
    mcp.ok(
        "layer_update",
        json!({"sceneId": id, "ops": [{"target": {"id": "c"}, "detach": true}]}),
    )
    .await;
    mcp.ok(
        "layer_update",
        json!({"sceneId": id, "ops": [{"target": {"id": "c.1.name"}, "set": {"fontSize": 50}}]}),
    )
    .await;
    mcp.stop().await;
}

#[tokio::test]
async fn flat_gradients_and_tokens_in_spans_work_as_agents_write_them() {
    let mcp = Mcp::start("reuse-guesses").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 400, "height": 200, "sizes": ["400x200"]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    // The first call of every benchmark run in one batch, which used to fail whole.
    let reply = mcp
        .ok(
            "layer_add",
            json!({"sceneId": id,
                "tokens": {"red": "#D0202E"},
                "layers": [
                    {"type": "rect", "width": 400, "height": 80, "fill": [{"type": "linear", "angle": 180, "stops": [{"color": "#FFFFFF", "offset": 0}, {"color": "#FFFFFF00", "offset": 1}]}]},
                    {"id": "t", "type": "text", "text": "Proven <span style=\"color:$red\">RESULTS</span>"}]}),
        )
        .await;
    assert!(reply.starts_with("added rect1,t v1 ok"), "{reply}");
    let e = mcp
        .call(
            "layer_add",
            json!({"sceneId": id,
                "layers": [{"type": "text", "text": "<span style=\"color:$blue\">x</span>"}]}),
        )
        .await
        .unwrap_err();
    assert!(e.contains("<span style=\"color:$blue\">: "), "{e}");
    mcp.stop().await;
}
