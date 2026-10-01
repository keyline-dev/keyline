//! Encoded output: PNG, JPEG, WebP or PDF files, a file-size target, and
//! the preview contact sheet.

use std::path::Path;

use anyhow::{Result, anyhow};
use skia_safe::{EncodedImageFormat, Image, surfaces};

use super::{draw_scene, render_image};
use crate::scene::{Scene, Size};

/// One cell of a preview sheet: a size of a scene, at rest or at a moment.
pub struct Cell<'a> {
    /// The scene (a row's variant).
    pub scene: &'a Scene,
    /// The size drawn.
    pub size: &'a Size,
    /// The moment, seconds; at rest when `None`.
    pub at: Option<f32>,
}

/// One preview PNG, to save image tokens: rows of cells, each scaled to
/// `height` px tall, a row shrunk further to stay within
/// [`SHEET_MAX_WIDTH`].
///
/// # Errors
/// Missing assets, or a clip's frames can't be decoded.
pub fn contact_sheet(rows: &[Vec<Cell>], height: f32, assets_dir: &Path) -> Result<Vec<u8>> {
    const GAP: f32 = 8.0;
    let rows: Vec<Vec<Image>> = rows
        .iter()
        .map(|row| {
            let tall = |c: &Cell| (height / c.size.height).min(1.0);
            let wide: f32 = row.iter().map(|c| c.size.width * tall(c)).sum();
            let room = SHEET_MAX_WIDTH - GAP * (row.len() as f32 + 1.0);
            let fit = (room / wide).min(1.0);
            row.iter()
                .map(|c| {
                    let scene = c.scene.resolved();
                    frame(&scene, c.size, c.at, tall(c) * fit, assets_dir)
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<_>>()?;
    let gap = GAP as i32;
    let w = rows
        .iter()
        .map(|r| r.iter().map(|i| i.width() + gap).sum::<i32>() + gap)
        .max()
        .unwrap_or(1);
    let h = rows
        .iter()
        .map(|r| r.iter().map(Image::height).max().unwrap_or(0) + gap)
        .sum::<i32>()
        + gap;
    let mut surface =
        surfaces::raster_n32_premul((w, h)).ok_or_else(|| anyhow!("can't allocate {w}×{h}"))?;
    let canvas = surface.canvas();
    canvas.clear(skia_safe::Color::from_rgb(128, 128, 128));
    let mut y = GAP;
    for row in &rows {
        let mut x = GAP;
        for shot in row {
            canvas.draw_image(shot, (x, y), None);
            x += shot.width() as f32 + GAP;
        }
        y += row.iter().map(Image::height).max().unwrap_or(0) as f32 + GAP;
    }
    encode_png(&surface.image_snapshot())
}

/// The widest a preview of moments gets, px.
pub const SHEET_MAX_WIDTH: f32 = 2000.0;

/// `size` drawn at `px` per pixel, at moment `t` (at rest when `None`),
/// with any clips' frames of that moment.
fn frame(scene: &Scene, size: &Size, t: Option<f32>, px: f32, assets_dir: &Path) -> Result<Image> {
    if crate::video::frame::has_video(scene) {
        let (at, frames) = crate::video::frame::still(scene, size, t.unwrap_or(0.0), assets_dir)?;
        return super::render_image_with(&at, size, px, assets_dir, false, frames);
    }
    match t {
        Some(t) => render_image(
            &crate::anim::at_time(scene, t, size),
            size,
            px,
            assets_dir,
            false,
        ),
        None => render_image(scene, size, px, assets_dir, false),
    }
}

pub(super) fn encode_png(image: &Image) -> Result<Vec<u8>> {
    let png = image
        .encode(None, EncodedImageFormat::PNG, None)
        .ok_or_else(|| anyhow!("PNG encoding failed"))?;
    Ok(png.as_bytes().to_vec())
}

/// A rendered file's format. Its schema is a plain string: the variants'
/// docs would cost the agent tokens on every `tools/list`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// Lossless, with transparency (default).
    #[default]
    Png,
    /// Lossy, smallest for photos; `jpg` too.
    #[serde(alias = "jpg")]
    Jpeg,
    /// Lossy, smaller than JPEG at the same quality.
    Webp,
    /// Vector PDF, one page at the size in points (1 px = 1 pt; a print
    /// preset's page is its paper size).
    Pdf,
    /// Animated PNG of an animated scene: lossless, transparent, plays in
    /// browsers.
    Apng,
    /// Animated GIF of an animated scene: plays everywhere, 256 colors.
    Gif,
    /// H.264 MP4 video of an animated scene (needs ffmpeg).
    Mp4,
    /// VP9 WebM video of an animated scene (needs ffmpeg).
    Webm,
}

impl schemars::JsonSchema for Format {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Format".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type": "string"})
    }
    fn inline_schema() -> bool {
        true
    }
}

