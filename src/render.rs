//! Draws a laid-out scene with Skia on the CPU and encodes it as PNG.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    AlphaType, ClipOp, ColorType, Data, EncodedImageFormat, FilterMode, Image, ImageInfo, Matrix,
    MipmapMode, Paint, PaintStyle, RRect, SamplingOptions, TileMode, canvas::SaveLayerRec,
    gradient, images, surfaces,
};

use crate::gpu::{self, Backend};
use crate::layout::{Placed, Rect, layout};
use crate::scene::{
    Asset, BlendMode, Color, Crop, Fit, Gradient, Kind, Scene, Size, Stroke, StrokeAlign,
};
use crate::text::Text;

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

fn encode_png(image: &Image) -> Result<Vec<u8>> {
    let png = image
        .encode(None, EncodedImageFormat::PNG, None)
        .ok_or_else(|| anyhow!("PNG encoding failed"))?;
    Ok(png.as_bytes().to_vec())
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
            } => {
                if let (Some((para, fit)), Some(t)) = (&p.text, Text::of(l, p.k)) {
                    let origin = (p.rect.x, p.text_top());
                    // The letters show an image or gradient laid over the text's box.
                    let custom = match (fill, gradient) {
                        (Some(f), _) => Some(self.image_paint(
                            &f.asset,
                            p.rect,
                            f.fit,
                            None,
                            f.tile_scale * p.k,
                            CENTER,
                        )?),
                        (None, Some(g)) => fill_paint(None, Some(g), r),
                        (None, None) => None,
                    };
                    match custom {
                        Some(paint) => t.repaint(fit, p.rect.w, &paint, true).paint(canvas, origin),
                        None => para.paint(canvas, origin),
                    }
                    if let Some(o) = outline {
                        let mut stroke = Paint::default();
                        stroke.set_anti_alias(true);
                        stroke.set_style(PaintStyle::Stroke);
                        stroke.set_stroke_join(skia_safe::PaintJoin::Round);
                        stroke.set_stroke_width(o.width * p.k * t.shrink(fit));
                        stroke.set_color(sk_color(o.color));
                        // Stroke the glyphs' merged outline: variable fonts
                        // draw letters from overlapping contours, whose inner
                        // edges would otherwise show as seams.
                        let mut para = t.repaint(fit, p.rect.w, &Paint::default(), false);
                        let merged = (0..para.line_number())
                            .map(|line| para.get_path_at(line).1)
                            .map(|path| path.simplify().unwrap_or(path))
                            .reduce(|all, line| {
                                all.op(&line, skia_safe::PathOp::Union).unwrap_or(all)
                            })
                            .unwrap_or_default();
                        canvas.save();
                        canvas.translate(origin);
                        canvas.draw_path(&merged, &stroke);
                        canvas.restore();
                    }
                }
            }
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

    /// A paint showing image `id` placed over `bx` (cover, contain, crop, or
    /// repeated tiles `tile` times the image's size); transparent elsewhere.
    fn image_paint(
        &mut self,
        id: &str,
        bx: Rect,
        fit: Fit,
        crop: Option<&Crop>,
        tile: f32,
        focus: [f32; 2],
    ) -> Result<Paint> {
        let a = self
            .scene
            .assets
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown asset {id}"))?;
        let drawn = image_rect(bx, a.width, a.height, fit, crop, tile, focus);
        let img = self.image(id, &a, drawn)?;
        let src = skia_safe::Rect::from_wh(img.width() as f32, img.height() as f32);
        let to_drawn = Matrix::rect_2_rect(src, sk_rect(drawn), None)
            .ok_or_else(|| anyhow!("asset {id}: empty image"))?;
        let mode = if fit == Fit::Tile && crop.is_none() {
            TileMode::Repeat
        } else {
            TileMode::Decal
        };
        let shader = img
            .to_shader(
                (mode, mode),
                SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
                &to_drawn,
            )
            .ok_or_else(|| anyhow!("asset {id}: can't make a shader"))?;
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_shader(shader);
        Ok(paint)
    }

    fn image(&mut self, id: &str, a: &Asset, drawn: Rect) -> Result<Image> {
        let bytes = || {
            std::fs::read(self.assets_dir.join(&a.sha256))
                .with_context(|| format!("asset {id} missing from store"))
        };
        if a.svg {
            // Rasterized at its on-screen pixel size so icons stay sharp.
            let pw = (drawn.w * self.px).ceil().clamp(1.0, MAX_SVG_PX) as u32;
            let ph = (drawn.h * self.px).ceil().clamp(1.0, MAX_SVG_PX) as u32;
            return svg_image(&bytes()?, pw, ph).with_context(|| format!("asset {id}"));
        }
        if let Some(img) = self.rasters.get(id) {
            return Ok(img.clone());
        }
        let img = Image::from_encoded(Data::new_copy(&bytes()?))
            .ok_or_else(|| anyhow!("asset {id}: can't decode image"))?;
        self.rasters.insert(id.to_owned(), img.clone());
        Ok(img)
    }
}

