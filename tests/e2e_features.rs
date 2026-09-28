//! End-to-end tests of core features through MCP: web fonts, text
//! fills and outlines, stacks, styles, icons, shapes, masks and per-size
//! changes, with golden renders.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::golden::check_golden;
use common::{CHECK_SVG, LEOPARD_SVG, Mcp, b64, build_reference_ad, flag_svg, photo_png};
use serde_json::json;

/// Web fonts work like CSS: naming a Google Fonts family downloads it once,
/// and a restarted server finds it in its cache instead of downloading again.
#[tokio::test]
#[ignore = "downloads from Google Fonts; run with --ignored"]
async fn web_fonts_download_once_and_survive_a_restart() {
    let mcp = Mcp::start("webfonts").await;
    let id = mcp
        .ok("scene_create", json!({"width": 400, "height": 200, "sizes": [{"id": "a", "width": 400, "height": 200}]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let layer = json!({"sceneId": id, "layers": [{"type": "text", "text": "SALE", "fontFamily": "Anton", "fontSize": 60}]});
    let first = mcp.ok("layer_add", layer.clone()).await;
    assert!(first.starts_with("fetched font Anton ("), "{first}");
    let again = mcp.ok("layer_add", layer.clone()).await;
    assert!(
        !again.contains("fetched"),
        "second use downloaded again: {again}"
    );

    let data = mcp.shut_down().await;
    let index = std::fs::read_to_string(data.join("fonts/index.json")).unwrap();
    assert!(index.contains("\"family\": \"Anton\""), "{index}");

    let mcp = Mcp::start_in(data).await;
    let after_restart = mcp.ok("layer_add", layer).await;
    assert!(
        !after_restart.contains("fetched"),
        "restart lost the cache: {after_restart}"
    );
    let err = mcp
        .call("layer_add", json!({"sceneId": id, "layers": [{"type": "text", "text": "x", "fontFamily": "Not A Real Font Family"}]}))
        .await
        .unwrap_err();
    assert!(err.contains("isn't on Google Fonts"), "{err}");
    mcp.stop().await;
}

/// Two servers on one data dir (two agent sessions, or a benchmark that
/// renders the agent's scene itself): a font one fetched after the other
/// started is used by both, not silently replaced by Inter.
#[tokio::test]
#[ignore = "downloads from Google Fonts; run with --ignored"]
async fn a_font_fetched_by_another_server_is_used_not_replaced() {
    let early = Mcp::start("shared-fonts").await;
    let late_data = early.data.clone();
    let id = early
        .ok("scene_create", json!({"width": 600, "height": 200, "sizes": [{"id": "a", "width": 600, "height": 200}]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    // `early` started before the font existed; `other` fetches it.
    let other = Mcp::start_in(late_data).await;
    let fetched = other
        .ok("layer_add", json!({"sceneId": id, "layers": [
            {"id": "t", "type": "text", "text": "GLIDDEN", "fontFamily": "Montserrat", "weight": 900, "fontSize": 60}]}))
        .await;
    assert!(fetched.starts_with("fetched font Montserrat"), "{fetched}");
    let width = |d: &str| {
        d.lines()
            .find(|l| l.trim_start().starts_with("t text"))
            .and_then(|l| l.split_whitespace().nth(3))
            .map(str::to_owned)
            .unwrap_or_default()
    };
    let theirs = width(
        &other
            .ok("scene_describe", json!({"sceneId": id, "full": true}))
            .await,
    );
    let ours = width(
        &early
            .ok("scene_describe", json!({"sceneId": id, "full": true}))
            .await,
    );
    assert_eq!(ours, theirs, "the early server measured another font");
    early.ok("render", json!({"sceneId": id})).await;
    other.stop().await;
    early.stop().await;
}

/// Text painted with images: a word in a flag, a word in a repeating pattern,
/// outlined and tilted, over a tiled background.
#[tokio::test]
async fn text_can_be_filled_with_images_and_outlined() {
    let mcp = Mcp::start("textfill").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 1080, "background": "#111111", "sizes": [
                {"id": "square", "width": 1080, "height": 1080},
                {"id": "story", "width": 1080, "height": 1920}
            ]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "flag", "base64": b64(flag_svg().as_bytes())}),
    )
    .await;
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "leopard", "base64": b64(LEOPARD_SVG.as_bytes())}),
    )
    .await;
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id, "layers": [
            {"id": "pattern", "type": "image", "asset": "leopard", "fit": "tile", "tileScale": 1.5,
             "width": 1080, "height": 1080, "opacity": 0.35,
             "constraints": {"h": "stretch", "v": "stretch"}},
            {"id": "usa", "type": "text", "text": "USA", "x": 90, "y": 170, "width": 900, "height": 330,
             "fontSize": 320, "weight": 900, "align": "center", "rotation": -8,
             "fill": {"asset": "flag"}, "outline": {"width": 8, "color": "#FFFFFF"},
             "constraints": {"h": "center", "v": "center"}},
            {"id": "leopard", "type": "text", "text": "LEOPARD", "x": 60, "y": 620, "width": 960, "height": 200,
             "fontSize": 190, "weight": 900, "align": "center", "rotation": 6,
             "fill": {"asset": "leopard", "fit": "tile", "tileScale": 0.8},
             "outline": {"width": 5, "color": "#1A1A1A"},
             "constraints": {"h": "center", "v": "center"}}
        ]}))
        .await;
    assert!(reply.lines().next().unwrap().ends_with(" ok"), "{reply}");

    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for line in rendered.lines().filter(|l| !l.starts_with(' ')) {
        let (size, path) = line.split_once(' ').unwrap();
        check_golden(
            &format!("textfill-{size}.png"),
            &std::fs::read(path).unwrap(),
        );
    }
    mcp.stop().await;
}

