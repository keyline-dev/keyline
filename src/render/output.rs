//! Encoded output: PNG bytes and the preview contact sheet.

use std::path::Path;

use anyhow::{Result, anyhow};
use skia_safe::{EncodedImageFormat, Image, surfaces};

use super::render_image;
use crate::scene::{Scene, Size};

/// One preview PNG with every size side by side, each scaled to `height`
/// px: one small image instead of one per size, to save image tokens.
pub fn contact_sheet(
    scene: &Scene,
    sizes: &[Size],
    height: f32,
    assets_dir: &Path,
) -> Result<Vec<u8>> {
    const GAP: f32 = 8.0;
    let shots = sizes
        .iter()
        .map(|s| render_image(scene, s, (height / s.height).min(1.0), assets_dir, false))
        .collect::<Result<Vec<_>>>()?;
    let w: i32 =
        shots.iter().map(Image::width).sum::<i32>() + GAP as i32 * (shots.len() as i32 + 1);
    let h = shots.iter().map(Image::height).max().unwrap_or(1) + 2 * GAP as i32;
    let mut surface =
        surfaces::raster_n32_premul((w, h)).ok_or_else(|| anyhow!("can't allocate {w}×{h}"))?;
    let canvas = surface.canvas();
    canvas.clear(skia_safe::Color::from_rgb(128, 128, 128));
    let mut x = GAP;
    for shot in &shots {
        canvas.draw_image(shot, (x, GAP), None);
        x += shot.width() as f32 + GAP;
    }
    encode_png(&surface.image_snapshot())
}

pub(super) fn encode_png(image: &Image) -> Result<Vec<u8>> {
    let png = image
        .encode(None, EncodedImageFormat::PNG, None)
        .ok_or_else(|| anyhow!("PNG encoding failed"))?;
    Ok(png.as_bytes().to_vec())
}