const CENTER: [f32; 2] = [0.5, 0.5];

/// Where the whole `iw`×`ih` image lands for `fit`, centered on `bx`; with a
/// `crop`, so that the cropped part exactly covers `bx`; for `tile`, the
/// first tile at `bx`'s corner, `tile` times the image's size. Anything
/// outside `bx` is clipped. `focus` (0–1 per axis) is the point of the
/// image kept in view when it overflows or letterboxes; center is `CENTER`.
fn image_rect(
    bx: Rect,
    iw: f32,
    ih: f32,
    fit: Fit,
    crop: Option<&Crop>,
    tile: f32,
    focus: [f32; 2],
) -> Rect {
    if let Some(c) = crop {
        let (w, h) = (bx.w / c.width, bx.h / c.height);
        return Rect {
            x: bx.x - c.x * w,
            y: bx.y - c.y * h,
            w,
            h,
        };
    }
    let s = match fit {
        Fit::Fill => (bx.w / iw).max(bx.h / ih),
        Fit::Fit => (bx.w / iw).min(bx.h / ih),
        Fit::Tile => {
            return Rect {
                x: bx.x,
                y: bx.y,
                w: iw * tile,
                h: ih * tile,
            };
        }
    };
    let (w, h) = (iw * s, ih * s);
    Rect {
        x: bx.x + (bx.w - w) * focus[0],
        y: bx.y + (bx.h - h) * focus[1],
        w,
        h,
    }
}

/// How much an image is enlarged on screen relative to its pixels
/// (`> 1` means upscaled, so blurry). SVGs never are; callers skip them.
pub fn image_scale(bx: Rect, iw: f32, ih: f32, fit: Fit, crop: Option<&Crop>, tile: f32) -> f32 {
    image_rect(bx, iw, ih, fit, crop, tile, CENTER).w / iw
}

/// Crop reported by `scene_describe`: the fraction of the image hidden on
/// each axis (none for tiles, which repeat instead).
pub fn image_crop(bx: Rect, iw: f32, ih: f32, fit: Fit, crop: Option<&Crop>) -> (f32, f32) {
    if fit == Fit::Tile && crop.is_none() {
        return (0.0, 0.0);
    }
    let d = image_rect(bx, iw, ih, fit, crop, 1.0, CENTER);
    (1.0 - (bx.w / d.w).min(1.0), 1.0 - (bx.h / d.h).min(1.0))
}

/// Reads an SVG's intrinsic size, which doubles as validation on upload.
pub fn svg_size(bytes: &[u8]) -> Result<(f32, f32)> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())?;
    let s = tree.size();
    Ok((s.width(), s.height()))
}

/// Reads a raster image's size, which doubles as validation on upload.
pub fn raster_size(bytes: &[u8]) -> Option<(f32, f32)> {
    let img = Image::from_encoded(Data::new_copy(bytes))?;
    Some((img.width() as f32, img.height() as f32))
}

