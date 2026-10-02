//! Deterministic end-to-end tests: the real server binary over stdio, driven
//! through MCP the way an agent would. Renders are compared against golden
//! PNGs, allowing for glyph anti-aliasing that differs between OS versions;
//! set `UPDATE_GOLDEN=1` to rewrite them deliberately.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::golden::{PIXEL_TOLERANCE, check_golden, golden, mismatch, png_of, rgba};
use common::{Mcp, b64, build_reference_ad};
use serde_json::json;

/// Tool definitions are sent to the model on every turn, so they have a
/// budget (~2.5k tokens; motion, video and shots take ~1k of it, and
/// `--no-motion` gives that back). Raise it only on purpose.
const TOOLS_LIST_MAX_CHARS: usize = 10_000;

#[test]
fn golden_check_tolerates_glyph_edges_but_not_changed_content() {
    let name = "reference-portrait.png";
    let (size, mut px) = rgba(&std::fs::read(golden(name)).unwrap());
    let width = usize::try_from(size.0).unwrap();
    // Scattered small shifts, like another OS version's anti-aliasing.
    for p in px.as_chunks_mut::<4>().0.iter_mut().step_by(97) {
        p[0] = p[0].saturating_add(PIXEL_TOLERANCE);
    }
    let want = std::fs::read(golden(name)).unwrap();
    assert_eq!(mismatch(&want, &png_of(size, &px)), None);
    // A 100×100 block of new content is a real change.
    for y in 500..600 {
        for x in 400..500 {
            let i = (y * width + x) * 4;
            px[i..i + 4].copy_from_slice(&[255, 0, 255, 255]);
        }
    }
    let why = mismatch(&want, &png_of(size, &px)).unwrap();
    assert!(why.starts_with("10000 of 1458000 pixels past"), "{why}");
}

#[tokio::test]
async fn reference_ad_renders_all_sizes_without_defects() {
    let mcp = Mcp::start("reference").await;
    let id = build_reference_ad(&mcp).await;

    // No defects; the photo band's crops are described in the layout.
    let described = mcp.ok("scene_describe", json!({"sceneId": id})).await;
    assert_eq!(described, "ok");
    let described = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    for size in ["portrait 1080×1350", "wide 1200×1000", "sky 300×600"] {
        assert!(described.contains(size), "{described}");
    }

    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    // Indented lines report how wrapped or shrunk text was actually drawn.
    assert!(
        rendered.contains(" headline 64px: \"Proven RESULTS for\" / \"WILLOWMERE Families\""),
        "{rendered}"
    );
    let paths: Vec<(&str, &str)> = common::files(&rendered).collect();
    assert_eq!(paths.len(), 3, "{rendered}");
    for (size, path) in &paths {
        check_golden(
            &format!("reference-{size}.png"),
            &std::fs::read(path).unwrap(),
        );
    }

    // Same scene JSON, byte-identical PNGs.
    let first: Vec<Vec<u8>> = paths
        .iter()
        .map(|(_, p)| std::fs::read(p).unwrap())
        .collect();
    mcp.ok("render", json!({"sceneId": id})).await;
    let second: Vec<Vec<u8>> = paths
        .iter()
        .map(|(_, p)| std::fs::read(p).unwrap())
        .collect();
    assert!(first == second, "renders are not deterministic");
    mcp.stop().await;
}

#[tokio::test]
async fn preview_is_one_small_sheet_of_all_sizes() {
    let mcp = Mcp::start("preview").await;
    let id = build_reference_ad(&mcp).await;
    let r = mcp
        .call_raw("render", json!({"sceneId": id, "preview": true}))
        .await;
    let images: Vec<_> = r.content.iter().filter_map(|c| c.as_image()).collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime_type, "image/png");
    let png = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &images[0].data)
        .unwrap();
    // Portrait 307 + wide 461 + sky 192 px wide at 384 px tall, with 8 px gaps.
    assert_eq!(keyline_mcp::render::raster_size(&png), Some((992.0, 400.0)));
    mcp.stop().await;
}

