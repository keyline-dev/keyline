//! End-to-end: video through ffmpeg. Three one-second clips, each a shot
//! with its own animated title, joined by a push and a wipe under a caption
//! bar that stays: checked as stills at moments against goldens, and
//! encoded as MP4 (with the clips' sound, or without) and WebM. Without
//! ffmpeg, video is refused and APNG still works.
//!
//! A scene soundtrack (a WAV) plays under a video, cut to its length.
//! A clip cut short fails the render with where it stopped, rather than
//! freezing on its last good frame.
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
        reply.contains(" frames at ") && !reply.contains("loops") && reply.contains(", with sound"),
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
    assert!(!reply.contains("with sound"), "{reply}");
    mcp.stop().await;
}

/// Seconds of the audio stream in a file.
fn audio_seconds(file: &str) -> f32 {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=duration",
        ])
        .args(["-of", "csv=p=0", file])
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn a_soundtrack_plays_under_the_video_cut_to_its_length() {
    if !have_ffmpeg() {
        return;
    }
    let dir = scratch("soundtrack");
    // Five seconds of tone: longer than the two-second video.
    let song = dir.join("song.wav");
    let made = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=f=330:d=5"])
        .arg(&song)
        .status()
        .unwrap()
        .success();
    assert!(made, "ffmpeg made the song");
    let mcp = Mcp::start_args("soundtrack", &["--allow-read", dir.to_str().unwrap()]).await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": ["320x180"], "duration": 2, "fps": 10}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    let reply = mcp
        .ok(
            "asset_add",
            json!({"sceneId": id, "id": "song", "path": song.to_str().unwrap()}),
        )
        .await;
    assert!(reply.starts_with("song sound 5s v"), "{reply}");
    mcp.ok("layer_add", json!({"sceneId": id, "layers": [{"type": "rect", "width": 100, "height": 100, "fill": "#3A86FF"}]}))
        .await;
    let reply = mcp
        .ok(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"scene": true},
            "set": {"audio": {"asset": "song", "volume": 0.8, "trimStart": 1, "fadeOut": 0.5}}}]}),
        )
        .await;
    assert!(reply.contains(" ok"), "{reply}");

    let reply = mcp
        .ok("render", json!({"sceneId": id, "format": "mp4"}))
        .await;
    assert!(
        reply.contains("2s, 20 frames at 10 fps, with sound"),
        "{reply}"
    );
    let mp4 = reply.split_whitespace().nth(1).unwrap();
    assert_eq!(streams(mp4), ["video", "audio"]);
    let secs = audio_seconds(mp4);
    assert!((secs - 2.0).abs() < 0.1, "cut to the video: {secs}s");

    let reply = mcp
        .ok(
            "render",
            json!({"sceneId": id, "format": "webm", "muted": true}),
        )
        .await;
    assert_eq!(streams(reply.split_whitespace().nth(1).unwrap()), ["video"]);
    assert!(!reply.contains("with sound"), "{reply}");
    // A picture isn't a soundtrack.
    let e = mcp
        .call(
            "layer_update",
            json!({"sceneId": id, "ops": [{"target": {"scene": true}, "set": {"audio": "nope"}}]}),
        )
        .await
        .unwrap_err();
    assert!(e.contains("audio: nope isn't a sound"), "{e}");
    mcp.stop().await;
}

/// The size of the file a render reply's first line names, and the line.
async fn render_mp4(mcp: &Mcp, id: &str, extra: Value) -> (u64, String) {
    let mut args = json!({"sceneId": id, "format": "mp4"});
    for (k, v) in extra.as_object().unwrap() {
        args[k] = v.clone();
    }
    let reply = mcp.ok("render", args).await;
    let line = reply.lines().next().unwrap().to_owned();
    let (_, path) = common::file_of(&line).unwrap();
    (std::fs::metadata(path).unwrap().len(), line)
}

