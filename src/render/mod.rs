//! Draws a laid-out scene with Skia on the CPU and encodes it as PNG.

mod animated;
mod effects;
mod fills;
mod image;
mod mask;
mod output;
mod paint;
#[cfg(test)]
mod paint_tests;
mod rough;
mod shape;
mod split;
mod stroke;
#[cfg(test)]
mod tests;
mod text;
mod text_extras;
#[cfg(test)]
mod text_tests;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    ClipOp, FilterMode, Image, MipmapMode, Paint, SamplingOptions, canvas::SaveLayerRec, surfaces,
};

use crate::gpu::{self, Backend};
use crate::layout::{Placed, layout};
use crate::scene::{BlendMode, Fit, Kind, Layer, Mask, MaskSource, OneOrMany, Scene, Size, Stroke};

use image::{CENTER, image_rect, svg_image};
use output::encode_png;
use paint::{sk_blend, sk_color, sk_rect};

pub use animated::{animation_dims, each_frame, render_apng, render_gif};
pub use effects::matrix;
pub use image::{image_crop, image_scale, raster_size, svg_size};
pub use output::{Cell, Encoded, Format, contact_sheet, encode, render_pdf};
pub use text_extras::{curve_sagitta, leader_clashes};

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
            draw_scene(
                canvas,
                scene,
                size,
                1.0,
                assets_dir,
                false,
                HashMap::new(),
                false,
            )
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
    render_image_with(scene, size, px, assets_dir, hide_text, HashMap::new())
}

/// Like [`render_image`], with images already decoded by key (a video's
/// frame for this moment, drawn by an image layer whose asset is the key).
///
/// # Errors
/// Missing assets.
pub fn render_image_with(
    scene: &Scene,
    size: &Size,
    px: f32,
    assets_dir: &Path,
    hide_text: bool,
    seeded: HashMap<String, Image>,
) -> Result<Image> {
    let (w, h) = pixel_size(size, px);
    let mut surface =
        surfaces::raster_n32_premul((w, h)).ok_or_else(|| anyhow!("can't allocate {w}×{h}"))?;
    draw_scene(
        surface.canvas(),
        scene,
        size,
        px,
        assets_dir,
        hide_text,
        seeded,
        false,
    )?;
    Ok(surface.image_snapshot())
}

