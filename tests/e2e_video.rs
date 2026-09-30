//! End-to-end: video through ffmpeg. Three one-second clips, each a shot
//! with its own animated title, joined by a push and a wipe under a caption
//! bar that stays: checked as stills at moments against goldens, and
//! encoded as MP4 (with the clips' sound, or without) and WebM. Without
//! ffmpeg, video is refused and APNG still works.
//!
//! The demo with livelier clips: `cargo test --test e2e_video -- --ignored`.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::Mcp;
use common::golden::check_golden;
use serde_json::{Value, json};

/// Whether ffmpeg is here; CI must have it.
fn have_ffmpeg() -> bool {
    let found = Command::new("ffmpeg").arg("-version").output().is_ok();
    assert!(
        found || std::env::var_os("CI").is_none(),
        "CI needs ffmpeg installed"
    );
    if !found {
        eprintln!("skipped: no ffmpeg");
    }
    found
}

/// Makes `name.mkv` in `dir` from an ffmpeg `lavfi` video source, with a
/// tone, losslessly so decoded frames don't depend on encoder versions.
fn clip(dir: &Path, name: &str, source: &str, tone: u32) -> PathBuf {
    let out = dir.join(format!("{name}.mkv"));
    let ok = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", source])
        .args(["-f", "lavfi", "-i", &format!("sine=f={tone}:d=1")])
        .args([
            "-t", "1", "-c:v", "ffv1", "-pix_fmt", "bgr0", "-c:a", "flac",
        ])
        .arg(&out)
        .status()
        .unwrap()
        .success();
    assert!(ok, "ffmpeg made {name}");
    out
}

/// The demo scene over MCP, its three clips made from `sources`.
async fn build(mcp: &Mcp, dir: &Path, sources: [&str; 3]) -> String {
    let path = format!(
        "{}/tests/fixtures/video-demo.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let id = mcp
        .ok("scene_create", fixture["create"].clone())
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    for ((name, source), tone) in ["one", "two", "three"]
        .into_iter()
        .zip(sources)
        .zip([440, 550, 660])
    {
        let file = clip(dir, name, source, tone);
        let reply = mcp
            .ok(
                "asset_add",
                json!({"sceneId": id, "id": name, "path": file.to_str().unwrap()}),
            )
            .await;
        assert!(
            reply.starts_with(&format!("{name} 1280×720 1s 30fps sound")),
            "{reply}"
        );
    }
    let mut add = fixture["add"].clone();
    add["sceneId"] = Value::String(id.clone());
    let reply = mcp.ok("layer_add", add).await;
    assert!(reply.lines().next().unwrap().ends_with(" ok"), "{reply}");
    id
}

/// The streams in a video file, by type: `["video", "audio"]`.
fn streams(file: &str) -> Vec<String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
            file,
        ])
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "keyline-mcp-e2e-{name}-clips-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn three_clips_play_as_shots_with_titles_and_sound() {
    if !have_ffmpeg() {
        return;
    }
    let dir = scratch("video");
    let mcp = Mcp::start_args("video", &["--allow-read", dir.to_str().unwrap()]).await;
    let colors = ["#3A86FF", "#FF006E", "#FB5607"].map(|c| format!("color=c={c}:s=1280x720:r=30"));
    let id = build(
        &mcp,
        &dir,
        [&colors[0], &colors[1], &colors[2]].map(String::as_str),
    )
    .await;

    // Clips are listed and measured like images.
    let full = mcp
        .ok("scene_describe", json!({"sceneId": id, "full": true}))
        .await;
    assert!(full.contains("one 1280×720 1s sound"), "{full}");
    assert!(full.contains("video1 video 0,0 1280×720 cover"), "{full}");

    // The first title flying in, the push, the wipe, the last title.
    for t in [0.3_f32, 0.8, 1.5, 2.2] {
        let reply = mcp.ok("render", json!({"sceneId": id, "time": t})).await;
        let path = reply.split_whitespace().nth(1).unwrap();
        check_golden(&format!("video-{t}.png"), &std::fs::read(path).unwrap());
    }

    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "mp4"}))
        .await;
    let mp4 = reply.split_whitespace().nth(1).unwrap();
    assert!(mp4.ends_with(".mp4"), "{reply}");
    // A video's facts: length and frames; players decide the looping.
    assert!(
        reply.contains(" frames at ") && !reply.contains("loops"),
        "{reply}"
    );
    assert_eq!(
        streams(mp4),
        ["video", "audio"],
        "the clips' sound comes along"
    );

    let reply = mcp
        .ok(
            "render",
            json!({"sceneId": id, "format": "webm", "muted": true}),
        )
        .await;
    let webm = reply.split_whitespace().nth(1).unwrap();
    assert!(webm.ends_with(".webm"), "{reply}");
    assert_eq!(streams(webm), ["video"], "muted leaves sound out");
    mcp.stop().await;
}