#[tokio::test]
async fn quality_and_max_kb_hold_for_video_on_every_encoder() {
    if !have_ffmpeg() {
        return;
    }
    for encoder in ["auto", "software"] {
        let mcp = Mcp::start_args(&format!("video-rate-{encoder}"), &["--encoder", encoder]).await;
        let id = mcp
            .ok(
                "scene_create",
                json!({"sizes": ["640x360"], "duration": 2, "fps": 24}),
            )
            .await
            .split(' ')
            .next()
            .unwrap()
            .to_owned();
        mcp.ok("layer_add", json!({"sceneId": id, "layers": [
            {"type": "rect", "width": "fill", "height": "fill",
             "fill": [{"type": "linear", "angle": 30, "stops": ["#D0202E", "#1B2A5C", "#F5C518"]}, {"noise": 0.4}],
             "animate": {"scale": [1, 1.3], "rotate": [0, 8], "duration": 2}},
            {"type": "text", "text": "Quality", "fontSize": 90, "color": "#FFFFFF", "place": "center",
             "animate": {"translate": {"from": [-200, 0]}, "duration": 2}}]}))
            .await;
        let (default, line) = render_mp4(&mcp, &id, json!({})).await;
        assert!(
            line.contains(" quality 90"),
            "{encoder}: says the quality: {line}"
        );
        let (low, line) = render_mp4(&mcp, &id, json!({"quality": 20})).await;
        assert!(line.contains(" quality 20"), "{encoder}: {line}");
        assert!(
            low < default,
            "{encoder}: quality 20 {low} bytes vs 90 {default}"
        );
        let budget = default / 1024 / 3;
        let (capped, line) = render_mp4(&mcp, &id, json!({"maxKB": budget})).await;
        // libx264 lowers its quality; a hardware encoder takes a bitrate.
        let how = if encoder == "software" {
            " quality "
        } else {
            " "
        };
        assert!(
            line.contains(how) && (line.contains(" quality ") || line.contains(" bitrate ")),
            "{encoder}: says how: {line}"
        );
        // Under the budget, and not far under it.
        assert!(
            capped <= budget * 1024 && capped * 10 >= budget * 1024 * 6,
            "{encoder}: {capped} bytes for a {budget} KB budget: {line}"
        );
        // A budget nothing meets: the smallest file tried, said to be too big.
        let (tiny, line) = render_mp4(&mcp, &id, json!({"maxKB": 1})).await;
        assert!(line.contains("!too-big"), "{encoder}: {line}");
        assert!(tiny < capped, "{encoder}: {tiny} vs {capped}");
        mcp.stop().await;
    }
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
    let tools = mcp.tools().await;
    // No description offers a path it can't take, motion's words included.
    for t in tools.iter().filter(|t| t.name == "scene_create") {
        let doc = t.description.as_deref().unwrap_or_default();
        assert!(!doc.contains("path"), "{doc}");
    }
    let (doc, schema) = asset_add(tools);
    assert!(schema["properties"].get("path").is_none(), "{schema}");
    assert!(doc.contains("from url or base64"), "{doc}");
    assert!(!doc.contains("path"), "{doc}");
    // base64 is for small images; without folders a photo comes by url.
    let b64 = schema["properties"]["base64"]["description"]
        .as_str()
        .unwrap();
    assert!(
        b64.ends_with(
            "a logo or signature, since every byte passes through the model. A photo: its url."
        ),
        "{b64}"
    );
    mcp.stop().await;

    let dir = std::fs::canonicalize(scratch("paths-on")).unwrap();
    let mcp = Mcp::start_args("paths-on", &["--allow-read", dir.to_str().unwrap()]).await;
    let (_, schema) = asset_add(mcp.tools().await);
    let path = schema["properties"]["path"]["description"]
        .as_str()
        .unwrap();
    assert_eq!(path, format!("Or a local file in {}", dir.display()));
    let b64 = schema["properties"]["base64"]["description"]
        .as_str()
        .unwrap();
    assert!(b64.ends_with("A photo: its url or path."), "{b64}");
    mcp.stop().await;
}

#[tokio::test]
async fn a_clip_cut_short_fails_the_render_instead_of_freezing() {
    if !have_ffmpeg() {
        return;
    }
    let dir = scratch("cut");
    let file = dir.join("cut.mkv");
    let made = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x120:rate=25",
        ])
        .args(["-t", "4", "-c:v", "ffv1", "-pix_fmt", "bgr0"])
        .arg(&file)
        .status()
        .unwrap()
        .success();
    assert!(made, "ffmpeg made the clip");
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();

    let dir = std::fs::canonicalize(&dir).unwrap();
    let file = dir.join("cut.mkv");
    let mcp = Mcp::start_args("cut-clip", &["--allow-read", dir.to_str().unwrap()]).await;
    let id = mcp
        .ok(
            "scene_create",
            json!({"sizes": [{"id": "s", "width": 160, "height": 120}], "duration": 4, "fps": 25}),
        )
        .await
        .split(' ')
        .next()
        .unwrap()
        .to_owned();
    // ffprobe reads the header, which still says four seconds.
    let reply = mcp
        .ok(
            "asset_add",
            json!({"sceneId": id, "id": "clip", "path": file.to_str().unwrap()}),
        )
        .await;
    assert!(reply.starts_with("clip 160×120 4s 25fps"), "{reply}");
    mcp.ok(
        "layer_add",
        json!({"sceneId": id, "layers": [{"type": "video", "id": "intro", "asset": "clip", "width": 160, "height": 120}]}),
    )
    .await;
    let e = mcp
        .call("render", json!({"sceneId": id, "time": 3}))
        .await
        .unwrap_err();
    assert!(
        e.starts_with("video layer intro: its clip stopped decoding at 2.0s ("),
        "{e}"
    );
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
