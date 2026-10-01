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

/// How hard the encoder compresses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rate {
    /// A quality, 1–100 (90 by default): the encoder's constant quality, or
    /// for a hardware encoder a bitrate in proportion.
    Quality(u32),
    /// A video bitrate, kbit/s: what a `maxKB` budget allows.
    Bitrate(u32),
}

/// An encoded video and how it was encoded.
pub struct Video {
    /// The file.
    pub bytes: Vec<u8>,
    /// The rate it was encoded at.
    pub rate: Rate,
    /// Still over `maxKB` after every try; the file is the smallest tried.
    pub too_big: bool,
}

/// Encodes `size` of a moving scene at `fps` as a video file, with its clips'
/// sound unless `sound` is false, at `quality` (1–100). With `max_kb`, a
/// file over it is encoded again, up to three times, at a lower quality
/// (or, on a hardware encoder, a bitrate; when that still doesn't fit,
/// libx264 tries at a lower quality).
///
/// # Errors
/// No ffmpeg, a scene without `duration`, missing assets, or ffmpeg failing
/// (its own message is passed on).
#[expect(
    clippy::too_many_arguments,
    reason = "one encode's settings, passed through"
)]
pub fn render_video(
    scene: &Scene,
    size: &Size,
    fps: f32,
    assets_dir: &Path,
    container: Container,
    sound: bool,
    quality: u32,
    max_kb: Option<u32>,
) -> Result<Video> {
    let rate = Rate::Quality(quality.clamp(1, 100));
    let bytes = encode_once(
        scene,
        size,
        fps,
        assets_dir,
        container,
        sound,
        Encode {
            rate,
            strict: false,
            software: false,
        },
    )?;
    let Some(kb) = max_kb else {
        return Ok(Video {
            bytes,
            rate,
            too_big: false,
        });
    };
    let budget = f64::from(kb) * 1024.0;
    if bytes.len() as f64 <= budget {
        return Ok(Video {
            bytes,
            rate,
            too_big: false,
        });
    }
    // Constant-quality encoders (libx264, VP9) lower their quality, which
    // keeps the look even; hardware encoders take a bitrate. Some ignore
    // a low one: then libx264, which can always go lower, has its turn.
    let hardware = container == Container::Mp4
        && super::h264_encoder(&super::ffmpeg().map_err(|e| anyhow!(e))?) != "libx264";
    let secs = f64::from(crate::anim::shots::length(scene).unwrap_or(1.0)).max(0.1);
    let audio = if sound && !super::audio::sources(scene, assets_dir).is_empty() {
        160.0
    } else {
        0.0
    };
    let encode = |rate, strict, software| {
        encode_once(
            scene,
            size,
            fps,
            assets_dir,
            container,
            sound,
            Encode {
                rate,
                strict,
                software,
            },
        )
    };
    let quality = quality.clamp(1, 100);
    let mut best = if hardware {
        let start = (budget * 8.0 * 0.95 / secs / 1000.0 - audio).max(50.0);
        let first = encode(Rate::Bitrate(start as u32), false, false)?;
        search(budget, Setting::Bitrate, (start, first), |r, strict| {
            encode(r, strict, false)
        })?
    } else {
        search(
            budget,
            Setting::Quality,
            (f64::from(quality), bytes),
            |r, _| encode(r, false, false),
        )?
    };
    if best.2 && hardware {
        let first = encode(Rate::Quality(quality), false, true)?;
        let soft = search(
            budget,
            Setting::Quality,
            (f64::from(quality), first),
            |r, _| encode(r, false, true),
        )?;
        if !soft.2 || soft.0.len() < best.0.len() {
            best = soft;
        }
    }
    let (bytes, rate, too_big) = best;
    Ok(Video {
        bytes,
        rate,
        too_big,
    })
}

/// What a `maxKB` search varies.
#[derive(Clone, Copy)]
enum Setting {
    /// The quality, 1–100: 6 CRF steps, about 17 points, halve the size.
    Quality,
    /// A bitrate, kbit/s, which scales the size.
    Bitrate,
}

impl Setting {
    fn rate(self, x: f64) -> Rate {
        match self {
            Setting::Quality => Rate::Quality(x.clamp(1.0, 100.0) as u32),
            Setting::Bitrate => Rate::Bitrate(x.max(50.0) as u32),
        }
    }

    /// The setting for a size `ratio` times the target, from one at `x`.
    fn guess(self, x: f64, ratio: f64) -> f64 {
        match self {
            Setting::Quality => x - 17.4 * ratio.log2(),
            Setting::Bitrate => x / ratio,
        }
    }
}

