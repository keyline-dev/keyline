//! What a video or sound file holds: its size, length and frame rate, from
//! ffprobe.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::scene::Clip;

/// How many frames a video file's first video stream holds, counted from
/// its packets (fast: nothing is decoded).
///
/// # Errors
/// No ffprobe, or a file it can't read.
pub fn video_frames(file: &Path) -> Result<usize, String> {
    let out = Command::new(super::ffprobe()?)
        .args(["-v", "error", "-select_streams", "v:0", "-count_packets"])
        .args(["-show_entries", "stream=nb_read_packets", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .map_err(|e| format!("can't run ffprobe: {e}"))?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(|_| "ffprobe can't count the video's frames".to_owned())
}

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
    let v: Probe = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let kind = |s: &Stream, k: &str| s.codec_type == k;
    let audio = v.streams.iter().any(|s| kind(s, "audio"));
    let duration = v
        .format
        .duration
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);
    // An MP3's cover art is a one-picture "video" stream.
    let Some(stream) = v
        .streams
        .iter()
        .find(|s| kind(s, "video") && s.disposition.attached_pic != 1)
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
    let (Some(w), Some(h)) = (stream.width, stream.height) else {
        return Ok(None);
    };
    // "30000/1001" and the like.
    let fps = stream
        .avg_frame_rate
        .as_deref()
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

/// The parts of ffprobe's JSON that [`probe`] reads.
#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    streams: Vec<Stream>,
    #[serde(default)]
    format: Format,
}

#[derive(Deserialize, Default)]
struct Format {
    /// Seconds, as a decimal string.
    duration: Option<String>,
}

#[derive(Deserialize)]
struct Stream {
    #[serde(default)]
    codec_type: String,
    width: Option<u32>,
    height: Option<u32>,
    /// A fraction, such as "30000/1001".
    avg_frame_rate: Option<String>,
    #[serde(default)]
    disposition: Disposition,
}

#[derive(Deserialize, Default)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_counted_from_the_file() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let file =
            std::env::temp_dir().join(format!("keyline-probe-frames-{}.mkv", std::process::id()));
        let made = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=64x48:rate=25",
            ])
            .args(["-t", "2", "-c:v", "ffv1"])
            .arg(&file)
            .status()
            .unwrap()
            .success();
        assert!(made, "ffmpeg made the clip");
        assert_eq!(video_frames(&file), Ok(50));
        let _ = std::fs::remove_file(&file);
    }
}
