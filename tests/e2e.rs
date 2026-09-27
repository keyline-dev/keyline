//! Deterministic end-to-end tests: the real server binary over stdio, driven
//! through MCP the way an agent would. Renders are compared against golden
//! PNGs, allowing for glyph anti-aliasing that differs between OS versions;
//! set `UPDATE_GOLDEN=1` to rewrite them deliberately.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use common::{CHECK_SVG, LEOPARD_SVG, Mcp, b64, build_reference_ad, flag_svg, photo_png};
use serde_json::json;

/// Tool definitions are sent to the model on every turn, so they have a
/// budget (~1.75k tokens). Raise it only on purpose, for features worth it.
const TOOLS_LIST_MAX_CHARS: usize = 7000;

// ponytail: Skia rasterizes glyphs through the OS font stack, so goldens are
// per OS and compared with a tolerance across its versions; embed FreeType
// if renders must match byte-for-byte everywhere.
fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(std::env::consts::OS)
        .join(name)
}

fn check_golden(name: &str, png: &[u8]) {
    let path = golden(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, png).unwrap();
        return;
    }
    let want = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}; run with UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    let Some(why) = mismatch(&want, png) else {
        return;
    };
    // Keep the actual render so CI can upload it next to the golden.
    let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("golden-actual")
        .join(name);
    std::fs::create_dir_all(actual.parent().unwrap()).unwrap();
    std::fs::write(&actual, png).unwrap();
    panic!(
        "{name} differs from {}: {why}; actual at {}",
        path.display(),
        actual.display()
    );
}

/// Why `got` doesn't match the golden `want`, or `None` when it does: the
/// same size, with at most [`MAX_DIFF_SHARE`] of pixels past [`PIXEL_TOLERANCE`].
fn mismatch(want: &[u8], got: &[u8]) -> Option<String> {
    if want == got {
        return None;
    }
    let (size, want_px) = rgba(want);
    let (got_size, got_px) = rgba(got);
    if size != got_size {
        return Some(format!("size {got_size:?}, want {size:?}"));
    }
    let off: Vec<u8> = want_px
        .as_chunks::<4>()
        .0
        .iter()
        .zip(got_px.as_chunks::<4>().0)
        .map(|(a, b)| {
            a.iter()
                .zip(b)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let past = |t: u8| off.iter().filter(|&&d| d > t).count();
    let over = past(PIXEL_TOLERANCE);
    #[expect(
        clippy::cast_precision_loss,
        reason = "pixel counts are far below 2^52"
    )]
    let share = over as f64 / off.len() as f64;
    (share > MAX_DIFF_SHARE).then(|| {
        let histogram = [8, 32, 64, 128]
            .map(|t| format!(">{t}: {}", past(t)))
            .join(", ");
        format!(
            "{over} of {} pixels past {PIXEL_TOLERANCE} ({:.3}%, max {:.3}%; {histogram})",
            off.len(),
            share * 100.0,
            MAX_DIFF_SHARE * 100.0
        )
    })
}

/// Channel difference up to which a pixel still matches the golden. The OS
/// rasterizes glyphs, and macOS versions anti-alias their edges differently.
const PIXEL_TOLERANCE: u8 = 48;
/// Share of pixels allowed past [`PIXEL_TOLERANCE`]. A moved, missing or
/// recolored element changes far more than glyph edges do.
const MAX_DIFF_SHARE: f64 = 0.002;

/// Encodes unpremultiplied RGBA pixels as a PNG.
fn png_of((w, h): (i32, i32), px: &[u8]) -> Vec<u8> {
    use skia_safe::{AlphaType, ColorType, Data, EncodedImageFormat, ImageInfo, images};
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let image = images::raster_from_data(&info, Data::new_copy(px), info.min_row_bytes()).unwrap();
    image
        .encode(None, EncodedImageFormat::PNG, None)
        .unwrap()
        .as_bytes()
        .to_vec()
}

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

/// A PNG's size and unpremultiplied RGBA pixels.
fn rgba(png: &[u8]) -> ((i32, i32), Vec<u8>) {
    use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};
    let image = Image::from_encoded(Data::new_copy(png)).expect("decodable PNG");
    let info = ImageInfo::new(
        image.dimensions(),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut px = vec![0; info.compute_min_byte_size()];
    assert!(image.read_pixels(
        &info,
        &mut px,
        info.min_row_bytes(),
        (0, 0),
        CachingHint::Disallow
    ));
    ((image.width(), image.height()), px)
}

#[tokio::test]
async fn reference_ad_renders_all_sizes_without_warnings() {
    let mcp = Mcp::start("reference").await;
    let id = build_reference_ad(&mcp).await;

    // A clean layout costs two characters.
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
    let paths: Vec<(&str, &str)> = rendered
        .lines()
        .filter(|l| !l.starts_with(' '))
        .map(|l| l.split_once(' ').expect("size path"))
        .collect();
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
    // Line 1: ids, version, ok. Line 2: facts for the agent to judge.
    let (status, facts) = reply.split_once('\n').expect("status and facts");
    assert!(status.ends_with(" ok") && status.len() < 60, "{reply}");
    assert_eq!(
        facts,
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
            json!({"sceneId": id, "ops": [
                {"target": {"id": "cta-text"}, "set": {"resize": "fixed", "width": 120, "height": 50}}
            ]}),
        )
        .await;
    // The warning says what size would fit, so one retry is enough.
    assert!(d.contains("!overflow needs 120×"), "{d}");
    for size in ["portrait", "wide", "sky"] {
        assert!(
            d.lines()
                .any(|l| l.starts_with(&format!("{size} cta-text")) && l.contains("!overflow")),
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
    assert!(err.contains("no size tiny"), "{err}");
    mcp.stop().await;
}
