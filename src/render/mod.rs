//! Draws a laid-out scene with Skia on the CPU and encodes it as PNG.

mod image;
mod output;
mod paint;
#[cfg(test)]
mod tests;
mod text;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    ClipOp, FilterMode, Image, MipmapMode, Paint, SamplingOptions, canvas::SaveLayerRec, surfaces,
};

use crate::gpu::{self, Backend};
use crate::layout::{Placed, layout};
use crate::scene::{BlendMode, Color, Fit, Kind, Scene, Size};

use image::{CENTER, image_rect, svg_image};
use output::encode_png;
use paint::{draw_stroke, fill_paint, rrect, sk_blend, sk_color, sk_rect};

pub use image::{image_crop, image_scale, raster_size, svg_size};
pub use output::contact_sheet;

/// Largest side, in pixels, an SVG is rasterized at.
const MAX_SVG_PX: f32 = 8192.0;

/// Renders `size` at `px` device pixels per scene pixel (1 for full size,
/// less for previews) on the CPU. `assets_dir` holds asset bytes named by
/// SHA-256. Byte-identical everywhere: tests and previews use it.
pub fn render_png(scene: &Scene, size: &Size, px: f32, assets_dir: &Path) -> Result<Vec<u8>> {
    encode_png(&render_image(scene, size, px, assets_dir, false)?)
}

/// Like [`render_png`] at full size, on `backend`: the GPU when asked for
/// and available, else the CPU.
///
/// # Errors
/// Missing assets, or a GPU present that fails mid-render.
pub fn render_png_on(
    scene: &Scene,
    size: &Size,
    assets_dir: &Path,
    backend: Backend,
) -> Result<Vec<u8>> {
    if backend == Backend::Gpu {
        let (w, h) = pixel_size(size, 1.0);
        let drawn = gpu::render_png(w, h, |canvas| {
            draw_scene(canvas, scene, size, 1.0, assets_dir, false)
        })?;
        if let Some(png) = drawn {
            return Ok(png);
        }
    }
    render_png(scene, size, 1.0, assets_dir)
}

fn pixel_size(size: &Size, px: f32) -> (i32, i32) {
    (
        (size.width * px).round().max(1.0) as i32,
        (size.height * px).round().max(1.0) as i32,
    )
}

/// Renders to a raster image. With `hide_text`, text layers are skipped:
/// the backdrop that text-contrast checks sample.
pub fn render_image(
    scene: &Scene,
    size: &Size,
    px: f32,
    assets_dir: &Path,
    hide_text: bool,
) -> Result<Image> {
    let (w, h) = pixel_size(size, px);
    let mut surface =
        surfaces::raster_n32_premul((w, h)).ok_or_else(|| anyhow!("can't allocate {w}×{h}"))?;
    draw_scene(surface.canvas(), scene, size, px, assets_dir, hide_text)?;
    Ok(surface.image_snapshot())
}

/// Draws the scene onto any canvas, CPU or GPU.
fn draw_scene(
    canvas: &skia_safe::Canvas,
    scene: &Scene,
    size: &Size,
    px: f32,
    assets_dir: &Path,
    hide_text: bool,
) -> Result<()> {
    canvas.clear(sk_color(scene.background));
    canvas.scale((px, px));
    let mut ctx = Ctx {
        scene,
        px,
        assets_dir,
        hide_text,
        rasters: HashMap::new(),
    };
    let scene = &*scene.for_size(&size.id);
    for p in &layout(scene, size) {
        ctx.draw(canvas, p)?;
    }
    Ok(())
}

struct Ctx<'a> {
    scene: &'a Scene,
    px: f32,
    assets_dir: &'a Path,
    hide_text: bool,
    /// Decoded raster assets, so an icon used three times decodes once.
    rasters: HashMap<String, Image>,
}

