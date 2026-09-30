//! End-to-end: the README's showcase. One "Cold Brew Season" scene becomes a
//! square post, a wide banner and a tall story; the same design, animated,
//! becomes the README's animated PNG. The stills and two moments of the
//! animation are checked against goldens; `UPDATE_GOLDEN=1` also rewrites
//! the README's animation in `docs/media/`.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::Mcp;
use common::golden::check_golden;
use serde_json::Value;

/// A scene from `tests/fixtures/<name>.json` (`{create, add}`), built over MCP.
async fn build(mcp: &Mcp, name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let id = mcp
        .ok("scene_create", fixture["create"].clone())
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let mut add = fixture["add"].clone();
    add["sceneId"] = Value::String(id.clone());
    let reply = mcp.ok("layer_add", add).await;
    assert!(
        reply.lines().next().unwrap().ends_with(" ok"),
        "no problems at any size: {reply}"
    );
    id
}

#[tokio::test]
async fn one_scene_becomes_a_post_a_banner_and_a_story() {
    let mcp = Mcp::start("showcase").await;
    let id = build(&mcp, "showcase").await;
    let rendered = mcp.ok("render", serde_json::json!({"sceneId": id})).await;
    let mut sizes = Vec::new();
    for l in rendered.lines().filter(|l| !l.starts_with(' ')) {
        let (size, path) = common::file_of(l).unwrap();
        check_golden(
            &format!("showcase-{size}.png"),
            &std::fs::read(path).unwrap(),
        );
        sizes.push(size.to_owned());
    }
    assert_eq!(sizes, ["square", "banner", "story"]);
    mcp.stop().await;
}

#[tokio::test]
async fn the_same_design_moves() {
    let mcp = Mcp::start("showcase-motion").await;
    let id = build(&mcp, "showcase-motion").await;
    for t in [0.5_f32, 2.4] {
        let reply = mcp
            .ok("render", serde_json::json!({"sceneId": id, "time": t}))
            .await;
        let path = reply.split_whitespace().nth(1).unwrap();
        check_golden(
            &format!("showcase-motion-{t}.png"),
            &std::fs::read(path).unwrap(),
        );
    }
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let reply = mcp
            .ok(
                "render",
                serde_json::json!({"sceneId": id, "format": "apng"}),
            )
            .await;
        let path = reply.split_whitespace().nth(1).unwrap();
        let media = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/media");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::copy(path, media.join("showcase-motion.png")).unwrap();
    }
    mcp.stop().await;
}
