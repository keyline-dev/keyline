//! Video layers at a moment: each becomes an image layer showing the clip's
//! frame for that moment, drawn from a key the frame is decoded under.

use std::path::Path;

use super::decode::Needed;
use crate::scene::{Kind, Layer, Scene};

/// Seconds into a clip at scene time `t`: the first frame holds before
/// `delay`, and the last after the end unless it loops.
pub fn clip_time(t: f32, start: f32, delay: f32, speed: f32, looping: bool, length: f32) -> f32 {
    let local = (t - delay).max(0.0) * speed.max(0.0);
    let span = (length - start).max(0.0);
    if looping && span > 0.0 {
        start + local % span
    } else {
        start + local.min(span)
    }
}

/// `scene` with every video layer shown as its frame at `t`, and the frames
/// that needs. Unchanged, with nothing needed, when it has no video.
pub fn at(scene: &Scene, t: f32, assets_dir: &Path) -> (Scene, Vec<Needed>) {
    let mut out = scene.clone();
    let mut needed = Vec::new();
    let mut extra = Vec::new();
    fn go(
        layers: &mut [Layer],
        scene: &Scene,
        t: f32,
        dir: &Path,
        needed: &mut Vec<Needed>,
        extra: &mut Vec<(String, crate::scene::Asset)>,
    ) {
        for l in layers {
            if let Kind::Video {
                asset,
                fit,
                crop,
                focus,
                adjust,
                start,
                delay,
                speed,
                looping,
                ..
            } = &l.kind
                && let Some(a) = scene.assets.get(asset)
                && let Some(clip) = a.clip
            {
                let key = format!("video:{}", l.id);
                needed.push(Needed {
                    key: key.clone(),
                    file: dir.join(&a.sha256),
                    time: clip_time(t, *start, *delay, *speed, *looping, clip.duration),
                    size: (a.width, a.height),
                });
                extra.push((key.clone(), a.clone()));
                l.kind = Kind::Image {
                    asset: key,
                    fit: *fit,
                    crop: *crop,
                    tile_scale: 1.0,
                    focus: *focus,
                    subject: None,
                    adjust: *adjust,
                };
            }
            if let Some(children) = l.kind.children_mut() {
                go(children, scene, t, dir, needed, extra);
            }
        }
    }
    go(
        &mut out.layers,
        scene,
        t,
        assets_dir,
        &mut needed,
        &mut extra,
    );
    out.assets.extend(extra);
    (out, needed)
}

/// `scene` at `t` with its videos' frames decoded: what a still of that
/// moment draws.
///
/// # Errors
/// No ffmpeg, or a clip that can't be decoded.
pub fn still(
    scene: &Scene,
    size: &crate::scene::Size,
    t: f32,
    assets_dir: &Path,
) -> anyhow::Result<(Scene, std::collections::HashMap<String, skia_safe::Image>)> {
    let (out, needed) = at(scene, t, assets_dir);
    let mut clips = super::decode::Clips::new(scene.fps, size.width.max(size.height));
    let frames = clips.frames(&needed)?;
    Ok((out, frames))
}

/// Whether any layer of `scene` is a video.
pub fn has_video(scene: &Scene) -> bool {
    let mut any = false;
    scene.walk(&mut |l| any |= matches!(l.kind, Kind::Video { .. }));
    any
}

#[cfg(test)]
mod tests {
    use super::clip_time;

    #[test]
    fn clip_time_follows_start_delay_speed_and_loop() {
        // A 2 s clip from 0.5 s in, starting 1 s into the scene.
        assert_eq!(
            clip_time(0.0, 0.5, 1.0, 1.0, false, 2.0),
            0.5,
            "holds before it starts"
        );
        assert_eq!(clip_time(1.5, 0.5, 1.0, 1.0, false, 2.0), 1.0);
        assert_eq!(
            clip_time(9.0, 0.5, 1.0, 1.0, false, 2.0),
            2.0,
            "the last frame holds"
        );
        assert!(
            (clip_time(2.5, 0.5, 1.0, 1.0, true, 2.0) - 0.5).abs() < 1e-5
                && (clip_time(3.0, 0.5, 1.0, 1.0, true, 2.0) - 1.0).abs() < 1e-5,
            "loops back to its start after 1.5 s"
        );
        assert_eq!(clip_time(2.0, 0.0, 0.0, 0.5, false, 2.0), 1.0, "half speed");
    }
}