/// Draws the scene onto any canvas, CPU, GPU or a PDF page (`pdf`).
#[expect(
    clippy::too_many_arguments,
    reason = "one draw's settings, passed through"
)]
fn draw_scene(
    canvas: &skia_safe::Canvas,
    scene: &Scene,
    size: &Size,
    px: f32,
    assets_dir: &Path,
    hide_text: bool,
    seeded: HashMap<String, Image>,
    pdf: bool,
) -> Result<()> {
    canvas.clear(sk_color(scene.background));
    canvas.scale((px, px));
    let mut mask_layers = HashSet::new();
    scene.walk(&mut |l| {
        if let Some(Mask {
            source: MaskSource::Layer(id),
            ..
        }) = &l.mask
        {
            mask_layers.insert(id.clone());
        }
    });
    let mut ctx = Ctx {
        scene,
        px,
        assets_dir,
        hide_text,
        rasters: seeded,
        mask_layers,
        drawing_mask: false,
        pdf,
    };
    let scene = &*scene.for_size(size);
    let placed = layout(scene, size);
    for p in &placed {
        ctx.draw(canvas, p, &placed)?;
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
    /// Ids of layers other layers use as masks: drawn only as masks.
    mask_layers: HashSet<String>,
    /// True while drawing a mask layer into another layer's mask.
    drawing_mask: bool,
    /// Drawing a PDF page: images go in as images, not image shaders,
    /// which Skia's PDF backend misplaces inside a mask's layer.
    pdf: bool,
}

impl Ctx<'_> {
    /// Draws one placed layer and its children; `root` is the whole tree.
    fn draw(&mut self, canvas: &skia_safe::Canvas, p: &Placed, root: &[Placed]) -> Result<()> {
        let l = p.layer;
        if self.mask_layers.contains(&l.id) && !self.drawing_mask {
            return Ok(());
        }
        let blur = l.look.blur * p.k;
        // A frame whose text knocks out must be its own group, so only the
        // frame is erased, not what's behind it.
        let knocked = p
            .children
            .iter()
            .any(|c| matches!(&c.layer.kind, Kind::Text { more, .. } if more.knockout));
        if l.opacity < 1.0
            || l.blend_mode != BlendMode::Normal
            || l.mask.is_some()
            || blur > 0.0
            || knocked
        {
            // Composite the whole layer (children included) as one.
            let mut layer = Paint::default();
            layer.set_alpha_f(l.opacity);
            layer.set_blend_mode(sk_blend(l.blend_mode));
            if blur > 0.0 {
                layer.set_image_filter(skia_safe::image_filters::blur(
                    (effects::sigma(blur), effects::sigma(blur)),
                    None,
                    None,
                    None,
                ));
            }
            canvas.save_layer(&SaveLayerRec::default().paint(&layer));
        } else {
            canvas.save();
        }
        effects::transform(canvas, p);
        if let Some(e) = &l.look.edges {
            canvas.clip_path(
                &rough::torn(sk_rect(p.rect), e, p.k),
                ClipOp::Intersect,
                true,
            );
        }
        let shape = shape::shape_of(p);
        if let (Some(sh), true) = (&shape, l.look.backdrop_blur > 0.0) {
            effects::backdrop_blur(canvas, sh, l.look.backdrop_blur * p.k);
        }
        let shadows = l.look.shadows.as_ref().map_or(&[][..], OneOrMany::as_slice);
        for s in shadows.iter().filter(|s| !s.inset) {
            match &shape {
                // Shapes cast CSS box shadows; images, text and icons cast
                // the shadow of their own alpha (a cutout's outline).
                Some(sh) if !matches!(l.kind, Kind::Image { .. } | Kind::Line { .. }) => {
                    effects::shape_shadow(canvas, sh, s, p.k);
                }
                _ => {
                    if let Some(paint) = effects::alpha_shadow_paint(s, p.k) {
                        canvas.save_layer(&SaveLayerRec::default().paint(&paint));
                        self.draw_own(canvas, p, shape.as_ref())?;
                        canvas.restore();
                    }
                }
            }
        }
        self.draw_own(canvas, p, shape.as_ref())?;
        if let Some(sh) = &shape {
            for s in shadows.iter().filter(|s| s.inset) {
                effects::inset_shadow(canvas, sh, s, p.k);
            }
        }
        match &l.kind {
            Kind::Frame { clip, .. } => {
                canvas.save();
                if let (true, Some(sh)) = (*clip, &shape) {
                    sh.clip(canvas, ClipOp::Intersect);
                }
                for child in &p.children {
                    self.draw(canvas, child, root)?;
                }
                canvas.restore();
            }
            Kind::FirstFit { .. } => {
                for child in &p.children {
                    self.draw(canvas, child, root)?;
                }
            }
            _ => {}
        }
        // As in design tools, a frame's stroke sits above its children, unclipped.
        if let Some(sh) = &shape {
            for st in strokes_of(l) {
                stroke::draw_stroke(canvas, sh, &st, p.k, crate::anim::drawn(l));
            }
        }
        if let Some(m) = &l.mask {
            self.apply_mask(canvas, p, m, root)?;
        }
        canvas.restore();
        Ok(())
    }

    /// The layer's own content: fills (or the image, icon or text), without
    /// children, strokes or effects.
    fn draw_own(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        shape: Option<&shape::Shape>,
    ) -> Result<()> {
        let l = p.layer;
        let r = sk_rect(p.rect);
        match &l.kind {
            // The backdrop for contrast checks: no letters, but their
            // highlights, which the letters sit on.
            Kind::Text { .. } if self.hide_text => {
                if let (Some((para, _)), Some(t)) = (&p.text, crate::text::Text::drawn(l, p.k)) {
                    text_extras::highlights(canvas, &t, para, p.text_origin(), p.k, l);
                }
            }
            Kind::Text { .. } => match l.time.moment {
                Some(moment) if l.time.split.is_some() => self.draw_split(canvas, p, r, moment)?,
                _ => self.draw_text(canvas, p, r)?,
            },
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
                let d = image_rect(p.rect, aspect, 1.0, Fit::Contain, None, 1.0, CENTER);
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
            Kind::Image {
                asset,
                fit,
                crop,
                tile_scale,
                focus,
                adjust,
            } => {
                if self.pdf && adjust.halftone <= 0.0 && *fit != Fit::Tile {
                    let (img, drawn) = self.image_drawn(
                        asset,
                        p.rect,
                        *fit,
                        crop.as_ref(),
                        tile_scale * p.k,
                        *focus,
                    )?;
                    let mut paint = Paint::default();
                    paint.set_anti_alias(true);
                    if let Some(cf) = fills::adjust_filter(adjust) {
                        paint.set_color_filter(cf);
                    }
                    canvas.save();
                    match shape {
                        Some(shape::Shape::Rect(rr)) if !rr.is_rect() => {
                            canvas.clip_rrect(rr, ClipOp::Intersect, true);
                        }
                        _ => {
                            canvas.clip_rect(r, ClipOp::Intersect, true);
                        }
                    }
                    canvas.draw_image_rect_with_sampling_options(
                        &img,
                        None,
                        sk_rect(drawn),
                        skia_safe::SamplingOptions::new(
                            skia_safe::FilterMode::Linear,
                            skia_safe::MipmapMode::Linear,
                        ),
                        &paint,
                    );
                    canvas.restore();
                    if let (Some(fs), Some(sh)) = (&l.look.fills, shape) {
                        for f in fs.as_slice() {
                            if let Some(fp) = self.fill(f, p.rect, p.k)? {
                                sh.fill(canvas, &fp);
                            }
                        }
                    }
                    return Ok(());
                }
                let mut paint =
                    self.image_paint(asset, p.rect, *fit, crop.as_ref(), tile_scale * p.k, *focus)?;
                if adjust.halftone > 0.0 {
                    effects::halftone(&mut paint, adjust.halftone * p.k, (r.left, r.top));
                }
                if let Some(cf) = fills::adjust_filter(adjust) {
                    paint.set_color_filter(cf);
                }
                match shape {
                    Some(shape::Shape::Rect(rr)) if !rr.is_rect() => {
                        shape::Shape::Rect(*rr).fill(canvas, &paint);
                    }
                    _ => {
                        canvas.draw_rect(r, &paint);
                    }
                }
                if let (Some(fs), Some(sh)) = (&l.look.fills, shape) {
                    for f in fs.as_slice() {
                        if let Some(fp) = self.fill(f, p.rect, p.k)? {
                            sh.fill(canvas, &fp);
                        }
                    }
                }
            }
            _ => {
                if let Some(sh) = shape {
                    for f in fills_of(l) {
                        if let Some(fp) = self.fill(&f, p.rect, p.k)? {
                            sh.fill(canvas, &fp);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// A shape's fills: `fills`, else its short-form `color`/`gradient` (the gradient wins).
fn fills_of(l: &Layer) -> Vec<crate::scene::Paint> {
    if let Some(fs) = &l.look.fills {
        return fs.as_slice().to_vec();
    }
    let (color, gradient) = match &l.kind {
        Kind::Rect {
            color, gradient, ..
        }
        | Kind::Ellipse {
            color, gradient, ..
        }
        | Kind::Frame {
            color, gradient, ..
        }
        | Kind::Polygon {
            color, gradient, ..
        }
        | Kind::Path {
            color, gradient, ..
        } => (*color, gradient.as_ref()),
        _ => (None, None),
    };
    match (gradient, color) {
        (Some(g), _) => vec![crate::scene::Paint::Gradient(crate::scene::GradientFill {
            gradient: g.clone(),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        })],
        (None, Some(c)) => vec![crate::scene::Paint::color(c)],
        (None, None) => Vec::new(),
    }
}

/// A shape's strokes: `strokes`, else its short-form `stroke` (a line's color and width).
fn strokes_of(l: &Layer) -> Vec<Cow<'_, Stroke>> {
    if let Some(ss) = &l.look.strokes {
        return ss.as_slice().iter().map(Cow::Borrowed).collect();
    }
    match &l.kind {
        Kind::Rect { stroke, .. }
        | Kind::Ellipse { stroke, .. }
        | Kind::Frame { stroke, .. }
        | Kind::Polygon { stroke, .. }
        | Kind::Path { stroke, .. } => stroke.iter().map(Cow::Borrowed).collect(),
        Kind::Line {
            color,
            stroke_width,
        } => vec![Cow::Owned(Stroke::solid(*stroke_width, *color))],
        _ => Vec::new(),
    }
}
