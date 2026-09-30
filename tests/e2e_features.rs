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
        .ok("layer_add", json!({"sceneId": id,
            "layers": [
                {"id": "t", "type": "text", "text": "GLIDDEN", "fontFamily": "Montserrat", "fontWeight": 900, "fontSize": 60}]}))
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
        .ok("layer_add", json!({"sceneId": id,
            "layers": [
                {"id": "pattern", "type": "image", "asset": "leopard", "fit": "tile", "tileScale": 1.5, "width": 1080, "height": 1080, "opacity": 0.35, "constraints": {"horizontal": "stretch", "vertical": "stretch"}},
                {"id": "usa", "type": "text", "text": "USA", "x": 90, "y": 170, "width": 900, "height": 330, "fontSize": 320, "fontWeight": 900, "textAlign": "center", "rotate": -8, "constraints": {"horizontal": "center", "vertical": "center"}, "fill": {"image": "flag"}, "stroke": {"width": 8, "color": "#FFFFFF", "align": "center"}},
                {"id": "leopard", "type": "text", "text": "LEOPARD", "x": 60, "y": 620, "width": 960, "height": 200, "fontSize": 190, "fontWeight": 900, "textAlign": "center", "rotate": 6, "constraints": {"horizontal": "center", "vertical": "center"}, "fill": {"fit": "tile", "tileScale": 0.8, "image": "leopard"}, "stroke": {"width": 5, "color": "#1A1A1A", "align": "center"}}]}))
        .await;
    assert!(reply.lines().next().unwrap().ends_with(" ok"), "{reply}");

    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    for line in rendered.lines().filter(|l| !l.starts_with(' ')) {
        let (size, path) = common::file_of(line).unwrap();
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
        json!({"type": "frame",
            "flexDirection": "column",
            "gap": 8,
            "alignItems": "center",
            "children": [
                {"type": "image", "asset": "check", "width": 48, "height": 48},
                {"type": "text", "text": label, "fontSize": 20, "fontWeight": 700, "color": "#1B2A5C"},
                {"type": "text", "text": name, "fontSize": 48, "fontWeight": 800, "color": "#D0202E"}]})
    };
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "layers": [
                {"id": "row", "type": "frame", "y": 180, "width": 1080, "height": 240, "constraints": {"horizontal": "stretch"}, "flexDirection": "row", "alignItems": "center", "justifyContent": "space-evenly", "children": [column("MAYOR", "GLIDDEN"), column("COUNCIL", "CHO"), column("COUNCIL", "JETHANI")]}]}))
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
        let (size, path) = common::file_of(line).unwrap();
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
        json!({"type": "frame",
            "flexDirection": "column",
            "gap": 10,
            "alignItems": "center",
            "children": [
                {"type": "frame", "width": 64, "height": 64, "children": [{"type": "ellipse", "width": 64, "height": 64, "fill": "#D0202E"}, {"type": "icon", "name": icon, "set": set, "color": "#FFFFFF", "x": 18, "y": 18, "width": 28, "height": 28}]},
                {"type": "text", "text": title, "style": "step"},
                {"type": "text", "text": "by Nov. 3", "style": "note"}]})
    };
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id,
            "styles": {"step": {"fontSize": 30, "fontWeight": 800, "color": "#1B2A5C"}, "note": {"fontSize": 18, "color": "#4B5563"}},
            "layers": [
                {"id": "photo", "type": "image", "asset": "photo", "width": 960, "height": 260, "focus": [0.5, 0.2], "constraints": {"horizontal": "stretch", "vertical": "stretch"}, "mask": {"from": [0.5, 0], "to": [0.5, 0.4], "stops": [{"offset": 0, "color": "#00000000"}, {"offset": 1, "color": "#000000"}]}},
                {"id": "rule", "type": "line", "x": 40, "y": 290, "width": 880, "constraints": {"horizontal": "stretch", "vertical": "bottom"}, "stroke": {"width": 2, "color": "#CFD3DA"}},
                {"id": "steps", "type": "frame", "y": 320, "width": 960, "height": 190, "constraints": {"horizontal": "stretch", "vertical": "bottom"}, "flexDirection": "row", "alignItems": "flex-start", "justifyContent": "space-evenly", "children": [step("mail", "lucide", "MAIL IT"), step("location-dot", "solid", "DROP IT"), step("landmark", "lucide", "HAND IT IN")]}]}))
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
        let (size, path) = common::file_of(line).unwrap();
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
            json!({"sceneId": id, "ops": [{"target": {"id": "footer"}, "set": {"media": {"sky": {"fontSize": 40}}}}]}),
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
            json!({"sceneId": id, "ops": [{"target": {"id": "footer"}, "set": {"media": {"tiny": {"fontSize": 9}}}}]}),
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
        .call("render", json!({"sceneId": id, "format": "bmp"}))
        .await
        .unwrap_err();
    assert!(e.contains("unknown variant `bmp`"), "{e}");
    // Animated formats need a scene that moves.
    let e = mcp
        .call("render", json!({"sceneId": id, "format": "gif"}))
        .await
        .unwrap_err();
    assert!(e.contains("gif needs an animated scene"), "{e}");
    mcp.stop().await;
}

