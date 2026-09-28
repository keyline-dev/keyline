//! The sound track of a video: each clip's own sound, timed with the clip
//! and mixed. A clip in a shot is heard only during its shot.

use std::path::{Path, PathBuf};

use crate::anim::shots;
use crate::scene::{Kind, Layer, Scene};

/// A clip whose sound the video carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The clip's file.
    pub file: PathBuf,
    /// Seconds into the clip where it starts.
    pub start: f32,
    /// When it starts in the video, seconds.
    pub offset: f32,
    /// When it stops in the video (its shot's end, or the video's).
    pub until: f32,
    /// Playback speed.
    pub speed: f32,
    /// Whether it repeats.
    pub looping: bool,
}

/// The clips in `scene` with sound that isn't turned off, in layer order.
pub fn sources(scene: &Scene, assets_dir: &Path) -> Vec<Source> {
    let length = shots::length(scene).unwrap_or(0.0);
    let windows: Vec<(usize, f32, f32)> = shots::timeline(scene)
        .into_iter()
        .map(|(i, start, dur, _)| (i, start, start + dur))
        .collect();
    let mut found = Vec::new();
    for (i, l) in scene.layers.iter().enumerate() {
        let (base, until) = windows
            .iter()
            .find(|w| w.0 == i)
            .map_or((0.0, length), |w| (w.1, w.2));
        visit(l, &mut |v| {
            if let Kind::Video {
                asset,
                start,
                delay,
                speed,
                looping,
                audio: true,
                ..
            } = &v.kind
                && let Some(a) = scene.assets.get(asset)
                && a.clip.is_some_and(|c| c.audio)
                // A clip that starts after its shot ends is never heard.
                && base + delay < until
            {
                found.push(Source {
                    file: assets_dir.join(&a.sha256),
                    start: *start,
                    offset: base + delay,
                    until,
                    speed: *speed,
                    looping: *looping,
                });
            }
        });
    }
    found
}

/// Each shown layer under `l`: a hidden layer's clips are silent. Shots
/// show in their time, whatever `hidden` says at rest.
fn visit(l: &Layer, f: &mut impl FnMut(&Layer)) {
    if l.hidden && l.time.shot.is_none() {
        return;
    }
    f(l);
    for c in l.kind.children().into_iter().flatten() {
        visit(c, f);
    }
}

/// ffmpeg's arguments for the sounds as inputs 1, 2, …: the input options,
/// and the filter making `[a]`, the tracks mixed, each starting, playing
/// and stopping in time.
pub fn ffmpeg_args(sources: &[Source]) -> (Vec<String>, String) {
    let mut inputs = Vec::new();
    let mut filters = Vec::new();
    for (i, s) in sources.iter().enumerate() {
        let (input, filter) = one(s, i + 1);
        inputs.extend(input);
        filters.push(filter);
    }
    let labels: String = (1..=sources.len()).map(|i| format!("[a{i}]")).collect();
    // normalize=0: each clip keeps its own loudness.
    filters.push(format!(
        "{labels}amix=inputs={}:duration=longest:normalize=0[a]",
        sources.len()
    ));
    (inputs, filters.join(";"))
}

/// One clip as input `n`, filtered to `[a<n>]`.
fn one(s: &Source, n: usize) -> (Vec<String>, String) {
    let input = vec!["-i".into(), s.file.display().to_string()];
    // How much of the clip plays: its time in the video, at its speed.
    let played = (s.until - s.offset) * s.speed;
    let delay_ms = (s.offset * 1000.0).round();
    let tempo = tempo(s.speed);
    // A loop repeats from `start`, as the pictures do.
    let looped = if s.looping {
        ",aloop=loop=-1:size=2147483647"
    } else {
        ""
    };
    let start = s.start;
    let filter = format!(
        "[{n}:a]atrim=start={start},asetpts=PTS-STARTPTS{looped},atrim=0:{played},\
         asetpts=PTS-STARTPTS,{tempo},adelay={delay_ms}:all=1,apad[a{n}]"
    );
    (input, filter)
}

/// `atempo` filters for `speed`: one takes 0.5–100, so slower speeds
/// chain halvings.
fn tempo(speed: f32) -> String {
    let mut rest = speed.clamp(0.01, 100.0);
    let mut steps = Vec::new();
    while rest < 0.5 {
        steps.push("atempo=0.5".to_owned());
        rest /= 0.5;
    }
    steps.push(format!("atempo={rest}"));
    steps.join(",")
}

#[cfg(test)]
mod tests {
    use super::{Source, ffmpeg_args, sources, tempo};

    #[test]
    fn slow_motion_chains_tempo_steps() {
        assert_eq!(tempo(0.25), "atempo=0.5,atempo=0.5");
        assert_eq!(tempo(0.2), "atempo=0.5,atempo=0.5,atempo=0.8");
        assert_eq!(tempo(2.0), "atempo=2");
    }
    use crate::scene::Scene;
    use serde_json::json;

    #[test]
    fn each_shot_is_heard_in_its_time_and_muted_clips_not_at_all() {
        let clip = json!({"sha256": "c", "width": 10, "height": 10,
            "clip": {"duration": 5, "fps": 30, "audio": true}});
        let scene: Scene = serde_json::from_value(json!({"width": 10, "height": 10,
            "sizes": [{"id": "a", "width": 10, "height": 10}],
            "assets": {"v": clip},
            "layers": [
                {"type": "frame", "shot": {"duration": 1},
                 "children": [{"type": "video", "asset": "v", "audio": false}]},
                {"type": "frame", "hidden": true,
                 "shot": {"duration": 2, "transition": {"type": "fade", "duration": 0.4}},
                 "children": [{"type": "video", "asset": "v", "delay": 0.1}]}]}))
        .unwrap();
        let s = sources(&scene, "/a".as_ref());
        assert_eq!(s.len(), 1, "{s:?}");
        assert!(
            (s[0].offset - 0.7).abs() < 1e-5 && (s[0].until - 2.6).abs() < 1e-5,
            "{s:?}"
        );
    }

    #[test]
    fn the_track_is_trimmed_sped_and_delayed_to_its_clip() {
        let s = Source {
            file: "/a/clip".into(),
            start: 2.0,
            offset: 1.5,
            until: 4.0,
            speed: 2.0,
            looping: false,
        };
        let later = Source {
            offset: 3.0,
            looping: true,
            ..s.clone()
        };
        let (input, filter) = ffmpeg_args(&[s, later]);
        assert_eq!(input, ["-i", "/a/clip", "-i", "/a/clip"]);
        assert_eq!(
            filter,
            "[1:a]atrim=start=2,asetpts=PTS-STARTPTS,atrim=0:5,asetpts=PTS-STARTPTS,atempo=2,\
             adelay=1500:all=1,apad[a1];\
             [2:a]atrim=start=2,asetpts=PTS-STARTPTS,aloop=loop=-1:size=2147483647,atrim=0:2,\
             asetpts=PTS-STARTPTS,atempo=2,adelay=3000:all=1,apad[a2];\
             [a1][a2]amix=inputs=2:duration=longest:normalize=0[a]"
        );
    }
}
