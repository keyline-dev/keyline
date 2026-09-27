//! Golden-image checks: renders compared against reference PNGs per OS,
//! allowing for glyph anti-aliasing that differs between OS versions.

use std::path::PathBuf;

// ponytail: Skia rasterizes glyphs through the OS font stack, so goldens are
// per OS and compared with a tolerance across its versions; embed FreeType
// if renders must match byte-for-byte everywhere.
pub fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(std::env::consts::OS)
        .join(name)
}

pub fn check_golden(name: &str, png: &[u8]) {
    let path = golden(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, png).unwrap();
        return;
    }
    let want = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}; run with UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    let Some(why) = mismatch(&want, png) else {
        return;
    };
    // Keep the actual render so CI can upload it next to the golden.
    let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("golden-actual")
        .join(name);
    std::fs::create_dir_all(actual.parent().unwrap()).unwrap();
    std::fs::write(&actual, png).unwrap();
    panic!(
        "{name} differs from {}: {why}; actual at {}",
        path.display(),
        actual.display()
    );
}

/// Why `got` doesn't match the golden `want`, or `None` when it does: the
/// same size, with at most [`MAX_DIFF_SHARE`] of pixels past [`PIXEL_TOLERANCE`].
pub fn mismatch(want: &[u8], got: &[u8]) -> Option<String> {
    if want == got {
        return None;
    }
    let (size, want_px) = rgba(want);
    let (got_size, got_px) = rgba(got);
    if size != got_size {
        return Some(format!("size {got_size:?}, want {size:?}"));
    }
    let off: Vec<u8> = want_px
        .as_chunks::<4>()
        .0
        .iter()
        .zip(got_px.as_chunks::<4>().0)
        .map(|(a, b)| {
            a.iter()
                .zip(b)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let past = |t: u8| off.iter().filter(|&&d| d > t).count();
    let over = past(PIXEL_TOLERANCE);
    #[expect(
        clippy::cast_precision_loss,
        reason = "pixel counts are far below 2^52"
    )]
    let share = over as f64 / off.len() as f64;
    (share > MAX_DIFF_SHARE).then(|| {
        let histogram = [8, 32, 64, 128]
            .map(|t| format!(">{t}: {}", past(t)))
            .join(", ");
        format!(
            "{over} of {} pixels past {PIXEL_TOLERANCE} ({:.3}%, max {:.3}%; {histogram})",
            off.len(),
            share * 100.0,
            MAX_DIFF_SHARE * 100.0
        )
    })
}

/// Channel difference up to which a pixel still matches the golden. The OS
/// rasterizes glyphs, and macOS versions anti-alias their edges differently.
pub const PIXEL_TOLERANCE: u8 = 48;
/// Share of pixels allowed past [`PIXEL_TOLERANCE`]. A moved, missing or
/// recolored element changes far more than glyph edges do.
pub const MAX_DIFF_SHARE: f64 = 0.002;

/// Encodes unpremultiplied RGBA pixels as a PNG.
pub fn png_of((w, h): (i32, i32), px: &[u8]) -> Vec<u8> {
    use skia_safe::{AlphaType, ColorType, Data, EncodedImageFormat, ImageInfo, images};
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let image = images::raster_from_data(&info, Data::new_copy(px), info.min_row_bytes()).unwrap();
    image
        .encode(None, EncodedImageFormat::PNG, None)
        .unwrap()
        .as_bytes()
        .to_vec()
}

/// A PNG's size and unpremultiplied RGBA pixels.
pub fn rgba(png: &[u8]) -> ((i32, i32), Vec<u8>) {
    use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};
    let image = Image::from_encoded(Data::new_copy(png)).expect("decodable PNG");
    let info = ImageInfo::new(
        image.dimensions(),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut px = vec![0; info.compute_min_byte_size()];
    assert!(image.read_pixels(
        &info,
        &mut px,
        info.min_row_bytes(),
        (0, 0),
        CachingHint::Disallow
    ));
    ((image.width(), image.height()), px)
}
