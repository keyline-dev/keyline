//! What a video or sound file holds: its size, length and frame rate, from
//! ffprobe.

use std::path::Path;
use std::process::Command;

use crate::scene::Clip;

/// The first video stream's `(width, height, clip)`; `(0, 0, clip)` for a
/// sound file (MP3, M4A, WAV); `None` for anything else (an unknown file, a
/// still image).
///
/// # Errors
/// No ffprobe.
pub fn probe(file: &Path) -> Result<Option<(f32, f32, Clip)>, String> {
    let out = Command::new(super::ffprobe()?)
        .args(["-v", "error"])
        .args([
            "-show_entries",
            "stream=codec_type,width,height,avg_frame_rate:stream_disposition=attached_pic:format=duration",
        ])
        .args(["-of", "json"])
        .arg(file)
        .output()
        .map_err(|e| format!("can't run ffprobe: {e}"))?;
    if !out.status.success() {
        return Ok(None);
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let kind = |s: &serde_json::Value, k: &str| s["codec_type"].as_str() == Some(k);
    let audio = streams.iter().any(|s| kind(s, "audio"));
    let num = |x: &serde_json::Value| {
        x.as_f64()
            .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
    };
    let duration = num(&v["format"]["duration"]).unwrap_or(0.0);
    // An MP3's cover art is a one-picture "video" stream.
    let Some(stream) = streams
        .iter()
        .find(|s| kind(s, "video") && s["disposition"]["attached_pic"].as_i64() != Some(1))
    else {
        return Ok((audio && duration > 0.0).then_some((
            0.0,
            0.0,
            Clip {
                duration: duration as f32,
                fps: 0.0,
                audio,
            },
        )));
    };
    let (Some(w), Some(h)) = (num(&stream["width"]), num(&stream["height"])) else {
        return Ok(None);
    };
    // "30000/1001" and the like.
    let fps = stream["avg_frame_rate"]
        .as_str()
        .and_then(|r| r.split_once('/'))
        .and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?))
        .filter(|f| f.is_finite() && *f > 0.0)
        .unwrap_or(30.0);
    if duration <= 0.0 {
        // A single image ffprobe can read (PNG, JPEG): not a clip.
        return Ok(None);
    }
    Ok(Some((
        w as f32,
        h as f32,
        Clip {
            duration: duration as f32,
            fps: fps as f32,
            audio,
        },
    )))
}