/// Finds the file that fits `budget` bytes best, from a first try over
/// it: up to three more, each between the closest under and over so far,
/// until one fits within 85% of it; never above the first quality.
/// `encode` takes the rate and whether a bitrate's ceiling should hold to
/// it (once a try overshot). Returns the biggest that fits, else the
/// smallest and `true` (too big).
fn search(
    budget: f64,
    setting: Setting,
    first: (f64, Vec<u8>),
    mut encode: impl FnMut(Rate, bool) -> Result<Vec<u8>>,
) -> Result<(Vec<u8>, Rate, bool)> {
    let start = first.0;
    let mut tries = vec![first];
    let target = budget * 0.95;
    for _ in 0..3 {
        let len = |t: &&(f64, Vec<u8>)| t.1.len();
        let under = tries
            .iter()
            .filter(|t| t.1.len() as f64 <= budget)
            .max_by_key(len);
        let over = tries
            .iter()
            .filter(|t| t.1.len() as f64 > budget)
            .min_by_key(len);
        let next = match (under, over) {
            (Some(u), _) if u.1.len() as f64 >= budget * 0.85 => break,
            // Between the two, where the size's log is on target.
            (Some(u), Some(o)) => {
                let (lu, lo) = ((u.1.len() as f64).ln(), (o.1.len() as f64).ln());
                u.0 + (o.0 - u.0) * (target.ln() - lu) / (lo - lu)
            }
            (Some(t), None) | (None, Some(t)) => setting.guess(t.0, t.1.len() as f64 / target),
            (None, None) => break,
        };
        let next = match setting {
            Setting::Quality => next.min(start),
            Setting::Bitrate => next,
        }
        .floor();
        let rate = setting.rate(next);
        if tries.iter().any(|t| setting.rate(t.0) == rate) {
            break;
        }
        let strict = tries.iter().any(|t| t.1.len() as f64 > budget);
        tries.push((next, encode(rate, strict)?));
    }
    let fits = |t: &(f64, Vec<u8>)| t.1.len() as f64 <= budget;
    let at = match tries
        .iter()
        .enumerate()
        .filter(|(_, t)| fits(t))
        .max_by_key(|(_, t)| t.1.len())
    {
        Some((i, _)) => i,
        None => tries
            .iter()
            .enumerate()
            .min_by_key(|(_, t)| t.1.len())
            .map_or(0, |(i, _)| i),
    };
    let (x, bytes) = tries.swap_remove(at);
    let too_big = bytes.len() as f64 > budget;
    Ok((bytes, setting.rate(x), too_big))
}

/// One encode's settings: the rate, whether a bitrate's ceiling holds to
/// the bitrate itself, and whether to use libx264 whatever the encoder.
#[derive(Clone, Copy)]
struct Encode {
    rate: Rate,
    strict: bool,
    software: bool,
}

/// One ffmpeg encode.
fn encode_once(
    scene: &Scene,
    size: &Size,
    fps: f32,
    assets_dir: &Path,
    container: Container,
    sound: bool,
    Encode {
        rate,
        strict,
        software,
    }: Encode,
) -> Result<Vec<u8>> {
    let ffmpeg = super::ffmpeg().map_err(|e| anyhow!(e))?;
    let (count, w, h) = crate::render::animation_dims(scene, size, fps, "video")?;
    // MP4's fast start needs a seekable file, not a pipe; the path deletes
    // the file when it drops, on every return.
    let out = tempfile::Builder::new()
        .prefix("keyline-")
        .suffix(match container {
            Container::Mp4 => ".mp4",
            Container::Webm => ".webm",
        })
        .tempfile()?
        .into_temp_path();
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
            let enc = if software {
                "libx264".to_owned()
            } else {
                super::h264_encoder(&ffmpeg)
            };
            cmd.args([
                "-c:v",
                &enc,
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ]);
            match rate {
                // Quality 90 is crf 20; 1 is 51, the worst.
                Rate::Quality(q) if enc == "libx264" => {
                    let crf = (51.0 - 0.345 * q as f32).round().clamp(0.0, 51.0);
                    cmd.args(["-preset", "medium", "-crf", &crf.to_string()]);
                }
                // Hardware encoders take a bitrate: ~0.12 bits a pixel at 90.
                Rate::Quality(q) => {
                    let per_px = 0.12 * (q as f32 / 90.0).powi(2);
                    let bits = (w * h) as f32 * fps * per_px;
                    cmd.args(["-b:v", &format!("{}k", (bits / 1000.0).round().max(100.0))]);
                }
                // A ceiling at the target kept the average near 60% of it,
                // so the first try allows more; `strict` after an overshoot.
                Rate::Bitrate(k) => {
                    let (max, buf) = if strict { (k, k) } else { (k * 3 / 2, k * 2) };
                    cmd.args([
                        "-b:v",
                        &format!("{k}k"),
                        "-maxrate",
                        &format!("{max}k"),
                        "-bufsize",
                        &format!("{buf}k"),
                    ]);
                    if enc == "libx264" {
                        cmd.args(["-preset", "medium"]);
                    }
                }
            }
        }
        Container::Webm => {
            // yuv420p plays everywhere; left to ffmpeg, RGBA frames pick a
            // format some libvpx builds won't open.
            cmd.args(["-c:v", "libvpx-vp9", "-pix_fmt", "yuv420p"]);
            match rate {
                // Quality 90 is crf 32; 1 is 63, the worst.
                Rate::Quality(q) => {
                    let crf = (63.0 - 0.345 * q as f32).round().clamp(0.0, 63.0);
                    cmd.args(["-b:v", "0", "-crf", &crf.to_string()]);
                }
                Rate::Bitrate(k) => {
                    let k = format!("{k}k");
                    cmd.args(["-b:v", &k, "-maxrate", &k]);
                }
            }
            cmd.args(["-row-mt", "1", "-deadline", "good", "-cpu-used", "4"]);
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
        let why = String::from_utf8_lossy(&done.stderr);
        // The first error says why; later lines are its consequences.
        let first = why
            .lines()
            .find(|l| l.to_lowercase().contains("error"))
            .or_else(|| why.lines().last());
        return Err(anyhow!("ffmpeg failed: {}", first.unwrap_or("no message")));
    }
    fed?;
    Ok(std::fs::read(&out)?)
}
