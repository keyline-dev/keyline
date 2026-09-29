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
    let template = json!({
        "sizes": ["400x200"],
        "tokens": {"headline": "Spring sale", "accent": "#000000"},
        "assets": {"photo": "photo.png"},
        "layers": [
            {"type": "image", "asset": "photo", "width": "fill", "height": "fill"},
            {"id": "headline", "type": "text", "text": "$headline", "fontSize": 40, "weight": 800,
             "color": "$accent", "place": "center"}
        ]
    });
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
    assert!(
        reply.ends_with(" v0 tokens: accent, headline ok"),
        "names what can be set, and that it fits: {reply}"
    );
    let id = reply.split(' ').next().unwrap().to_owned();

    let reply = mcp
        .ok(
            "render",
            json!({"sceneId": id, "rows": [{"headline": "Fall"}, {"headline": "Winter", "accent": "#D0202E"}]}),
        )
        .await;
    let files: Vec<&str> = reply
        .lines()
        .filter(|l| !l.starts_with(' '))
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