/// A new 400×300 scene's id.
async fn new_scene(mcp: &Mcp) -> String {
    mcp.ok("scene_create", json!({"sizes": ["400x300"]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned()
}

/// The error `asset_add` gives for a local path.
async fn path_refused(mcp: &Mcp, id: &str, path: &std::path::Path) -> String {
    mcp.call(
        "asset_add",
        json!({"sceneId": id, "path": path.display().to_string()}),
    )
    .await
    .unwrap_err()
}

/// Local paths: read only inside a folder the server was started with,
/// and never through a symlink that leads outside it.
#[tokio::test]
async fn assets_come_from_local_paths_only_where_allowed() {
    let base = std::env::temp_dir().join(format!("keyline-mcp-e2e-paths-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (allowed, outside) = (base.join("allowed"), base.join("outside"));
    std::fs::create_dir_all(&allowed).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(allowed.join("photo.png"), photo_png()).unwrap();
    std::fs::write(outside.join("photo.png"), photo_png()).unwrap();

    // Off unless a folder is allowed.
    let off = Mcp::start("paths-off").await;
    let id = new_scene(&off).await;
    let e = path_refused(&off, &id, &allowed.join("photo.png")).await;
    assert!(e.contains("start the server with --allow-read"), "{e}");
    off.stop().await;

    let dir = allowed.display().to_string();
    let on = Mcp::start_args("paths-on", &["--allow-read", &dir]).await;
    let id = new_scene(&on).await;
    let reply = on
        .ok(
            "asset_add",
            json!({"sceneId": id, "id": "photo", "path": allowed.join("photo.png").display().to_string()}),
        )
        .await;
    assert!(reply.starts_with("photo "), "{reply}");
    for p in [
        outside.join("photo.png"),
        allowed.join("../outside/photo.png"),
    ] {
        let e = path_refused(&on, &id, &p).await;
        assert!(e.contains("outside the folders"), "{p:?}: {e}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, allowed.join("door")).unwrap();
        let e = path_refused(&on, &id, &allowed.join("door/photo.png")).await;
        assert!(e.contains("outside the folders"), "{e}");
    }
    on.stop().await;
}

#[test]
fn the_command_line_explains_itself() {
    let run = |arg: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_keyline-mcp"))
            .arg(arg)
            .output()
            .unwrap()
    };
    let help = run("--help");
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(help.status.success(), "{text}");
    for word in [
        "--allow-read",
        "--data",
        "--renderer",
        "--ffmpeg",
        "--encoder",
    ] {
        assert!(text.contains(word), "{word} missing from --help");
    }
    let bad = run("--bogus");
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("see keyline-mcp --help"));
}

#[tokio::test]
async fn the_scene_itself_is_edited_like_a_layer() {
    let mcp = Mcp::start("scene-set").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 400, "height": 400, "sizes": ["400x400"]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"scene": true},
                "set": {"background": "#000", "sizes": ["200x100"]}}]}),
        )
        .await;
    assert_eq!(reply, "changed scene v1 ok");
    // The new size renders, on the new background.
    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    let (size, path) = common::file_of(rendered.lines().next().unwrap()).unwrap();
    assert_eq!(size, "200x100", "{rendered}");
    let (dims, px) = common::golden::rgba(&std::fs::read(path).unwrap());
    assert_eq!(dims, (200, 100));
    assert_eq!(px[..4], [0, 0, 0, 255]);
    let e = mcp
        .call(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"scene": true}, "set": {"fill": "#FFF"}}]}),
        )
        .await
        .unwrap_err();
    assert!(e.contains("the scene takes background, sizes"), "{e}");
    mcp.stop().await;
}

#[tokio::test]
async fn an_inline_svg_comes_as_a_data_url() {
    let mcp = Mcp::start("data-url").await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"width": 100, "height": 100, "sizes": ["100x100"]}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    // As agents write it: unencoded text, `#` as %23.
    let svg = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='24' \
