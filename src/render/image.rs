//! Images: where one lands in its box, decoding and SVG rasterizing, and the
//! paint that shows it.

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    AlphaType, ColorType, Data, FilterMode, Image, ImageInfo, Matrix, MipmapMode, Paint,
    SamplingOptions, TileMode, images,
};

use super::paint::sk_rect;
use super::{Ctx, MAX_SVG_PX};
use crate::layout::Rect;
use crate::scene::{Asset, Crop, Fit};

impl Ctx<'_> {
    /// A paint showing image `id` placed over `bx` (cover, contain, crop, or
    /// repeated tiles `tile` times the image's size); transparent elsewhere.
    pub(super) fn image_paint(
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

    pub(super) fn image(&mut self, id: &str, a: &Asset, drawn: Rect) -> Result<Image> {
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

pub(super) const CENTER: [f32; 2] = [0.5, 0.5];

/// Where the whole `iw`×`ih` image lands for `fit`, centered on `bx`; with a
/// `crop`, so that the cropped part exactly covers `bx`; for `tile`, the
/// first tile at `bx`'s corner, `tile` times the image's size. Anything
/// outside `bx` is clipped. `focus` (0–1 per axis) is the point of the
/// image kept in view when it overflows or letterboxes; center is `CENTER`.
pub(super) fn image_rect(
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

pub(super) fn svg_image(bytes: &[u8], pw: u32, ph: u32) -> Result<Image> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn svg_size_reads_viewbox() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="40"><rect width="50" height="40"/></svg>"#;
        assert_eq!(svg_size(svg).unwrap(), (50.0, 40.0));
        assert!(svg_size(b"not svg").is_err());
    }
}
