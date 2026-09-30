//! End-to-end: templates. A scene file with its photo next to it is loaded
//! by path with one variable set, then rendered once per row of variables.
//! A template can't reach outside the folders the server may read.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use common::Mcp;
use common::golden::check_golden;
use serde_json::json;

/// A folder holding `template.json` and the `photo.png` it names.
fn template_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "keyline-mcp-e2e-{name}-files-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("photo.png"), common::photo_png()).unwrap();
    let template = json!({"sizes": ["400x200"],
        "tokens": {"headline": "Spring sale", "accent": "#000000"},
        "assets": {"photo": "photo.png"},
        "layers": [
            {"type": "image", "asset": "photo", "width": "fill", "height": "fill"},
            {"id": "headline", "type": "text", "text": "{{headline}}", "fontSize": 40, "fontWeight": 800, "color": "{{accent}}", "place": "center"}]});
    std::fs::write(dir.join("template.json"), template.to_string()).unwrap();
    dir
}

#[tokio::test]
async fn a_template_loads_with_its_variables_and_renders_a_file_per_row() {
    let dir = template_dir("template");
    let mcp = Mcp::start_args("template", &["--allow-read", dir.to_str().unwrap()]).await;
    let path = dir.join("template.json");
    let reply = mcp
        .ok(
            "scene_create",
            json!({"path": path.to_str().unwrap(), "tokens": {"headline": "Summer sale"}}),
        )
        .await;
    // What can be set, that it fits, and the facts an edit would give.
    assert!(
        reply.ends_with(" v0 tokens: accent, headline ok\nsmallest text: 400x200 40px (headline)"),
        "{reply}"
    );
    let id = reply.split(' ').next().unwrap().to_owned();

    let reply = mcp
        .ok(
            "render",
            json!({"sceneId": id, "rows": [{"headline": "Fall"}, {"headline": "Winter", "accent": "#D0202E"}]}),
        )
        .await;
    assert!(reply.ends_with("\nfonts: Inter 800"), "{reply}");
    let files: Vec<&str> = reply
        .lines()
        .filter(|l| !l.starts_with(' ') && !l.starts_with("fonts: "))
        .map(|l| {
            let mut parts = l.split(' ');
            let (row, size, file) = (parts.next(), parts.next(), parts.next().unwrap());
            assert_eq!(size, Some("400x200"), "{reply}");
            assert!(file.ends_with(&format!(".{}.png", row.unwrap())), "{reply}");
            file
        })
        .collect();
    assert_eq!(files.len(), 2, "{reply}");
    check_golden("template-r1.png", &std::fs::read(files[0]).unwrap());
    check_golden("template-r2.png", &std::fs::read(files[1]).unwrap());

    let e = mcp
        .call(
            "render",
            json!({"sceneId": id, "rows": [{"headlin": "Typo"}]}),
        )
        .await
        .unwrap_err();
    assert!(
        e.contains("row 1: no token headlin; tokens: accent, headline"),
        "{e}"
    );
    let e = mcp
        .call(
            "scene_create",
            json!({"path": path.to_str().unwrap(), "tokens": {"nope": 1}}),
        )
        .await
        .unwrap_err();
    assert!(
        e.contains("no token nope; the template has: accent, headline"),
        "{e}"
    );
    mcp.stop().await;
}