height='24'><rect width='24' height='24' fill='%23D0202E'/></svg>";
    let reply = mcp
        .ok("asset_add", json!({"sceneId": id, "id": "dot", "url": svg}))
        .await;
    assert!(reply.starts_with("dot 24×24"), "{reply}");
    mcp.ok(
        "layer_add",
        json!({"sceneId": id, "layers": [{"type": "image", "asset": "dot", "width": 100, "height": 100}]}),
    )
    .await;
    let rendered = mcp.ok("render", json!({"sceneId": id})).await;
    let path = common::file_of(rendered.lines().next().unwrap()).unwrap().1;
    let (_, px) = common::golden::rgba(&std::fs::read(path).unwrap());
    assert_eq!(px[..4], [0xD0, 0x20, 0x2E, 255]);
    mcp.stop().await;
}

#[test]
fn render_draws_a_scene_file_and_fails_on_a_defect() {
    let dir = std::env::temp_dir().join(format!("keyline-mcp-e2e-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let scene = dir.join("ad.json");
    std::fs::write(
        &scene,
        json!({"width": 400, "height": 200, "sizes": ["400x200", {"id": "small", "width": 200, "height": 100}],
            "tokens": {"headline": "Sale"},
            "layers": [{"id": "h", "type": "text", "text": "{{headline}}", "place": "center", "fontSize": 40}]})
        .to_string(),
    )
    .unwrap();
    let rows = dir.join("rows.json");
    std::fs::write(
        &rows,
        r#"[{"headline": "Hi"}, {"headline": "A headline far too long for any of these sizes"}]"#,
    )
    .unwrap();
    let run = |extra: &[&str]| {
        let out = dir.join("out");
        let _ = std::fs::remove_dir_all(&out);
        let o = std::process::Command::new(env!("CARGO_BIN_EXE_keyline-mcp"))
            .arg("render")
            .arg(&scene)
            .args(["--renderer", "cpu", "--data"])
            .arg(dir.join("data"))
            .arg("--out")
            .arg(&out)
            .args(extra)
            .output()
            .unwrap();
        let mut files: Vec<String> = std::fs::read_dir(&out)
            .map(|d| {
                d.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            files,
        )
    };
    // Fits: every size, exit 0.
    let (code, text, files) = run(&[]);
    assert_eq!(code, Some(0), "{text}");
    assert_eq!(files, ["400x200-v0.png", "small-v0.png"]);
    // One row per variant; the long one is a defect, so a script fails.
    let (code, text, files) = run(&["--rows", rows.to_str().unwrap(), "--size", "400x200"]);
    assert_eq!(code, Some(1), "{text}");
    assert_eq!(files, ["400x200-v0.r1.png", "400x200-v0.r2.png"]);
    assert!(
        text.contains("r2 400x200 h text") && text.contains("!clipped"),
        "{text}"
    );
    assert!(!text.contains("small"), "only the sizes drawn: {text}");
    // Render flags without render are a mistake.
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_keyline-mcp"))
        .args(["--out", "x"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&o.stderr).contains("go with render"));
}

#[test]
fn without_data_the_home_folder_holds_it_on_every_os() {
    // Windows has no HOME; its home folder is USERPROFILE.
    let home = std::env::temp_dir().join(format!("keyline-mcp-e2e-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let scene = home.join("ad.json");
    std::fs::write(
        &scene,
        json!({"width": 100, "height": 100, "sizes": ["100x100"], "layers": [{"type": "rect", "fill": "#000"}]})
            .to_string(),
    )
    .unwrap();
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_keyline-mcp"))
        .arg("render")
        .arg(&scene)
        .args(["--renderer", "cpu", "--out"])
        .arg(home.join("out"))
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(home.join(".keyline-mcp").join("scenes").is_dir());
}

#[test]
#[ignore = "needs the network (Google Fonts)"]
fn web_fonts_a_scene_file_names_are_fetched_before_it_is_checked() {
    let dir = std::env::temp_dir().join(format!("keyline-mcp-e2e-tplfont-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let scene = dir.join("ad.json");
    std::fs::write(
        &scene,
        json!({"width": 200, "height": 100, "sizes": ["200x100"],
            "layers": [{"type": "text", "text": "Hi", "fontFamily": "Poppins", "fontSize": 30}]})
        .to_string(),
    )
    .unwrap();
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_keyline-mcp"))
        .arg("render")
        .arg(&scene)
        .args(["--renderer", "cpu", "--data"])
        .arg(dir.join("data"))
        .arg("--out")
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(dir.join("out").join("200x100-v0.png").is_file());
}