#[tokio::test]
async fn stacks_lay_out_rows_and_columns_at_every_size() {
    let mcp = Mcp::start("stacks").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 600, "sizes": [
                {"id": "banner", "width": 1080, "height": 600},
                {"id": "wide", "width": 1600, "height": 600}
            ]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "check", "base64": b64(CHECK_SVG.as_bytes())}),
    )
    .await;
    // Three centered columns, spread evenly across a full-width row: no x or
    // y below the row, and it keeps its spacing when the row stretches.
    let column = |label: &str, name: &str| {
        json!({"type": "frame", "stack": {"dir": "column", "gap": 8, "align": "center"}, "children": [
            {"type": "image", "asset": "check", "width": 48, "height": 48},
            {"type": "text", "text": label, "fontSize": 20, "weight": 700, "color": "#1B2A5C"},
            {"type": "text", "text": name, "fontSize": 48, "weight": 800, "color": "#D0202E"}
        ]})
    };
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id, "layers": [
            {"id": "row", "type": "frame", "y": 180, "width": 1080, "height": 240,
             "constraints": {"h": "stretch"},
             "stack": {"dir": "row", "justify": "evenly", "align": "center"},
             "children": [column("MAYOR", "GLIDDEN"), column("COUNCIL", "CHO"), column("COUNCIL", "JETHANI")]}
        ]}))
        .await;
    assert!(reply.lines().next().unwrap().ends_with(" ok"), "{reply}");

    let d = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    // Asset sizes come first, so images can be placed without guessing.
    assert!(d.starts_with("assets check 40×40\n"), "{d}");
    assert!(d.contains("row frame 0,180 1080×240"), "{d}");
    assert!(d.contains("row frame 0,180 1600×240"), "{d}");

    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for line in rendered.lines().filter(|l| !l.starts_with(' ')) {
        let (size, path) = line.split_once(' ').unwrap();
        check_golden(&format!("stacks-{size}.png"), &std::fs::read(path).unwrap());
    }
    mcp.stop().await;
}

#[tokio::test]
async fn styles_icons_shapes_masks_and_focus_work_end_to_end() {
    let mcp = Mcp::start("kit").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 960, "height": 540, "sizes": [
                {"id": "card", "width": 960, "height": 540},
                {"id": "tall", "width": 960, "height": 800}
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
    // One step: a red circle with a white icon, a title and a note.
    let step = |icon: &str, set: &str, title: &str| {
        json!({"type": "frame", "stack": {"dir": "column", "gap": 10, "align": "center"}, "children": [
            {"type": "frame", "width": 64, "height": 64, "children": [
                {"type": "ellipse", "width": 64, "height": 64, "color": "#D0202E"},
                {"type": "icon", "name": icon, "set": set, "color": "#FFFFFF", "x": 18, "y": 18, "width": 28, "height": 28}
            ]},
            {"type": "text", "text": title, "style": "step"},
            {"type": "text", "text": "by Nov. 3", "style": "note"}
        ]})
    };
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "styles": {
                "step": {"fontSize": 30, "weight": 800, "color": "#1B2A5C"},
                "note": {"fontSize": 18, "color": "#4B5563"}
            },
            "layers": [
                {"id": "photo", "type": "image", "asset": "photo", "width": 960, "height": 260, "focus": [0.5, 0.2],
                 "constraints": {"h": "stretch", "v": "stretch"},
                 "mask": {"from": [0.5, 0], "to": [0.5, 0.4], "stops": [{"at": 0, "color": "#00000000"}, {"at": 1, "color": "#000000"}]}},
                {"id": "rule", "type": "line", "x": 40, "y": 290, "width": 880, "color": "#CFD3DA", "strokeWidth": 2,
                 "constraints": {"h": "stretch", "v": "bottom"}},
                {"id": "steps", "type": "frame", "y": 320, "width": 960, "height": 190,
                 "constraints": {"h": "stretch", "v": "bottom"},
                 "stack": {"dir": "row", "justify": "evenly"},
                 "children": [step("mail", "lucide", "MAIL IT"), step("location-dot", "solid", "DROP IT"), step("landmark", "lucide", "HAND IT IN")]}
            ]}))
        .await;
    assert!(reply.lines().next().unwrap().ends_with(" ok"), "{reply}");

    // Restyling is one edit to the style, not one per layer.
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"style": "step"}, "set": {"color": "#D0202E"}}]}),
        )
        .await;
    assert!(reply.starts_with("changed step v"), "{reply}");
    let err = mcp
        .call(
            "layer_add",
            json!({"sceneId": id, "layers": [{"type": "icon", "name": "envelope"}]}),
        )
        .await
        .unwrap_err();
    assert!(err.contains("no lucide icon envelope"), "{err}");

    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for line in rendered.lines().filter(|l| !l.starts_with(' ')) {
        let (size, path) = line.split_once(' ').unwrap();
        check_golden(&format!("kit-{size}.png"), &std::fs::read(path).unwrap());
    }
    mcp.stop().await;
}

