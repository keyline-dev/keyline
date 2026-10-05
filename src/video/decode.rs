//! Clip frames for drawing: each clip decoded by ffmpeg as one stream of raw
//! frames at the scene's frame rate, scaled down to the output size, and
//! read in order as the scene's frames need them.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

use anyhow::{Result, anyhow};
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, images};

/// One clip frame a drawing needs: the clip's asset file, the moment in
/// the clip, and the raster key it's drawn from.
#[derive(Debug, Clone, PartialEq)]
pub struct Needed {
    /// Key the frame is drawn from (`video:<layer id>`).
    pub key: String,
    /// The clip's file in the asset store.
    pub file: PathBuf,
    /// Seconds into the clip.
    pub time: f32,
    /// The clip's size, px.
    pub size: (f32, f32),
}

/// A clip being decoded: frames arrive in order, and the last one read is
/// kept (it holds once the clip ends).
struct Reader {
    child: Child,
    out: ChildStdout,
    w: usize,
    h: usize,
    fps: f32,
    /// What the clip is drawn by, for errors (`video layer intro`).
    name: String,
    /// Index of the next frame the stream will give.
    next: usize,
    last: Option<Image>,
    /// ffmpeg's error output, read on its own thread so a damaged file's
    /// errors can't fill the pipe and stall the frames.
    errors: Option<JoinHandle<String>>,
}

impl Reader {
    fn open(name: String, file: &Path, fps: f32, (w, h): (usize, usize)) -> Result<Reader> {
        let mut child = Command::new(super::ffmpeg().map_err(|e| anyhow!(e))?)
            .args(["-v", "error", "-i"])
            .arg(file)
            .args(["-vf", &format!("fps={fps},scale={w}:{h}")])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("can't start ffmpeg: {e}"))?;
        let out = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("no pipe from ffmpeg"))?;
        let errors = child.stderr.take().map(|mut err| {
            std::thread::spawn(move || {
                let mut text = String::new();
                let _ = err.read_to_string(&mut text);
                text
            })
        });
        Ok(Reader {
            child,
            out,
            w,
            h,
            fps,
            name,
            next: 0,
            last: None,
            errors,
        })
    }

    /// Frame `index` of the stream, or the last one when the clip has ended.
    ///
    /// # Errors
    /// The clip stopped decoding before its end (a file cut short or
    /// damaged), or has no frames.
    fn frame(&mut self, index: usize) -> Result<Image> {
        while self.next <= index {
            let mut px = vec![0u8; self.w * self.h * 4];
            if self.out.read_exact(&mut px).is_err() {
                // The stream ended. With nothing on ffmpeg's error output
                // that's the clip's end, and its last frame holds; anything
                // there means the file stopped early, which would otherwise
                // freeze a frame without a word.
                if let Some(why) = self
                    .errors
                    .take()
                    .and_then(|e| first_error(&e.join().unwrap_or_default()))
                {
                    #[expect(clippy::cast_precision_loss, reason = "a frame index, far below 2^24")]
                    let at = self.next as f32 / self.fps;
                    return Err(anyhow!(
                        "{}: its clip stopped decoding at {at:.1}s ({why}); re-encode or replace the clip",
                        self.name
                    ));
                }
                break;
            }
            let info = ImageInfo::new(
                (i32::try_from(self.w)?, i32::try_from(self.h)?),
                ColorType::RGBA8888,
                AlphaType::Unpremul,
                None,
            );
            self.last = images::raster_from_data(&info, Data::new_copy(&px), self.w * 4);
            self.next += 1;
        }
        self.last
            .clone()
            .ok_or_else(|| anyhow!("the clip has no frames"))
    }
}

/// The first line of ffmpeg's error output, without its `[demuxer @ 0x…]`
/// prefix; `None` when there's nothing.
fn first_error(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line = match line.strip_prefix('[').and_then(|l| l.split_once("] ")) {
        Some((_, rest)) => rest,
        None => line,
    };
    Some(line.to_owned())
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The clips of one render, each read as far as the frames drawn so far.
pub struct Clips {
    fps: f32,
    /// Largest side a decoded frame needs, px.
    max_side: f32,
    readers: HashMap<String, Reader>,
}

impl Clips {
    /// Clips decoded at `fps`, no larger than `max_side` px on either side.
    pub fn new(fps: f32, max_side: f32) -> Clips {
        Clips {
            fps,
            max_side,
            readers: HashMap::new(),
        }
    }

    /// The frames `needed` for one moment, by key. A clip read past the
    /// moment asked for (a loop starting over) is opened again.
    ///
    /// # Errors
    /// No ffmpeg, or a clip that can't be decoded.
    pub fn frames(&mut self, needed: &[Needed]) -> Result<HashMap<String, Image>> {
        let mut out = HashMap::new();
        for n in needed {
            let index = (n.time * self.fps + 1e-3).floor().max(0.0) as usize;
            let behind = self.readers.get(&n.key).is_some_and(|r| r.next > index + 1);
            if behind || !self.readers.contains_key(&n.key) {
                let scale = (self.max_side / n.size.0.max(n.size.1)).min(1.0);
                // Even sides, at least 2 px.
                let side = |v: f32| (((v * scale) / 2.0).round() as usize * 2).max(2);
                let name = n
                    .key
                    .strip_prefix("video:")
                    .map_or_else(|| n.key.clone(), |id| format!("video layer {id}"));
                let reader =
                    Reader::open(name, &n.file, self.fps, (side(n.size.0), side(n.size.1)))?;
                self.readers.insert(n.key.clone(), reader);
            }
            if let Some(r) = self.readers.get_mut(&n.key) {
                out.insert(n.key.clone(), r.frame(index)?);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A four-second 160×120 clip in `dir`, cut to half its bytes when
    /// `cut`; `None` without ffmpeg, which CI must have.
    fn clip(dir: &Path, cut: bool) -> Option<PathBuf> {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            assert!(
                std::env::var_os("CI").is_none(),
                "CI needs ffmpeg installed"
            );
            return None;
        }
        let file = dir.join(if cut { "cut.mkv" } else { "whole.mkv" });
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
        if cut {
            let bytes = std::fs::read(&file).unwrap();
            std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
        }
        Some(file)
    }

    fn at(file: PathBuf, time: f32) -> Result<HashMap<String, Image>> {
        Clips::new(25.0, 160.0).frames(&[Needed {
            key: "video:intro".into(),
            file,
            time,
            size: (160.0, 120.0),
        }])
    }

    #[test]
    fn a_clip_cut_short_is_an_error_not_a_frozen_frame() {
        let dir = std::env::temp_dir().join(format!("keyline-decode-cut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let Some(file) = clip(&dir, true) else { return };
        let e = at(file, 3.0).unwrap_err().to_string();
        assert!(
            e.starts_with("video layer intro: its clip stopped decoding at 2.0s ("),
            "{e}"
        );
        assert!(e.ends_with("); re-encode or replace the clip"), "{e}");
    }

    #[test]
    fn a_whole_clip_holds_its_last_frame_past_its_end() {
        let dir = std::env::temp_dir().join(format!("keyline-decode-whole-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let Some(file) = clip(&dir, false) else {
            return;
        };
        assert!(at(file, 5.0).unwrap().contains_key("video:intro"));
    }

    #[test]
    fn ffmpeg_errors_lose_their_demuxer_prefix() {
        assert_eq!(
            first_error("\n[matroska,webm @ 0x7c73074000] File ended prematurely\nmore\n")
                .as_deref(),
            Some("File ended prematurely")
        );
        assert_eq!(first_error("  \n"), None);
    }
}