#[tokio::test]
async fn edits_are_batched_atomic_and_terse() {
    let mcp = Mcp::start("edits").await;
    let id = build_reference_ad(&mcp).await;

    // One call moves every name by role; the reply is ids and a version only.
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "ops": [
                {"target": {"role": "name"}, "set": {"fontSize": 40}},
                {"target": {"id": "footer"}, "delete": true}
            ]}),
        )
        .await;
    assert!(
        reply.starts_with("changed text1,text3,text5,footer v"),
        "{reply}"
    );
    // Line 1: ids and version, then the advisories, then facts for the
    // agent to judge.
    let (status, rest) = reply.split_once('\n').expect("status and facts");
    assert!(status.len() < 60, "{reply}");
    assert_eq!(
        rest,
        "smallest text: portrait 30px (text2), wide 26px (text2), sky 8.4px (text2)"
    );

    // A bad op rejects the whole batch.
    let err = mcp
        .call(
            "layer_update",
            json!({"sceneId": id, "ops": [
                {"target": {"id": "headline"}, "set": {"y": 0}},
                {"target": {"id": "nope"}, "set": {"y": 0}}
            ]}),
        )
        .await
        .unwrap_err();
    assert!(err.contains("ops[1]") && err.contains("nope"), "{err}");
    let d = mcp
        .ok(
            "scene_describe",
            json!({"sceneId": id, "size": "portrait", "full": true}),
        )
        .await;
    assert!(d.contains("headline text 60,40"), "{d}");
    assert!(!d.contains("footer"), "{d}");

    // Overflow is measured server-side and reported in the edit's own reply,
    // for every size, so no scene_describe round trip is needed.
    let d = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id,
                "ops": [{"target": {"id": "cta-text"}, "set": {"width": 120, "height": 50}}]}),
        )
        .await;
    // The warning says what size would fit, so one retry is enough.
    assert!(d.contains("!truncated needs 120×"), "{d}");
    for size in ["portrait", "wide", "sky"] {
        assert!(
            d.lines()
                .any(|l| l.starts_with(&format!("{size} cta-text")) && l.contains("!truncated")),
            "{d}"
        );
    }
    mcp.stop().await;
}

#[tokio::test]
async fn bad_input_gets_one_line_errors() {
    let mcp = Mcp::start("errors").await;
    let id = mcp
        .ok("scene_create", json!({"width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let cases = [
        (
            "asset_add",
            json!({"sceneId": id, "url": "file:///etc/passwd"}),
            "http",
        ),
        (
            "asset_add",
            json!({"sceneId": id, "url": "http://127.0.0.1/x.png"}),
            "not a public address",
        ),
        (
            "asset_add",
            json!({"sceneId": id, "base64": b64(b"hello")}),
            "not a PNG",
        ),
        (
            "layer_add",
            json!({"sceneId": id, "layers": [{"type": "text", "text": "x", "fontsize": 3}]}),
            "fontsize",
        ),
        (
            "layer_add",
            json!({"sceneId": id, "layers": [{"type": "image", "asset": "missing"}]}),
            "unknown asset",
        ),
        ("scene_describe", json!({"sceneId": "../x"}), "bad id"),
        (
            "render",
            json!({"sceneId": id, "sizes": ["nope"]}),
            "no size nope",
        ),
        (
            "render",
            json!({"sceneId": id, "size": ["a"]}),
            "unknown field `size`",
        ),
    ];
    for (tool, args, want) in cases {
        let err = mcp.call(tool, args).await.unwrap_err();
        assert!(err.contains(want), "{tool}: {err}");
        assert!(!err.contains('\n') && err.len() < 200, "{tool}: {err}");
    }
    mcp.stop().await;
}

#[tokio::test]
async fn tool_surface_stays_small() {
    let mcp = Mcp::start("tools").await;
    assert_eq!(mcp.server_name(), "keyline-mcp");
    let tools = mcp.tools().await;
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names.len(), 6, "{names:?}");
    let size = serde_json::to_string(&tools).unwrap().len();
    println!("tools/list: {size} chars (~{} tokens)", size / 4);
    assert!(size < TOOLS_LIST_MAX_CHARS, "tools/list is {size} chars");

    // The whole reference ad costs about 2k tokens of tool traffic.
    build_reference_ad(&mcp).await;
    let traffic = mcp.traffic.get();
    println!(
        "reference ad: {traffic} chars of tool traffic (~{} tokens, base64 excluded)",
        traffic / 4
    );
    assert!(
        traffic / 4 < 2000,
        "reference ad took ~{} tokens of tool traffic",
        traffic / 4
    );
    mcp.stop().await;
}