#[tokio::test]
async fn without_ffmpeg_video_is_refused_and_apng_still_works() {
    let mcp = Mcp::start_args("no-ffmpeg", &["--ffmpeg", "/no/such/ffmpeg"]).await;
    let created = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["100x50"], "duration": 0.2, "fps": 10}),
        )
        .await;
    assert!(
        created.contains("\nvideo off: no ffmpeg"),
        "said up front: {created}"
    );
    let id = created.split(' ').next().unwrap().to_owned();
    mcp.ok(
        "layer_add",
        json!({"sceneId": id,
            "layers": [{"type": "rect", "width": 20, "height": 20, "animate": {"rotate": [0, 90]}}]}),
    )
    .await;
    let e = mcp
        .call("render", json!({"sceneId": id, "format": "mp4"}))
        .await
        .unwrap_err();
    assert!(e.contains("video needs ffmpeg"), "{e}");
    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "apng"}))
        .await;
    assert!(reply.contains(".anim.png"), "{reply}");
    mcp.stop().await;
}

#[tokio::test]
async fn video_off_goes_unsaid_when_motion_is_off() {
    let args = ["--no-motion", "--ffmpeg", "/no/such/ffmpeg"];
    let mcp = Mcp::start_args("no-ffmpeg-no-motion", &args).await;
    let created = mcp.ok("scene_create", json!({"sizes": ["100x50"]})).await;
    assert!(!created.contains("video"), "{created}");
    mcp.stop().await;
}

#[tokio::test]
async fn local_paths_are_offered_only_with_folders_and_name_them() {
    let asset_add = |tools: Vec<rmcp::model::Tool>| {
        let t = tools.into_iter().find(|t| t.name == "asset_add").unwrap();
        (
            t.description.unwrap_or_default().to_string(),
            Value::Object((*t.input_schema).clone()),
        )
    };
    let mcp = Mcp::start("paths-off").await;
    let (doc, schema) = asset_add(mcp.tools().await);
    assert!(schema["properties"].get("path").is_none(), "{schema}");
    assert!(doc.contains("from url or base64"), "{doc}");
    mcp.stop().await;

    let dir = std::fs::canonicalize(scratch("paths-on")).unwrap();
    let mcp = Mcp::start_args("paths-on", &["--allow-read", dir.to_str().unwrap()]).await;
    let (_, schema) = asset_add(mcp.tools().await);
    let path = schema["properties"]["path"]["description"]
        .as_str()
        .unwrap();
    assert_eq!(path, format!("Or a local file in {}", dir.display()));
    mcp.stop().await;
}

/// The demo, with livelier clips: writes `keyline-video-demo.mp4` and
/// `.gif` to the temp directory.
#[tokio::test]
#[ignore = "writes the demo; run on demand"]
async fn video_demo() {
    assert!(have_ffmpeg(), "the demo needs ffmpeg");
    let dir = scratch("video-demo");
    let mcp = Mcp::start_args("video-demo", &["--allow-read", dir.to_str().unwrap()]).await;
    let id = build(
        &mcp,
        &dir,
        [
            "mandelbrot=s=1280x720:r=30",
            "life=s=320x180:r=30:seed=7:mold=10:life_color=#FF006E:death_color=#3A0CA3,scale=1280:720:flags=neighbor",
            "gradients=s=1280x720:r=30:seed=3:speed=0.05",
        ],
    )
    .await;
    for format in ["mp4", "gif"] {
        let reply = mcp
            .ok("render", json!({"sceneId": id, "format": format}))
            .await;
        let path = reply.split_whitespace().nth(1).unwrap();
        let out = std::env::temp_dir().join(format!("keyline-video-demo.{format}"));
        std::fs::copy(path, &out).unwrap();
        println!("{}", out.display());
    }
    mcp.stop().await;
}
