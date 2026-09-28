//! Clip frames for drawing: each clip decoded by ffmpeg as one stream of raw
//! frames at the scene's frame rate, scaled down to the output size, and
//! read in order as the scene's frames need them.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

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
    /// Index of the next frame the stream will give.
    next: usize,
    last: Option<Image>,
}

impl Reader {
    fn open(file: &Path, fps: f32, (w, h): (usize, usize)) -> Result<Reader> {
        let mut child = Command::new(super::ffmpeg().map_err(|e| anyhow!(e))?)
            .args(["-v", "error", "-i"])
            .arg(file)
            .args(["-vf", &format!("fps={fps},scale={w}:{h}")])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("can't start ffmpeg: {e}"))?;
        let out = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("no pipe from ffmpeg"))?;
        Ok(Reader {
            child,
            out,
            w,
            h,
            next: 0,
            last: None,
        })
    }

    /// Frame `index` of the stream, or the last one when the clip has ended.
    fn frame(&mut self, index: usize) -> Result<Image> {
        while self.next <= index {
            let mut px = vec![0u8; self.w * self.h * 4];
            if self.out.read_exact(&mut px).is_err() {
                break; // The clip ended: its last frame holds.
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
                let reader = Reader::open(&n.file, self.fps, (side(n.size.0), side(n.size.1)))?;
                self.readers.insert(n.key.clone(), reader);
            }
            if let Some(r) = self.readers.get_mut(&n.key) {
                out.insert(n.key.clone(), r.frame(index)?);
            }
        }
        Ok(out)
    }
}