#[tokio::test]
async fn a_template_cannot_read_outside_the_allowed_folders() {
    let outside = template_dir("template-outside");
    let inside = outside.join("inside");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::write(
        inside.join("template.json"),
        json!({"sizes": ["100x100"], "assets": {"secret": "../photo.png"}}).to_string(),
    )
    .unwrap();
    let mcp = Mcp::start_args(
        "template-outside",
        &["--allow-read", inside.to_str().unwrap()],
    )
    .await;
    let e = mcp
        .call(
            "scene_create",
            json!({"path": inside.join("template.json").to_str().unwrap()}),
        )
        .await
        .unwrap_err();
    assert!(
        e.contains("template asset secret") && e.contains("outside the folders"),
        "{e}"
    );
    mcp.stop().await;

    // Without --allow-read, neither tool offers a path at all.
    let mcp = Mcp::start("template-no-paths").await;
    let tools = serde_json::to_value(mcp.tools().await).unwrap();
    for t in tools.as_array().unwrap() {
        if matches!(t["name"].as_str(), Some("scene_create" | "asset_add")) {
            assert!(t["inputSchema"]["properties"].get("path").is_none(), "{t}");
        }
    }
    mcp.stop().await;
}

/// The RGB pixel at `(x, y)` of a PNG file.
fn pixel(file: &str, x: usize, y: usize) -> [u8; 3] {
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let px = info.color_type.samples();
    let at = y * info.line_size + x * px;
    [buf[at], buf[at + 1], buf[at + 2]]
}

#[tokio::test]
async fn rows_fill_a_sentence_and_swap_the_photo() {
    let mcp = Mcp::start("rows-photo").await;
    let id = mcp
        .ok("scene_create", json!({"sizes": ["400x200"]}))
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    for (asset, bytes) in [
        ("sky", common::photo_png()),
        ("mark", common::CHECK_SVG.as_bytes().to_vec()),
    ] {
        mcp.ok(
            "asset_add",
            json!({"sceneId": id, "id": asset, "base64": common::b64(&bytes)}),
        )
        .await;
    }
    let reply = mcp
        .ok("layer_add", json!({"sceneId": id, "tokens": {"name": "Mia", "photo": "sky", "price": 29},
            "layers": [
                {"id": "photo", "type": "image", "asset": "{{photo}}", "width": 200, "height": 200, "fit": "cover"},
                {"id": "t", "type": "text", "text": "Meet {{name}}", "x": 220, "y": 80, "fontSize": 28},
                {"id": "p", "type": "text", "text": "Only $price", "x": 220, "y": 140, "fontSize": 16}]}))
        .await;
    // A `$name` in text is pointed out, not rewritten.
    assert!(
        reply.contains("hint: did you mean {{price}}? (p says $price)"),
        "{reply}"
    );

    let reply = mcp
        .ok(
            "render",
            json!({"sceneId": id, "rows": [{"name": "Mia"}, {"name": "Rex", "photo": "mark"}]}),
        )
        .await;
    let files: Vec<&str> = reply
        .lines()
        .filter(|l| l.starts_with('r'))
        .map(|l| l.split(' ').nth(2).unwrap())
        .collect();
    assert_eq!(files.len(), 2, "{reply}");
    // Row 1 shows the sky photo (blue at the top left); row 2 the red mark.
    let [r, _, b] = pixel(files[0], 20, 10);
    assert!(b > r, "sky: {:?}", pixel(files[0], 20, 10));
    let [r, g, _] = pixel(files[1], 100, 100);
    assert!(r > 150 && g < 100, "mark: {:?}", pixel(files[1], 100, 100));
    // "Meet Mia" and "Meet Rex" differ where the names are drawn.
    let names = |f: &str| (300..380).map(|x| pixel(f, x, 95)).collect::<Vec<_>>();
    assert_ne!(names(files[0]), names(files[1]), "each row's own name");
    // The preview shows every row, not just the first: a row of sizes each.
    let r = mcp
        .call_raw(
            "render",
            json!({"sceneId": id, "preview": true, "rows": [{"name": "Mia"}, {"name": "Rex", "photo": "mark"}]}),
        )
        .await;
    let image = r.content.iter().find_map(|c| c.as_image()).unwrap();
    let png =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &image.data).unwrap();
    assert_eq!(
        keyline_mcp::render::raster_size(&png),
        Some((400.0 + 16.0, 2.0 * 200.0 + 3.0 * 8.0))
    );
    mcp.stop().await;
}