impl Ctx<'_> {
    fn draw(&mut self, canvas: &skia_safe::Canvas, p: &Placed) -> Result<()> {
        let l = p.layer;
        if l.opacity < 1.0 || l.blend_mode != BlendMode::Normal || l.mask.is_some() {
            // Composite the whole layer (children included) as one.
            let mut layer = Paint::default();
            layer.set_alpha_f(l.opacity);
            layer.set_blend_mode(sk_blend(l.blend_mode));
            canvas.save_layer(&SaveLayerRec::default().paint(&layer));
        } else {
            canvas.save();
        }
        let r = sk_rect(p.rect);
        if l.rotation != 0.0 {
            canvas.rotate(l.rotation, Some(r.center()));
        }
        match &l.kind {
            Kind::Rect {
                color,
                gradient,
                stroke,
                corner_radius,
            } => {
                let radius = corner_radius * p.k;
                if let Some(fill) = fill_paint(*color, gradient.as_ref(), r) {
                    canvas.draw_rrect(rrect(r, radius), &fill);
                }
                if let Some(s) = stroke {
                    draw_stroke(canvas, s, r, radius, p.k);
                }
            }
            Kind::Ellipse {
                color,
                gradient,
                stroke,
            } => {
                if let Some(fill) = fill_paint(*color, gradient.as_ref(), r) {
                    canvas.draw_oval(r, &fill);
                }
                if let Some(s) = stroke {
                    // Radii past half the box make Skia draw an ellipse.
                    draw_stroke(canvas, s, r, r.width().max(r.height()), p.k);
                }
            }
            Kind::Line {
                color,
                stroke_width,
            } => {
                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                paint.set_color(sk_color(*color));
                paint.set_stroke_width(stroke_width * p.k);
                canvas.draw_line((r.left, r.top), (r.right, r.bottom), &paint);
            }
            Kind::Icon {
                name,
                set,
                color,
                stroke_width,
            } => {
                let svg = crate::icons::svg(*set, name, *color, *stroke_width)
                    .ok_or_else(|| anyhow!("no {set} icon {name}"))?;
                let aspect = crate::icons::aspect(*set, name).unwrap_or(1.0);
                // Contained in the box, centered, like `fit: fit`.
                let d = image_rect(p.rect, aspect, 1.0, Fit::Fit, None, 1.0, CENTER);
                let pw = (d.w * self.px).ceil().clamp(1.0, MAX_SVG_PX) as u32;
                let ph = (d.h * self.px).ceil().clamp(1.0, MAX_SVG_PX) as u32;
                let img =
                    svg_image(svg.as_bytes(), pw, ph).with_context(|| format!("icon {name}"))?;
                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                canvas.draw_image_rect_with_sampling_options(
                    &img,
                    None,
                    sk_rect(d),
                    SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    &paint,
                );
            }
            Kind::Frame {
                clip,
                color,
                gradient,
                stroke,
                corner_radius,
                ..
            } => {
                let radius = corner_radius * p.k;
                let rr = rrect(r, radius);
                if let Some(fill) = fill_paint(*color, gradient.as_ref(), r) {
                    canvas.draw_rrect(rr, &fill);
                }
                canvas.save();
                if *clip {
                    canvas.clip_rrect(rr, ClipOp::Intersect, true);
                }
                for child in &p.children {
                    self.draw(canvas, child)?;
                }
                canvas.restore();
                // As in design tools, a frame's stroke sits above its children, unclipped.
                if let Some(s) = stroke {
                    draw_stroke(canvas, s, r, radius, p.k);
                }
            }
            Kind::Text { .. } if self.hide_text => {}
            Kind::Text {
                fill,
                gradient,
                outline,
                ..
            } => self.draw_text(
                canvas,
                p,
                r,
                fill.as_ref(),
                gradient.as_ref(),
                outline.as_ref(),
            )?,
            Kind::Image {
                asset,
                fit,
                crop,
                tile_scale,
                focus,
            } => {
                let paint =
                    self.image_paint(asset, p.rect, *fit, crop.as_ref(), tile_scale * p.k, *focus)?;
                canvas.draw_rect(r, &paint);
            }
        }
        if let Some(m) = &l.mask {
            // Keep what's drawn in proportion to the gradient's alpha.
            let mut stops = m.clone();
            for s in &mut stops.stops {
                s.color = Color(s.color.0 & 0xFF00_0000);
            }
            if let Some(mut paint) = fill_paint(None, Some(&stops), r) {
                paint.set_blend_mode(skia_safe::BlendMode::DstIn);
                canvas.draw_rect(r, &paint);
            }
        }
        canvas.restore();
        Ok(())
    }
}