#[tokio::test]
async fn at_adapts_one_size_without_touching_the_others() {
    let mcp = Mcp::start("at").await;
    let id = build_reference_ad(&mcp).await;
    let before = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    // The skyscraper's footer is tiny at scale 0.28; make it bigger there only.
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"id": "footer"}, "set": {"at": {"sky": {"fontSize": 40}}}}]}),
        )
        .await;
    assert!(reply.starts_with("changed footer v"), "{reply}");
    let after = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    let footer = |d: &str, size: &str| {
        d.split(&format!("{size} "))
            .nth(1)
            .and_then(|s| s.lines().find(|l| l.trim_start().starts_with("footer ")))
            .unwrap()
            .to_owned()
    };
    assert_eq!(footer(&before, "portrait"), footer(&after, "portrait"));
    assert_ne!(footer(&before, "sky"), footer(&after, "sky"));
    assert!(footer(&after, "sky").contains(" 11px"), "{after}");
    let err = mcp
        .call(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"id": "footer"}, "set": {"at": {"tiny": {"fontSize": 9}}}}]}),
        )
        .await
        .unwrap_err();
    assert!(err.contains("no size or aspect class tiny"), "{err}");
    mcp.stop().await;
}

#[tokio::test]
async fn renders_jpeg_webp_and_pdf_and_fit_a_file_size_cap() {
    let mcp = Mcp::start("formats").await;
    let id = build_reference_ad(&mcp).await;
    let paths = |reply: &str| -> Vec<(String, String)> {
        reply
            .lines()
            .filter(|l| !l.starts_with(' '))
            .map(|l| {
                let mut w = l.splitn(3, ' ');
                let size = w.next().unwrap().to_owned();
                let path = w.next().unwrap().to_owned();
                (size, format!("{path} {}", w.next().unwrap_or("")))
            })
            .collect()
    };
    let file = |p: &str| std::fs::read(p.split(' ').next().unwrap()).unwrap();

    let jpeg = mcp
        .ok(
            "render",
            json!({"sceneId": id, "format": "jpeg", "sizes": ["portrait"]}),
        )
        .await;
    let (_, p) = &paths(&jpeg)[0];
    assert!(p.contains(".jpg"), "{jpeg}");
    let full = file(p);
    assert!(full.starts_with(&[0xFF, 0xD8]));

    // A cap below the quality-90 size lowers the quality and says so.
    let cap = full.len() / 1024 * 2 / 3;
    let capped = mcp
        .ok(
            "render",
            json!({"sceneId": id, "format": "jpeg", "maxKB": cap, "sizes": ["portrait"]}),
        )
        .await;
    let (_, p) = &paths(&capped)[0];
    assert!(p.contains(" quality "), "{capped}");
    assert!(file(p).len() <= cap * 1024);

    let webp = mcp
        .ok(
            "render",
            json!({"sceneId": id, "format": "webp", "sizes": ["portrait"]}),
        )
        .await;
    assert_eq!(&file(&paths(&webp)[0].1)[8..12], b"WEBP");

    let pdf = mcp
        .ok("render", json!({"sceneId": id, "format": "pdf"}))
        .await;
    for (size, p) in paths(&pdf) {
        let bytes = file(&p);
        assert!(bytes.starts_with(b"%PDF"), "{size}");
        // Vector: the text is set in fonts (Type3 outlines for variable
        // fonts), not painted as pixels.
        assert!(bytes.windows(11).any(|w| w == b"/Type /Font"), "{size}");
    }

    let e = mcp
        .call("render", json!({"sceneId": id, "format": "gif"}))
        .await
        .unwrap_err();
    assert!(e.contains("unknown variant `gif`"), "{e}");
    mcp.stop().await;
}