fn svg_image(bytes: &[u8], pw: u32, ph: u32) -> Result<Image> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())?;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(pw, ph).ok_or_else(|| anyhow!("bad SVG raster size"))?;
    let s = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(pw as f32 / s.width(), ph as f32 / s.height());
    resvg::render(&tree, t, &mut pixmap.as_mut());
    let info = ImageInfo::new(
        (pw as i32, ph as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    images::raster_from_data(&info, Data::new_copy(pixmap.data()), pw as usize * 4)
        .ok_or_else(|| anyhow!("can't wrap SVG raster"))
}

/// Paint for a solid color or a gradient (the gradient wins); `None` when
/// the shape has no fill.
fn fill_paint(
    color: Option<Color>,
    gradient: Option<&Gradient>,
    r: skia_safe::Rect,
) -> Option<Paint> {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    match (gradient, color) {
        (Some(g), _) => {
            let at = |[x, y]: [f32; 2]| (r.left + x * r.width(), r.top + y * r.height());
            let colors: Vec<skia_safe::Color4f> =
                g.stops.iter().map(|s| sk_color(s.color).into()).collect();
            let pos: Vec<f32> = g.stops.iter().map(|s| s.at).collect();
            let spec = gradient::Gradient::new(
                gradient::Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                gradient::Interpolation::default(),
            );
            p.set_shader(gradient::shaders::linear_gradient(
                (at(g.from), at(g.to)),
                &spec,
                None,
            ));
        }
        (None, Some(c)) => {
            p.set_color(sk_color(c));
        }
        (None, None) => return None,
    }
    Some(p)
}

/// Strokes the box edge; `inside` and `outside` shift the path by half the
/// width so the stroke lies wholly on one side, as design tools draw it.
fn draw_stroke(canvas: &skia_safe::Canvas, s: &Stroke, r: skia_safe::Rect, radius: f32, k: f32) {
    let width = s.width * k;
    let shift = match s.align {
        StrokeAlign::Inside => -width / 2.0,
        StrokeAlign::Center => 0.0,
        StrokeAlign::Outside => width / 2.0,
    };
    let edge = r.with_outset((shift, shift));
    let color = s
        .color
        .or((s.gradient.is_none()).then_some(Color(0xFF00_0000)));
    if let Some(mut p) = fill_paint(color, s.gradient.as_ref(), edge) {
        p.set_style(PaintStyle::Stroke);
        p.set_stroke_width(width);
        canvas.draw_rrect(rrect(edge, (radius + shift).max(0.0)), &p);
    }
}

fn sk_blend(m: BlendMode) -> skia_safe::BlendMode {
    use skia_safe::BlendMode as B;
    match m {
        BlendMode::Normal => B::SrcOver,
        BlendMode::Multiply => B::Multiply,
        BlendMode::Screen => B::Screen,
        BlendMode::Overlay => B::Overlay,
        BlendMode::Darken => B::Darken,
        BlendMode::Lighten => B::Lighten,
        BlendMode::ColorDodge => B::ColorDodge,
        BlendMode::ColorBurn => B::ColorBurn,
        BlendMode::HardLight => B::HardLight,
        BlendMode::SoftLight => B::SoftLight,
        BlendMode::Difference => B::Difference,
        BlendMode::Exclusion => B::Exclusion,
        BlendMode::Hue => B::Hue,
        BlendMode::Saturation => B::Saturation,
        BlendMode::Color => B::Color,
        BlendMode::Luminosity => B::Luminosity,
    }
}

fn sk_color(c: Color) -> skia_safe::Color {
    skia_safe::Color::new(c.0)
}

fn sk_rect(r: Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(r.x, r.y, r.w, r.h)
}

fn rrect(r: skia_safe::Rect, radius: f32) -> RRect {
    RRect::new_rect_xy(r, radius, radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn fill_covers_and_fit_letterboxes() {
        // 400×200 image into a 100×100 box.
        assert_eq!(
            image_rect(
                r(0.0, 0.0, 100.0, 100.0),
                400.0,
                200.0,
                Fit::Fill,
                None,
                1.0,
                CENTER
            ),
            r(-50.0, 0.0, 200.0, 100.0)
        );
        assert_eq!(
            image_rect(
                r(0.0, 0.0, 100.0, 100.0),
                400.0,
                200.0,
                Fit::Fit,
                None,
                1.0,
                CENTER
            ),
            r(0.0, 25.0, 100.0, 50.0)
        );
        assert_eq!(
            image_crop(r(0.0, 0.0, 100.0, 100.0), 400.0, 200.0, Fit::Fill, None),
            (0.5, 0.0)
        );
    }

    #[test]
    fn crop_stretches_the_chosen_region_over_the_box() {
        // The right half of a 400×200 image fills a 100×100 box.
        let c = Crop {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        let d = image_rect(
            r(10.0, 10.0, 100.0, 100.0),
            400.0,
            200.0,
            Fit::Fill,
            Some(&c),
            1.0,
            CENTER,
        );
        assert_eq!(d, r(-90.0, 10.0, 200.0, 100.0));
        assert_eq!(
            image_crop(
                r(10.0, 10.0, 100.0, 100.0),
                400.0,
                200.0,
                Fit::Fill,
                Some(&c)
            ),
            (0.5, 0.0)
        );
    }

    #[test]
    fn renders_deterministic_png_of_the_right_size() {
        let scene: Scene = serde_json::from_value(json!({
            "width": 200, "height": 100, "background": "#EEEEEE",
            "sizes": [{"id": "a", "width": 200, "height": 100}],
            "layers": [
                {"id": "r", "type": "rect", "x": 10, "y": 10, "width": 50, "height": 50, "color": "#D0202E", "cornerRadius": 8},
                {"id": "t", "type": "text", "text": "Hi", "x": 80, "y": 20, "fontSize": 30, "weight": 800}
            ]
        }))
        .unwrap();
        let dir = std::env::temp_dir();
        let a = render_png(&scene, &scene.sizes[0], 1.0, &dir).unwrap();
        let b = render_png(&scene, &scene.sizes[0], 1.0, &dir).unwrap();
        assert_eq!(a, b);
        assert_eq!(raster_size(&a), Some((200.0, 100.0)));
        let preview = render_png(&scene, &scene.sizes[0], 0.5, &dir).unwrap();
        assert_eq!(raster_size(&preview), Some((100.0, 50.0)));
    }

    #[test]
    fn svg_size_reads_viewbox() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="40"><rect width="50" height="40"/></svg>"#;
        assert_eq!(svg_size(svg).unwrap(), (50.0, 40.0));
        assert!(svg_size(b"not svg").is_err());
    }

    fn pixels(layers: serde_json::Value) -> impl Fn(i32, i32) -> (u8, u8, u8) {
        pixels_with(layers, &[])
    }

    /// Renders a 100×100 scene whose `assets` are `(id, PNG bytes)`.
    fn pixels_with(
        layers: serde_json::Value,
        assets: &[(&str, Vec<u8>)],
    ) -> impl Fn(i32, i32) -> (u8, u8, u8) + use<> {
        let dir = std::env::temp_dir();
        let mut registry = serde_json::Map::new();
        for (id, png) in assets {
            let sha = crate::store::sha256_hex(png);
            std::fs::write(dir.join(&sha), png).unwrap();
            let (w, h) = raster_size(png).unwrap();
            registry.insert(
                (*id).to_owned(),
                json!({"sha256": sha, "width": w, "height": h}),
            );
        }
        let mut v = json!({
            "width": 100, "height": 100, "background": "#FFFFFF",
            "sizes": [{"id": "a", "width": 100, "height": 100}],
        });
        v["layers"] = layers;
        v["assets"] = serde_json::Value::Object(registry);
        let scene: Scene = serde_json::from_value(v).unwrap();
        let img = render_image(&scene, &scene.sizes[0], 1.0, &dir, false).unwrap();
        move |x, y| {
            let c = img.peek_pixels().unwrap().get_color((x, y));
            (c.r(), c.g(), c.b())
        }
    }

    /// A `w`×`h` PNG, left half `left`, right half `right`.
    fn two_tone(w: i32, h: i32, left: skia_safe::Color, right: skia_safe::Color) -> Vec<u8> {
        let mut surface = surfaces::raster_n32_premul((w, h)).unwrap();
        let c = surface.canvas();
        let mut p = Paint::default();
        p.set_color(left);
        c.draw_rect(
            skia_safe::Rect::from_xywh(0.0, 0.0, w as f32 / 2.0, h as f32),
            &p,
        );
        p.set_color(right);
        c.draw_rect(
            skia_safe::Rect::from_xywh(w as f32 / 2.0, 0.0, w as f32 / 2.0, h as f32),
            &p,
        );
        encode_png(&surface.image_snapshot()).unwrap()
    }

    /// How many pixels of the 100×100 render satisfy `f`.
    fn count(px: &impl Fn(i32, i32) -> (u8, u8, u8), f: impl Fn((u8, u8, u8)) -> bool) -> usize {
        (0..100)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .filter(|&(x, y)| f(px(x, y)))
            .count()
    }

    const RED: (u8, u8, u8) = (255, 0, 0);
    const BLUE: (u8, u8, u8) = (0, 0, 255);

    #[test]
    fn tiles_repeat_at_the_image_size_times_tile_scale() {
        let tile = two_tone(10, 10, skia_safe::Color::RED, skia_safe::Color::BLUE);
        let px = pixels_with(
            json!([{"type": "image", "asset": "t", "width": 100, "height": 100, "fit": "tile"}]),
            &[("t", tile.clone())],
        );
        assert_eq!(
            [px(2, 5), px(7, 5), px(12, 5), px(17, 5), px(92, 95)],
            [RED, BLUE, RED, BLUE, RED]
        );
        let px = pixels_with(
            json!([{"type": "image", "asset": "t", "width": 100, "height": 100, "fit": "tile", "tileScale": 2}]),
            &[("t", tile)],
        );
        assert_eq!([px(5, 5), px(15, 5), px(25, 5)], [RED, BLUE, RED]);
    }

    #[test]
    fn ellipses_lines_and_icons_draw_where_their_box_says() {
        let px = pixels(json!([
            {"type": "ellipse", "width": 100, "height": 100, "color": "#FF0000"},
        ]));
        // Inside the circle, not its bounding box's corner.
        assert_eq!((px(50, 50), px(3, 3)), (RED, (255, 255, 255)));
        let px = pixels(json!([
            {"type": "line", "y": 50, "width": 100, "color": "#0000FF", "strokeWidth": 4},
        ]));
        assert_eq!((px(50, 50), px(50, 45)), (BLUE, (255, 255, 255)));
        // A solid icon fills with its color; its box is contained, centered.
        let px = pixels(json!([
            {"type": "icon", "name": "square", "set": "solid", "color": "#FF0000", "width": 100, "height": 100},
        ]));
        assert_eq!(px(50, 50), RED);
        let red = count(&px, |c| c == RED);
        assert!(red > 5000, "{red}");
    }

    #[test]
    fn masks_fade_a_layer_and_focus_picks_what_a_crop_keeps() {
        let px = pixels(json!([
            {"type": "rect", "width": 100, "height": 100, "color": "#0000FF",
             "mask": {"from": [0.5, 0], "to": [0.5, 1], "stops": [{"at": 0, "color": "#00000000"}, {"at": 1, "color": "#000000"}]}},
        ]));
        // Transparent at the top, opaque at the bottom, half-way between.
        // (Pixel centers sit half a pixel into the gradient, so ±1.)
        let near = |(r, g, b): (u8, u8, u8), (x, y, z): (u8, u8, u8)| {
            r.abs_diff(x) <= 1 && g.abs_diff(y) <= 1 && b.abs_diff(z) <= 1
        };
        assert!(near(px(50, 0), (255, 255, 255)) && near(px(50, 99), BLUE));
        let (r, _, b) = px(50, 50);
        assert!(r > 100 && r < 160 && b == 255, "{:?}", px(50, 50));
        // A 20×10 image, left red, right blue, cover-cropped to a square:
        // the focus decides which half stays.
        let img = two_tone(20, 10, skia_safe::Color::RED, skia_safe::Color::BLUE);
        let layer = |focus: [f32; 2]| json!([{"type": "image", "asset": "i", "width": 100, "height": 100, "focus": focus}]);
        let left = pixels_with(layer([0.0, 0.5]), &[("i", img.clone())]);
        let right = pixels_with(layer([1.0, 0.5]), &[("i", img)]);
        assert_eq!((left(50, 50), right(50, 50)), (RED, BLUE));
    }

    #[test]
    fn text_can_be_filled_with_an_image_or_a_gradient() {
        let text = |extra: serde_json::Value| {
            let mut t = json!({"type": "text", "text": "III", "fontSize": 90, "weight": 900, "color": "#00FF00"});
            t.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            json!([t])
        };
        let image = two_tone(20, 20, skia_safe::Color::RED, skia_safe::Color::BLUE);
        let px = pixels_with(text(json!({"fill": {"asset": "img"}})), &[("img", image)]);
        // The letters show the image's red and blue, never the text color.
        assert!(count(&px, |c| c == RED) > 50 && count(&px, |c| c == BLUE) > 50);
        assert_eq!(count(&px, |c| c.1 > 200 && c.0 < 50), 0);

        let px = pixels(text(
            json!({"gradient": {"stops": [{"at": 0, "color": "#FF0000"}, {"at": 1, "color": "#0000FF"}]}}),
        ));
        assert!(
            count(&px, |c| c.0 > 200 && c.2 < 60) > 20
                && count(&px, |c| c.2 > 200 && c.0 < 60) > 20
        );
    }

    #[test]
    fn outlined_text_can_be_hollow() {
        let px = pixels(
            json!([{"type": "text", "text": "III", "fontSize": 90, "weight": 900,
            "color": "#00000000", "outline": {"width": 4, "color": "#FF0000"}}]),
        );
        assert!(count(&px, |c| c == RED) > 50, "outline drawn");
        assert_eq!(
            count(&px, |c| c.0 < 100 && c.1 < 100 && c.2 < 100),
            0,
            "no dark fill"
        );
        let filled =
            pixels(json!([{"type": "text", "text": "III", "fontSize": 90, "weight": 900}]));
        assert!(count(&filled, |c| c == RED) == 0 && count(&filled, |c| c.0 < 30) > 200);
    }

    #[test]
    fn gradients_run_between_their_points() {
        let px = pixels(
            json!([{"type": "rect", "width": 100, "height": 100, "gradient":
            {"from": [0, 0.5], "to": [1, 0.5], "stops": [{"at": 0, "color": "#000000"}, {"at": 1, "color": "#FFFFFF"}]}}]),
        );
        assert!(px(2, 50).0 < 20 && px(97, 50).0 > 235 && (px(50, 50).0 as i32 - 128).abs() < 10);
        let px = pixels(
            json!([{"type": "rect", "width": 100, "height": 100, "gradient":
            {"from": [0.5, 0], "to": [0.5, 1], "stops": [{"at": 0, "color": "#FF0000"}, {"at": 1, "color": "#0000FF"}]}}]),
        );
        assert!(px(50, 2).0 > 240 && px(50, 97).2 > 240, "top to bottom");
    }

    #[test]
    fn strokes_sit_inside_center_or_outside_the_edge() {
        let stroke = |align: &str| {
            pixels(
                json!([{"type": "rect", "x": 30, "y": 30, "width": 40, "height": 40,
                "stroke": {"width": 6, "color": "#FF0000", "align": align}}]),
            )
        };
        let red = (255, 0, 0);
        let inside = stroke("inside");
        assert_eq!((inside(32, 50), inside(28, 50)), (red, (255, 255, 255)));
        let outside = stroke("outside");
        assert_eq!((outside(28, 50), outside(32, 50)), (red, (255, 255, 255)));
        let center = stroke("center");
        assert_eq!((center(28, 50), center(32, 50)), (red, red));
    }

    #[test]
    fn rotation_turns_the_layer_and_its_children_about_the_center() {
        // A 60×10 bar through the middle, turned 90°, stands upright.
        let px = pixels(
            json!([{"type": "frame", "x": 20, "y": 45, "width": 60, "height": 10, "rotation": 90,
            "children": [{"type": "rect", "width": 60, "height": 10, "color": "#000000"}]}]),
        );
        assert_eq!(px(50, 25), (0, 0, 0));
        assert_eq!(px(25, 50), (255, 255, 255));
    }

    #[test]
    fn blend_modes_composite_with_what_is_below() {
        let px = pixels(json!([
            {"type": "rect", "width": 100, "height": 100, "color": "#FF0000"},
            {"type": "rect", "width": 50, "height": 100, "color": "#00FF00", "blendMode": "multiply"}
        ]));
        assert_eq!(px(25, 50), (0, 0, 0), "red × green = black");
        assert_eq!(px(75, 50), (255, 0, 0));
    }

    #[test]
    fn gpu_renders_match_the_cpu_closely() {
        if !gpu::available() {
            eprintln!("no GPU on this machine; the CPU fallback covers renders");
            return;
        }
        let scene: Scene = serde_json::from_value(json!({
            "width": 200, "height": 100, "background": "#EEEEEE",
            "sizes": [{"id": "a", "width": 200, "height": 100}],
            "layers": [
                {"id": "r", "type": "rect", "x": 10, "y": 10, "width": 80, "height": 80, "cornerRadius": 12,
                 "gradient": {"stops": [{"at": 0, "color": "#D0202E"}, {"at": 1, "color": "#1B2A5C"}]}, "rotation": 10},
                {"id": "t", "type": "text", "text": "GPU", "x": 110, "y": 20, "fontSize": 40, "weight": 800,
                 "outline": {"width": 2, "color": "#000000"}, "color": "#FFFFFF"}
            ]
        }))
        .unwrap();
        let dir = std::env::temp_dir();
        let cpu = render_png_on(&scene, &scene.sizes[0], &dir, Backend::Cpu).unwrap();
        let gpu = render_png_on(&scene, &scene.sizes[0], &dir, Backend::Gpu).unwrap();
        assert_eq!(raster_size(&gpu), Some((200.0, 100.0)));
        let decode = |png: &[u8]| {
            Image::from_encoded(Data::new_copy(png))
                .and_then(|i| i.make_raster_image(None, None))
                .unwrap()
        };
        let (a, b) = (decode(&cpu), decode(&gpu));
        let (pa, pb) = (a.peek_pixels().unwrap(), b.peek_pixels().unwrap());
        // Same drawing, different rasterizer: edges antialias differently,
        // so compare the average difference, not a count of pixels.
        let mut total = 0u64;
        for y in 0..100 {
            for x in 0..200 {
                let (ca, cb) = (pa.get_color((x, y)), pb.get_color((x, y)));
                total += u64::from(ca.r().abs_diff(cb.r()))
                    + u64::from(ca.g().abs_diff(cb.g()))
                    + u64::from(ca.b().abs_diff(cb.b()));
            }
        }
        let mean = total as f64 / (200.0 * 100.0 * 3.0);
        assert!(
            mean < 2.0,
            "GPU and CPU differ by {mean:.2} levels per channel on average"
        );
    }
}
