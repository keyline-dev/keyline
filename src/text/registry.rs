//! The font registry: the bundled Inter plus every font file added since,
//! and each thread's Skia font collection built from them.

use std::cell::RefCell;
use std::path::Path;
use std::sync::{LazyLock, PoisonError, RwLock};

use anyhow::{Context, Result};
use skia_safe::{
    Data, FontMgr,
    textlayout::{FontCollection, TypefaceFontProvider},
};

/// Inter as one variable font: every weight from 100 to 900 in one file.
const BUNDLED: &[u8] = include_bytes!("../../fonts/InterVariable.ttf");

/// Family name of the bundled font, whatever its file calls itself (the
/// variable Inter file says "Inter Variable").
const BUNDLED_FAMILY: &str = "Inter";

/// Every font file in use, and the family names they provide.
struct Registry {
    /// Each font file and the family name it's registered under.
    files: Vec<(Vec<u8>, String)>,
    families: Vec<String>,
    /// Bumped whenever fonts are added, so threads rebuild their collection.
    generation: u64,
}

static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(|| {
    RwLock::new(Registry {
        files: vec![(BUNDLED.to_vec(), BUNDLED_FAMILY.to_owned())],
        families: vec![BUNDLED_FAMILY.to_owned()],
        generation: 0,
    })
});

thread_local! {
    // Skia font objects aren't Send, so each render thread builds its own,
    // tagged with the registry generation it was built from.
    static FONTS: RefCell<Option<(u64, FontCollection)>> = const { RefCell::new(None) };
}

/// Adds every `.ttf` and `.otf` in `dirs` (missing dirs are skipped),
/// except web-font cache files, which `fonts::load_cache` registers.
/// Returns the available families.
///
/// # Errors
/// When a directory can't be read or a file isn't a font.
pub fn load_fonts(dirs: &[&Path]) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for dir in dirs.iter().filter(|d| d.is_dir()) {
        let cached = crate::fonts::index(dir);
        let mut paths: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf"))
            })
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_none_or(|n| !cached.contains_key(n))
            })
            .collect();
        paths.sort(); // deterministic registration order
        for p in paths {
            let bytes = std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?;
            files.push((bytes, None));
        }
    }
    add_fonts(files)?;
    Ok(families())
}

/// Registers font files, each under its own family name or the given alias.
///
/// # Errors
/// When a file isn't a font; nothing is added then.
pub fn add_fonts(files: Vec<(Vec<u8>, Option<String>)>) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    let mgr = FontMgr::new();
    let mut named = Vec::with_capacity(files.len());
    for (i, (bytes, alias)) in files.into_iter().enumerate() {
        let face = mgr
            .new_from_data(Data::new_copy(&bytes), None)
            .with_context(|| format!("font file #{i} is not a font"))?;
        named.push((bytes, alias.unwrap_or_else(|| face.family_name())));
    }
    let mut r = REGISTRY.write().unwrap_or_else(PoisonError::into_inner);
    r.files.extend(named);
    let mut families: Vec<String> = r.files.iter().map(|(_, f)| f.clone()).collect();
    families.sort();
    families.dedup();
    r.families = families;
    r.generation += 1;
    Ok(())
}

/// Font families that text can use.
pub fn families() -> Vec<String> {
    REGISTRY
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .families
        .clone()
}

/// Runs `f` with this thread's font collection, rebuilt if fonts were added.
pub(super) fn with_fonts<R>(f: impl FnOnce(&FontCollection) -> R) -> R {
    let generation = REGISTRY
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .generation;
    FONTS.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.as_ref().is_none_or(|(g, _)| *g != generation) {
            *cached = Some((generation, collection()));
        }
        let (_, fc) = cached.get_or_insert_with(|| (generation, collection()));
        f(fc)
    })
}

fn collection() -> FontCollection {
    let r = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
    let mgr = FontMgr::new();
    let mut provider = TypefaceFontProvider::new();
    for (bytes, family) in &r.files {
        if let Some(face) = mgr.new_from_data(Data::new_copy(bytes), None) {
            provider.register_typeface(face, Some(family.as_str()));
        }
    }
    let mut fc = FontCollection::new();
    fc.set_asset_font_manager(Some(provider.into()));
    fc.disable_font_fallback();
    fc
}

/// The cap height of `family` at `size` px, when the family is registered.
pub(super) fn cap_height(family: &str, size: f32) -> Option<f32> {
    with_fonts(|fc| {
        let mut fc = fc.clone();
        let tf = fc
            .find_typefaces(&[family], skia_safe::FontStyle::normal())
            .into_iter()
            .next()?;
        let font = skia_safe::Font::from_typeface(tf, size);
        let (_, m) = font.metrics();
        (m.cap_height > 0.0).then_some(m.cap_height)
    })
}

/// The weight a text in `family` at `weight` is drawn at: the closest
/// registered face's (any weight along a variable font's axis). `None`
/// when no face of the family is registered, so a fallback draws it.
pub fn drawn_weight(family: &str, weight: u16, italic: bool) -> Option<u16> {
    // The collection falls back to another family rather than fail.
    if !families().iter().any(|f| f.eq_ignore_ascii_case(family)) {
        return None;
    }
    with_fonts(|fc| {
        let mut fc = fc.clone();
        let slant = if italic {
            skia_safe::font_style::Slant::Italic
        } else {
            skia_safe::font_style::Slant::Upright
        };
        let style = skia_safe::FontStyle::new(
            skia_safe::font_style::Weight::from(i32::from(weight)),
            skia_safe::font_style::Width::NORMAL,
            slant,
        );
        let tf = fc.find_typefaces(&[family], style).into_iter().next()?;
        let wght = skia_safe::FourByteTag::from(('w', 'g', 'h', 't'));
        let axis = tf
            .variation_design_parameters()
            .and_then(|axes| axes.into_iter().find(|a| a.tag == wght));
        Some(match axis {
            Some(a) => f32::from(weight).clamp(a.min, a.max).round() as u16,
            None => u16::try_from(*tf.font_style().weight()).unwrap_or(weight),
        })
    })
}

/// The registered typeface for `family` at `weight` (on a variable font's
/// `wght` axis) and slant, for drawing glyphs outside a paragraph.
pub fn typeface(family: &str, weight: u16, italic: bool) -> Option<skia_safe::Typeface> {
    use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
    with_fonts(|fc| {
        let mut fc = fc.clone();
        let slant = if italic {
            skia_safe::font_style::Slant::Italic
        } else {
            skia_safe::font_style::Slant::Upright
        };
        let style = skia_safe::FontStyle::new(
            skia_safe::font_style::Weight::from(i32::from(weight)),
            skia_safe::font_style::Width::NORMAL,
            slant,
        );
        let tf = fc.find_typefaces(&[family], style).into_iter().next()?;
        let wght = [Coordinate {
            axis: ('w', 'g', 'h', 't').into(),
            value: f32::from(weight),
        }];
        let args = skia_safe::FontArguments::new()
            .set_variation_design_position(VariationPosition { coordinates: &wght });
        Some(tf.clone_with_arguments(&args).unwrap_or(tf))
    })
}
