//! Video files: the frames of a moving scene piped to ffmpeg as raw RGBA,
//! encoded as H.264 MP4 (what ad platforms take) or VP9 WebM.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow};

use crate::scene::{Scene, Size};

/// A video container and codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// H.264 in MP4, `yuv420p`, fast start.
    Mp4,
    /// VP9 in WebM.
    Webm,
}

/// Encodes `size` of a moving scene at `fps` as a video file, with its clips'
/// sound unless `sound` is false.
///
/// # Errors
/// No ffmpeg, a scene without `duration`, missing assets, or ffmpeg failing
/// (its own message is passed on).
pub fn render_video(
    scene: &Scene,
    size: &Size,
    fps: f32,
    assets_dir: &Path,
    container: Container,
    sound: bool,
) -> Result<Vec<u8>> {
    let ffmpeg = super::ffmpeg().map_err(|e| anyhow!(e))?;
    let (count, w, h) = crate::render::animation_dims(scene, size, fps, "video")?;
    let out = tempfile_path(container);
    let mut cmd = Command::new(&ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba"])
        .args(["-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
    let clips = if sound {
        super::audio::sources(scene, assets_dir)
    } else {
        Vec::new()
    };
    let audio = (!clips.is_empty()).then(|| super::audio::ffmpeg_args(&clips));
    // H.264 and VP9 want even sides.
    let mut graph = "[0:v]scale=trunc(iw/2)*2:trunc(ih/2)*2[v]".to_owned();
    if let Some((input, filter)) = &audio {
        cmd.args(input);
        graph = format!("{graph};{filter}");
    }
    cmd.args(["-filter_complex", &graph, "-map", "[v]"]);
    if audio.is_some() {
        let codec = if container == Container::Mp4 {
            "aac"
        } else {
            "libopus"
        };
        // The sound pads with silence; the frames set the length.
        let length = count as f32 / fps;
        cmd.args([
            "-map",
            "[a]",
            "-c:a",
            codec,
            "-b:a",
            "160k",
            "-t",
            &length.to_string(),
        ]);
    }
    match container {
        Container::Mp4 => {
            let enc = super::h264_encoder(&ffmpeg);
            cmd.args([
                "-c:v",
                &enc,
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ]);
            if enc == "libx264" {
                cmd.args(["-preset", "medium", "-crf", "20"]);
            } else {
                // Hardware encoders take a bitrate: ~0.12 bits a pixel.
                let bits = (w * h) as f32 * fps * 0.12;
                cmd.args(["-b:v", &format!("{}k", (bits / 1000.0).round().max(500.0))]);
            }
        }
        Container::Webm => {
            // yuv420p plays everywhere; left to ffmpeg, RGBA frames pick a
            // format some libvpx builds won't open.
            cmd.args([
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuv420p",
                "-b:v",
                "0",
                "-crf",
                "32",
            ])
            .args(["-row-mt", "1", "-deadline", "good", "-cpu-used", "4"]);
        }
    }
    let mut child = cmd
        .arg(&out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("can't start ffmpeg: {e}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("no pipe to ffmpeg"))?;
    let fed = crate::render::each_frame(scene, size, fps, count, assets_dir, |frame, _| {
        stdin
            .write_all(frame)
            .map_err(|e| anyhow!("ffmpeg stopped reading frames: {e}"))
    });
    drop(stdin);
    let done = child.wait_with_output()?;
    if !done.status.success() {
        let _ = std::fs::remove_file(&out);
        let why = String::from_utf8_lossy(&done.stderr);
        // The first error says why; later lines are its consequences.
        let first = why
            .lines()
            .find(|l| l.to_lowercase().contains("error"))
            .or_else(|| why.lines().last());
        return Err(anyhow!("ffmpeg failed: {}", first.unwrap_or("no message")));
    }
    fed?;
    let bytes = std::fs::read(&out)?;
    let _ = std::fs::remove_file(&out);
    Ok(bytes)
}

/// A fresh temporary file for ffmpeg to write (MP4's fast start needs a
/// seekable file, not a pipe).
fn tempfile_path(container: Container) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ext = match container {
        Container::Mp4 => "mp4",
        Container::Webm => "webm",
    };
    std::env::temp_dir().join(format!("keyline-{}-{n}.{ext}", std::process::id()))
}
