//! Animated output: the scene drawn at every frame's moment, into memory on
//! the CPU (several frames at once, one per core), then encoded. Animated
//! PNG and GIF are written here, each frame after the first storing only
//! the box that changed; video formats go to ffmpeg.

use std::path::Path;

use anyhow::{Result, anyhow};
use skia_safe::{AlphaType, ColorType, ImageInfo};

use crate::scene::{Scene, Size};

/// One frame's pixels, RGBA, unpremultiplied.
type Frame = Vec<u8>;

/// Draws `size` at each of `count` moments, `1 / fps` apart, and hands each
/// frame to `emit` in order, with the frame before it. Frames are drawn a
/// batch at a time, one per core, and only a batch and one earlier frame
/// are ever held: a long story doesn't fill memory with every frame.
///
/// # Errors
/// A frame that fails to draw, or `emit` failing.
pub fn each_frame(
    scene: &Scene,
    size: &Size,
    fps: f32,
    count: usize,
    assets_dir: &Path,
    mut emit: impl FnMut(&Frame, Option<&Frame>) -> Result<()>,
) -> Result<()> {
    let cores = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut prev: Option<Frame> = None;
    // Clips decode in order on this thread; frames then draw in parallel.
    let mut clips = crate::video::frame::has_video(scene)
        .then(|| crate::video::decode::Clips::new(fps, size.width.max(size.height)));
    for start in (0..count).step_by(cores) {
        let batch = start..(start + cores).min(count);
        let mut moments = Vec::with_capacity(cores);
        for i in batch {
            let t = i as f32 / fps;
            let at = crate::anim::at_time(scene, t, size);
            let (at, needed) = crate::video::frame::at(&at, t, assets_dir);
            let seeded = match &mut clips {
                Some(c) => c.frames(&needed)?,
                None => std::collections::HashMap::new(),
            };
            moments.push((at, seeded));
        }
        let drawn: Vec<Result<Frame>> = std::thread::scope(|s| {
            let handles: Vec<_> = moments
                .into_iter()
                .map(|(at, seeded)| {
                    s.spawn(move || {
                        rgba(&super::render_image_with(
                            &at, size, 1.0, assets_dir, false, seeded,
                        )?)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(anyhow!("a frame's renderer panicked")))
                })
                .collect()
        });
        for frame in drawn {
            let frame = frame?;
            emit(&frame, prev.as_ref())?;
            prev = Some(frame);
        }
    }
    Ok(())
}

/// An image's pixels as unpremultiplied RGBA.
fn rgba(image: &skia_safe::Image) -> Result<Frame> {
    let (w, h) = (image.width(), image.height());
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let row = usize::try_from(w)? * 4;
    let mut px = vec![0u8; row * usize::try_from(h)?];
    if !image.read_pixels(
        &info,
        &mut px,
        row,
        (0, 0),
        skia_safe::image::CachingHint::Allow,
    ) {
        return Err(anyhow!("can't read a frame's pixels"));
    }
    Ok(px)
}

/// The smallest box `[x, y, w, h]` holding every pixel that differs between
/// two frames `w` wide; a 1×1 box when none does.
fn changed(a: &[u8], b: &[u8], w: usize) -> [usize; 4] {
    let h = a.len() / (w * 4);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        let (ra, rb) = (
            &a[y * w * 4..(y + 1) * w * 4],
            &b[y * w * 4..(y + 1) * w * 4],
        );
        if ra == rb {
            continue;
        }
        let first = ra
            .chunks(4)
            .zip(rb.chunks(4))
            .position(|(p, q)| p != q)
            .unwrap_or(0);
        let last = w
            - 1
            - ra.chunks(4)
                .rev()
                .zip(rb.chunks(4).rev())
                .position(|(p, q)| p != q)
                .unwrap_or(0);
        (x0, y0, x1, y1) = (x0.min(first), y0.min(y), x1.max(last), y1.max(y));
    }
    if x0 > x1 {
        return [0, 0, 1, 1];
    }
    [x0, y0, x1 - x0 + 1, y1 - y0 + 1]
}

