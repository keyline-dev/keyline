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
        let (img, drawn) = self.image_drawn(id, bx, fit, crop, tile, focus)?;
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

    /// Asset `id` decoded, and the rect it's drawn in for box `bx`.
    pub(super) fn image_drawn(
        &mut self,
        id: &str,
        bx: Rect,
        fit: Fit,
        crop: Option<&Crop>,
        tile: f32,
        focus: [f32; 2],
    ) -> Result<(Image, Rect)> {
        let a = self
            .scene
            .assets
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown asset {id}"))?;
        let drawn = image_rect(bx, a.width, a.height, fit, crop, tile, focus);
        Ok((self.image(id, &a, drawn)?, drawn))
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
        Fit::Stretch => return bx,
        Fit::Cover => (bx.w / iw).max(bx.h / ih),
        Fit::Contain => (bx.w / iw).min(bx.h / ih),
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
    let d = image_rect(bx, iw, ih, fit, crop, tile, CENTER);
    (d.w / iw).max(d.h / ih)
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

/// `img` at most `width` px wide, its height in proportion: a print PDF
/// needs no more than its dpi at the drawn size. Smaller ones as they are.
pub(super) fn at_most(img: &Image, width: f32) -> Image {
    let w = width.ceil().max(1.0);
    if (img.width() as f32) <= w * 1.02 {
        return img.clone();
    }
    let h = (img.height() as f32 * w / img.width() as f32)
        .ceil()
        .max(1.0);
    let Some(mut surface) = skia_safe::surfaces::raster_n32_premul((w as i32, h as i32)) else {
        return img.clone();
    };
    surface.canvas().draw_image_rect_with_sampling_options(
        img,
        None,
        skia_safe::Rect::from_wh(w, h),
        skia_safe::SamplingOptions::new(
            skia_safe::FilterMode::Linear,
            skia_safe::MipmapMode::Linear,
        ),
        &skia_safe::Paint::default(),
    );
    surface.image_snapshot()
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

/// `focus` aimed at `subject` (0–1 of the image) for a cover crop in `bx`:
/// the window, the box's shape as large as the image (or its `crop`)
/// allows, centred on the subject and kept inside the image. Without a
/// subject, or when nothing is cropped on an axis, `focus` as given.
pub fn aim(
    focus: [f32; 2],
    subject: Option<[f32; 4]>,
    bx: Rect,
    (iw, ih): (f32, f32),
    crop: Option<&Crop>,
) -> [f32; 2] {
    let Some([sx, sy, sw, sh]) = subject else {
        return focus;
    };
    let (sx, sy, sw, sh) = (sx * iw, sy * ih, sw * iw, sh * ih);
    let (rx, ry, rw, rh) = crop.map_or((0.0, 0.0, iw, ih), |c| {
        (c.x * iw, c.y * ih, c.width * iw, c.height * ih)
    });
    let s = (bx.w / rw).max(bx.h / rh);
    let (vw, vh) = (bx.w / s, bx.h / s);
    let along = |f: f32, region: f32, view: f32, centre: f32| {
        if region - view > 0.5 {
            ((centre - view / 2.0) / (region - view)).clamp(0.0, 1.0)
        } else {
            f
        }
    };
    [
        along(focus[0], rw, vw, sx + sw / 2.0 - rx),
        along(focus[1], rh, vh, sy + sh / 2.0 - ry),
    ]
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
                Fit::Cover,
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
                Fit::Contain,
                None,
                1.0,
                CENTER
            ),
            r(0.0, 25.0, 100.0, 50.0)
        );
        assert_eq!(
            image_crop(r(0.0, 0.0, 100.0, 100.0), 400.0, 200.0, Fit::Cover, None),
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
            Fit::Cover,
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
                Fit::Cover,
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

    #[test]
    fn a_crop_is_aimed_at_the_subject() {
        // A 1600×1000 photo in a 1200×300 band shows 1600×400 of it.
        let band = r(0.0, 0.0, 1200.0, 300.0);
        let photo = (1600.0, 1000.0);
        // No subject: focus as given.
        assert_eq!(aim([0.5, 0.5], None, band, photo, None), [0.5, 0.5]);
        // A subject from 30% to 60% down: the window centres on 45%.
        let f = aim([0.5, 0.5], Some([0.0, 0.3, 1.0, 0.3]), band, photo, None);
        let top = (1000.0 - 400.0) * f[1];
        assert!((top + 200.0 - 450.0).abs() < 0.5, "{f:?}");
        // Near an edge, the window stops at the photo's edge.
        assert_eq!(
            aim([0.5, 0.5], Some([0.0, 0.0, 1.0, 0.1]), band, photo, None)[1],
            0.0
        );
        // Nothing is cropped across: focus x stays.
        assert_eq!(
            aim([0.2, 0.5], Some([0.6, 0.3, 0.2, 0.3]), band, photo, None)[0],
            0.2
        );
        // Within a hand-picked crop (the bottom half), the window slides only
        // inside it: low as it goes, 600–1000, to hold a subject at 800–900.
        let c = Crop {
            x: 0.0,
            y: 0.5,
            width: 1.0,
            height: 0.5,
        };
        let f = aim(
            [0.5, 0.5],
            Some([0.0, 0.8, 1.0, 0.1]),
            band,
            photo,
            Some(&c),
        );
        assert_eq!(f[1], 1.0, "{f:?}");
    }
}