impl Format {
    /// The file extension.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
            Format::Pdf => "pdf",
            Format::Apng => "anim.png",
            Format::Gif => "gif",
            Format::Mp4 => "mp4",
            Format::Webm => "webm",
        }
    }
}

/// An encoded file and how it came out.
pub struct Encoded {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// The quality used, when `maxKB` made it lower than asked.
    pub lowered: Option<u32>,
    /// Still over `maxKB` at the lowest quality (or lossless).
    pub too_big: bool,
}

/// Encodes `image` as `format` at `quality` (0–100, lossy formats only).
/// With `max_kb`, JPEG and WebP take the highest quality that fits.
///
/// # Errors
/// The encoder fails, or `format` is PDF (see [`render_pdf`]).
pub fn encode(image: &Image, format: Format, quality: u32, max_kb: Option<u32>) -> Result<Encoded> {
    let sk = match format {
        Format::Png => EncodedImageFormat::PNG,
        Format::Jpeg => EncodedImageFormat::JPEG,
        Format::Webp => EncodedImageFormat::WEBP,
        Format::Pdf | Format::Apng | Format::Gif | Format::Mp4 | Format::Webm => {
            return Err(anyhow!("{} isn't a still format", format.ext()));
        }
    };
    let at = |q: u32| {
        image
            .encode(None, sk, q)
            .map(|d| d.as_bytes().to_vec())
            .ok_or_else(|| anyhow!("{} encoding failed", format.ext()))
    };
    let quality = quality.min(100);
    let bytes = at(quality)?;
    let limit = max_kb.map(|kb| kb as usize * 1024);
    let fits = |b: &[u8]| limit.is_none_or(|l| b.len() <= l);
    if fits(&bytes) || format == Format::Png {
        let too_big = !fits(&bytes);
        return Ok(Encoded {
            bytes,
            lowered: None,
            too_big,
        });
    }
    // Binary search for the highest quality that fits.
    let (mut lo, mut hi, mut best) = (1, quality, None);
    while lo < hi {
        let mid = u32::midpoint(lo, hi);
        let b = at(mid)?;
        if fits(&b) {
            best = Some((mid, b));
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Ok(match best {
        Some((q, bytes)) => Encoded {
            bytes,
            lowered: Some(q),
            too_big: false,
        },
        None => {
            let bytes = at(1)?;
            let too_big = !fits(&bytes);
            Encoded {
                bytes,
                lowered: Some(1),
                too_big,
            }
        }
    })
}

/// Renders `size` as a one-page vector PDF: 1 px = 1 pt, except a print
/// preset, whose page is its paper size (`a4-portrait` is A4). Blurs and
/// shaders Skia can't express in PDF are rasterized inside it. Photos are
/// embedded at most at 300 dpi of their drawn size, opaque ones as JPEG at
/// `quality` (1–100).
///
/// # Errors
/// Missing assets.
pub fn render_pdf(scene: &Scene, size: &Size, assets_dir: &Path, quality: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let pt = crate::scene::pdf_points_per_px(&size.id);
    // Opaque photos go in as JPEG at `quality`, not lossless.
    let meta = skia_safe::pdf::Metadata {
        encoding_quality: Some(i32::try_from(quality.clamp(1, 100)).unwrap_or(90)),
        ..Default::default()
    };
    let mut page = skia_safe::pdf::new_document(&mut out, Some(&meta))
        .begin_page((size.width * pt, size.height * pt), None);
    page.canvas().scale((pt, pt));
    draw_scene(
        page.canvas(),
        scene,
        size,
        1.0,
        assets_dir,
        false,
        std::collections::HashMap::new(),
        true,
    )?;
    page.end_page().close();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{Format, encode, render_pdf};

    #[test]
    fn a_pdfs_photo_is_kept_to_print_resolution_and_its_quality() {
        // A 2400 × 1600 photo of noise (it doesn't compress) drawn 300 pt
        // wide: at 300 dpi that's 1250 px, so most of its pixels go, and
        // JPEG at a lower quality takes it smaller still.
        let dir = std::env::temp_dir().join(format!("keyline-pdf-size-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Opaque, as a photo is (JPEG holds no transparency).
        let info = skia_safe::ImageInfo::new(
            (2400, 1600),
            skia_safe::ColorType::RGB888x,
            skia_safe::AlphaType::Opaque,
            None,
        );
        let mut surface = skia_safe::surfaces::raster(&info, None, None).unwrap();
        surface.canvas().clear(skia_safe::Color::WHITE);
        let mut paint = skia_safe::Paint::default();
        paint.set_shader(skia_safe::shaders::fractal_noise((0.9, 0.9), 4, 7.0, None));
        surface.canvas().draw_paint(&paint);
        let png = surface
            .image_snapshot()
            .encode(None, skia_safe::EncodedImageFormat::PNG, None)
            .unwrap();
        std::fs::write(dir.join("noise"), png.as_bytes()).unwrap();
        let scene: crate::scene::Scene = serde_json::from_value(serde_json::json!({
            "width": 600, "height": 400, "sizes": [{"id": "page", "width": 600, "height": 400}],
            "assets": {"photo": {"sha256": "noise", "width": 2400, "height": 1600}},
            "layers": [{"id": "p", "type": "image", "asset": "photo", "width": 300, "height": 200}]}))
        .unwrap();
        let at = |q| render_pdf(&scene, &scene.sizes[0], &dir, q).unwrap().len();
        let (q90, q30) = (at(90), at(30));
        // Full size and lossless, the noise alone was several MB.
        assert!(q90 < 1_500_000, "{q90} bytes at 90");
        assert!(q30 * 2 < q90, "{q30} bytes at 30 vs {q90} at 90");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_a4_pdf_is_an_a4_page() {
        let page = |size: &str| {
            let size = crate::scene::SizeSpec::Named(size.into())
                .resolve()
                .unwrap();
            let scene: crate::scene::Scene = serde_json::from_value(serde_json::json!({
                "width": size.width, "height": size.height, "sizes": [size],
                "layers": [{"id": "r", "type": "rect", "width": 100, "height": 100, "fill": "#D0202E"}]}))
            .unwrap();
            let pdf = render_pdf(&scene, &scene.sizes[0], &std::env::temp_dir(), 90).unwrap();
            let text = String::from_utf8_lossy(&pdf).into_owned();
            let at = text.find("/MediaBox [").unwrap() + "/MediaBox [".len();
            let end = at + text[at..].find(']').unwrap();
            text[at..end].to_owned()
        };
        assert_eq!(page("a4-portrait"), "0 0 595 842");
        assert_eq!(page("300x200"), "0 0 300 200");
    }

    /// A noisy image, so lossy encoders can't make it tiny.
    fn noisy() -> skia_safe::Image {
        let mut s = skia_safe::surfaces::raster_n32_premul((200, 200)).unwrap();
        let mut v: u32 = 7;
        for y in 0..200 {
            for x in 0..200 {
                v ^= v << 13;
                v ^= v >> 17;
                v ^= v << 5;
                let mut p = skia_safe::Paint::default();
                p.set_color(skia_safe::Color::new(v | 0xFF00_0000));
                s.canvas().draw_point((x as f32, y as f32), &p);
            }
        }
        s.image_snapshot()
    }

    #[test]
    fn max_kb_lowers_lossy_quality_until_the_file_fits() {
        let img = noisy();
        let full = encode(&img, Format::Jpeg, 90, None).unwrap();
        assert!(full.bytes.starts_with(&[0xFF, 0xD8]) && full.lowered.is_none());
        let cap = (full.bytes.len() / 1024 / 2) as u32;
        let fit = encode(&img, Format::Jpeg, 90, Some(cap)).unwrap();
        assert!(fit.bytes.len() <= cap as usize * 1024 && !fit.too_big);
        assert!(fit.lowered.is_some_and(|q| q < 90));
        let webp = encode(&img, Format::Webp, 80, None).unwrap();
        assert_eq!(&webp.bytes[8..12], b"WEBP");
        // Lossless PNG can't shrink: it's flagged instead.
        let png = encode(&img, Format::Png, 90, Some(1)).unwrap();
        assert!(png.too_big && png.lowered.is_none());
        let never = encode(&img, Format::Jpeg, 90, Some(0)).unwrap();
        assert!(never.too_big && never.lowered == Some(1));
    }
}