/// An animated scene's frame count at `fps`, and its frame size in pixels.
///
/// # Errors
/// A scene without `duration` (named by `format`), or a size too big.
pub fn animation_dims(
    scene: &Scene,
    size: &Size,
    fps: f32,
    format: &str,
) -> Result<(usize, usize, usize)> {
    let duration = crate::anim::shots::length(scene)
        .ok_or_else(|| anyhow!("{format} needs an animated scene: give it a duration"))?;
    let px = |v: f32| usize::try_from(v.round().max(1.0) as i64);
    Ok((
        ((duration * fps).round() as usize).max(1),
        px(size.width)?,
        px(size.height)?,
    ))
}

/// A frame's changed box and its pixels: the whole frame first, then only
/// what differs from the frame before.
fn part(frame: &[u8], prev: Option<&Frame>, w: usize) -> ([usize; 4], Vec<u8>) {
    let bx @ [x, y, bw, bh] = match prev {
        None => [0, 0, w, frame.len() / (w * 4)],
        Some(p) => changed(p, frame, w),
    };
    let pixels = (y..y + bh)
        .flat_map(|row| &frame[(row * w + x) * 4..(row * w + x + bw) * 4])
        .copied()
        .collect();
    (bx, pixels)
}

/// Renders `size` of an animated scene as an animated PNG at `fps`: lossless
/// and fully transparent; parts that hold still cost nothing.
///
/// # Errors
/// A scene without `duration`, missing assets, or an encoding failure.
pub fn render_apng(scene: &Scene, size: &Size, fps: f32, assets_dir: &Path) -> Result<Vec<u8>> {
    let (count, w, h) = animation_dims(scene, size, fps, "apng")?;
    let (pw, ph) = (u32::try_from(w)?, u32::try_from(h)?);
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, pw, ph);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_animated(u32::try_from(count)?, if scene.looping { 0 } else { 1 })?;
    // Delay in thousandths of a second, so fractional rates stay close.
    enc.set_frame_delay((1000.0 / fps).round() as u16, 1000)?;
    let mut writer = enc.write_header()?;
    writer.set_blend_op(png::BlendOp::Source)?;
    writer.set_dispose_op(png::DisposeOp::None)?;
    each_frame(scene, size, fps, count, assets_dir, |frame, prev| {
        let ([x, y, bw, bh], pixels) = part(frame, prev, w);
        writer.reset_frame_position()?;
        writer.set_frame_dimension(u32::try_from(bw)?, u32::try_from(bh)?)?;
        writer.set_frame_position(u32::try_from(x)?, u32::try_from(y)?)?;
        writer.write_image_data(&pixels)?;
        Ok(())
    })?;
    writer.finish()?;
    Ok(out)
}

/// Renders `size` of an animated scene as an animated GIF at `fps`: plays
/// everywhere (email, chat), in at most 256 colors per frame.
///
/// # Errors
/// A scene without `duration`, a size over 65,535 px, missing assets, or
/// an encoding failure.
pub fn render_gif(scene: &Scene, size: &Size, fps: f32, assets_dir: &Path) -> Result<Vec<u8>> {
    let (count, w, h) = animation_dims(scene, size, fps, "gif")?;
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, u16::try_from(w)?, u16::try_from(h)?, &[])?;
        enc.set_repeat(if scene.looping {
            gif::Repeat::Infinite
        } else {
            gif::Repeat::Finite(0)
        })?;
        // GIF counts in hundredths; browsers slow anything under 2 down.
        let delay = ((100.0 / fps).round() as u16).max(2);
        each_frame(scene, size, fps, count, assets_dir, |frame, prev| {
            let ([x, y, bw, bh], mut pixels) = part(frame, prev, w);
            let mut frame = gif::Frame::from_rgba_speed(
                u16::try_from(bw)?,
                u16::try_from(bh)?,
                &mut pixels,
                10,
            );
            frame.left = u16::try_from(x)?;
            frame.top = u16::try_from(y)?;
            frame.delay = delay;
            frame.dispose = gif::DisposalMethod::Keep;
            enc.write_frame(&frame)?;
            Ok(())
        })?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::changed;

    #[test]
    fn only_the_changed_box_is_stored() {
        // 4×3 frames; one pixel differs at (2, 1).
        let a = vec![0u8; 4 * 3 * 4];
        let mut b = a.clone();
        b[(4 + 2) * 4] = 255;
        assert_eq!(changed(&a, &b, 4), [2, 1, 1, 1]);
        b[0] = 9;
        assert_eq!(changed(&a, &b, 4), [0, 0, 3, 2]);
        assert_eq!(changed(&a, &a, 4), [0, 0, 1, 1], "unchanged: a 1×1 frame");
    }
}
