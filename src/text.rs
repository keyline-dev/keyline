//! Text shaping and measurement on Skia's paragraph module. Fonts are the
//! bundled Inter plus any font files added at startup or later (downloaded
//! web fonts, see `fonts`); the machine's own fonts are never used, so output
//! doesn't depend on them.

use std::cell::RefCell;
use std::path::Path;
use std::sync::{LazyLock, PoisonError, RwLock};

use anyhow::{Context, Result};
use skia_safe::{
    Data, FontArguments, FontMgr, FontStyle, Paint,
    font_arguments::{VariationPosition, variation_position::Coordinate},
    font_style::{Slant, Weight, Width},
    textlayout::{
        FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign, TextShadow,
        TextStyle, TypefaceFontProvider,
    },
};

use crate::scene::{Align, Color, Kind, Layer, Range, Resize, TextCase};

/// Inter as one variable font: every weight from 100 to 900 in one file.
const BUNDLED: &[u8] = include_bytes!("../fonts/InterVariable.ttf");

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
fn with_fonts<R>(f: impl FnOnce(&FontCollection) -> R) -> R {
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

/// A text layer's content and style at a given scale factor.
pub struct Text<'a> {
    /// The text as drawn: with `textCase` applied.
    display: String,
    /// Byte ranges of `display` and their colors.
    runs: Vec<(std::ops::Range<usize>, Color)>,
    font_size: f32,
    family: &'a str,
    weight: u16,
    align: Align,
    resize: Resize,
    max_lines: Option<usize>,
    min_font_scale: f32,
    ellipsis: bool,
    /// Scaled with the font size, so `fit` shrinks them together.
    letter_spacing: f32,
    line_height: Option<f32>,
    shadow: Option<(Color, f32, f32, f32)>,
}

/// How a laid-out paragraph fits its box.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fit {
    /// Font size used, px; below the requested size when `fit` shrank it.
    pub font_size: f32,
    /// Text runs past the box (fit / fixed / truncate modes only).
    pub overflow: bool,
    /// An ellipsis was applied (truncate, or fit at its minimum size).
    pub truncated: bool,
    /// Number of lines drawn.
    pub lines: usize,
    /// Height the text needs at the box's width.
    pub need_height: f32,
    /// Width the text needs to sit on one line.
    pub one_line_width: f32,
    /// Line limit applied (fit at its minimum, or truncate), for repaints.
    pub line_limit: Option<usize>,
}

impl<'a> Text<'a> {
    /// `k` is the size's Scale-tool factor; it multiplies the font size.
    pub fn of(layer: &'a Layer, k: f32) -> Option<Self> {
        let Kind::Text {
            text,
            ranges,
            max_lines,
            min_font_scale,
            ellipsis,
            font_size,
            weight,
            align,
            color,
            font_family,
            letter_spacing,
            line_height,
            text_case,
            shadow,
            ..
        } = &layer.kind
        else {
            return None;
        };
        // Case is applied per color run, so ranges keep counting characters
        // of the text as written even when casing changes its length (ß → SS).
        let mut display = String::new();
        let mut runs = Vec::new();
        for (run, c) in color_runs(text, ranges, *color) {
            let start = display.len();
            match text_case {
                TextCase::None => display.push_str(run),
                TextCase::Upper => display.push_str(&run.to_uppercase()),
                TextCase::Lower => display.push_str(&run.to_lowercase()),
            }
            runs.push((start..display.len(), c));
        }
        Some(Text {
            display,
            runs,
            font_size: font_size * k,
            family: font_family,
            weight: *weight,
            align: *align,
            resize: layer.text_resize()?,
            max_lines: *max_lines,
            min_font_scale: *min_font_scale,
            ellipsis: *ellipsis,
            letter_spacing: letter_spacing * k,
            line_height: *line_height,
            shadow: shadow
                .as_ref()
                .map(|s| (s.color, s.x * k, s.y * k, s.blur * k)),
        })
    }

    /// The sizing mode in effect.
    pub fn resize(&self) -> Resize {
        self.resize
    }

    /// Box size the text wants before constraints. `width`/`height` are the
    /// layer's (already scaled) box, used by the modes that fix them.
    pub fn natural_size(&self, width: f32, height: f32) -> (f32, f32) {
        match self.resize {
            Resize::AutoWidth => {
                let mut p = self.paragraph(self.font_size, None);
                p.layout(f32::MAX);
                let w = p.max_intrinsic_width().ceil();
                p.layout(w);
                (w, p.height().ceil())
            }
            Resize::AutoHeight => {
                let mut p = self.paragraph(self.font_size, None);
                p.layout(width);
                (width, p.height().ceil())
            }
            Resize::Fit | Resize::Fixed | Resize::Truncate => (width, height),
        }
    }

    /// Lays the text out in its final box.
    pub fn layout(&self, width: f32, height: f32) -> (Paragraph, Fit) {
        let (size, max_lines) = match self.resize {
            Resize::Fit => {
                let size = self.fitting_size(width, height);
                let fits = self.fits(size, width, height);
                // At its minimum size and still too big: ellipsize, like UILabel.
                (
                    size,
                    (!fits).then(|| self.truncated_lines(size, width, height)),
                )
            }
            Resize::Truncate => (
                self.font_size,
                Some(self.truncated_lines(self.font_size, width, height)),
            ),
            _ => (self.font_size, None),
        };
        let mut p = self.paragraph(size, max_lines);
        p.layout(width);
        let fit = Fit {
            font_size: size,
            overflow: matches!(self.resize, Resize::Fit | Resize::Fixed | Resize::Truncate)
                && (p.height() > height + 0.5 || p.longest_line() > width + 0.5),
            truncated: max_lines.is_some() && p.did_exceed_max_lines(),
            lines: p.line_number(),
            need_height: p.height().ceil(),
            one_line_width: p.max_intrinsic_width().ceil(),
            line_limit: max_lines,
        };
        (p, fit)
    }

    /// The same layout as `fit` came from, every glyph painted with `paint`
    /// (an image or gradient fill, or an outline's stroke). `shadow` keeps
    /// the drop shadow; an outline pass leaves it off so it isn't doubled.
    pub fn repaint(&self, fit: &Fit, width: f32, paint: &Paint, shadow: bool) -> Paragraph {
        let mut p = self.build(fit.font_size, fit.line_limit, Some(paint), shadow);
        p.layout(width);
        p
    }

    /// How much `fit` shrank the font; outlines shrink with it.
    pub fn shrink(&self, fit: &Fit) -> f32 {
        fit.font_size / self.font_size
    }

    /// Largest size between `fontSize × minFontScale` and `fontSize` at
    /// which the text fits, found by bisection to 1/64 px.
    fn fitting_size(&self, width: f32, height: f32) -> f32 {
        let max = self.font_size;
        let min = max * self.min_font_scale;
        if self.fits(max, width, height) {
            return max;
        }
        if !self.fits(min, width, height) {
            return min;
        }
        let (mut lo, mut hi) = (min, max);
        while hi - lo > 1.0 / 64.0 {
            let mid = (lo + hi) / 2.0;
            if self.fits(mid, width, height) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    fn fits(&self, size: f32, width: f32, height: f32) -> bool {
        let mut p = self.paragraph(size, self.max_lines);
        p.layout(width);
        p.height() <= height + 0.5 && p.longest_line() <= width + 0.5 && !p.did_exceed_max_lines()
    }

    /// Lines to keep before the ellipsis: `maxLines`, or as many as fit.
    fn truncated_lines(&self, size: f32, width: f32, height: f32) -> usize {
        let fitting = self.lines_fitting(size, width, height);
        self.max_lines.map_or(fitting, |n| n.min(fitting))
    }

    fn lines_fitting(&self, size: f32, width: f32, height: f32) -> usize {
        let mut p = self.paragraph(size, None);
        p.layout(width);
        let n = p
            .get_line_metrics()
            .iter()
            .take_while(|m| (m.baseline + m.descent) as f32 <= height + 0.5)
            .count();
        n.max(1)
    }

    fn paragraph(&self, size: f32, max_lines: Option<usize>) -> Paragraph {
        self.build(size, max_lines, None, true)
    }

    fn build(
        &self,
        size: f32,
        max_lines: Option<usize>,
        paint: Option<&Paint>,
        shadow: bool,
    ) -> Paragraph {
        let mut style = ParagraphStyle::new();
        style.set_text_align(match self.align {
            Align::Left => TextAlign::Left,
            Align::Center => TextAlign::Center,
            Align::Right => TextAlign::Right,
        });
        if let Some(n) = max_lines {
            style.set_max_lines(n);
            if self.ellipsis {
                style.set_ellipsis("…");
            }
        }
        with_fonts(|fc| {
            let mut b = ParagraphBuilder::new(&style, fc.clone());
            for (range, color) in &self.runs {
                b.push_style(&self.style(size, *color, paint, shadow));
                b.add_text(&self.display[range.clone()]);
                b.pop();
            }
            b.build()
        })
    }

    fn style(&self, size: f32, color: Color, paint: Option<&Paint>, shadow: bool) -> TextStyle {
        // Spacing and shadow shrink with the font when `fit` shrinks it.
        let ratio = size / self.font_size;
        let mut s = TextStyle::new();
        s.set_font_families(&[self.family]);
        s.set_font_size(size);
        s.set_letter_spacing(self.letter_spacing * ratio);
        if let Some(h) = self.line_height {
            s.set_height(h);
            s.set_height_override(true);
        }
        if let Some((c, x, y, blur)) = self.shadow.filter(|_| shadow) {
            // A design tool's blur radius is about two standard deviations.
            let sigma = f64::from(blur * ratio / 2.0);
            s.add_shadow(TextShadow::new(
                skia_safe::Color::new(c.0),
                (x * ratio, y * ratio),
                sigma,
            ));
        }
        s.set_font_style(FontStyle::new(
            Weight::from(i32::from(self.weight)),
            Width::NORMAL,
            Slant::Upright,
        ));
        // Variable fonts (like the bundled Inter) take the weight on their
        // `wght` axis; static fonts ignore it and match by style instead.
        let wght = [Coordinate {
            axis: ('w', 'g', 'h', 't').into(),
            value: f32::from(self.weight),
        }];
        let args = FontArguments::new()
            .set_variation_design_position(VariationPosition { coordinates: &wght });
        s.set_font_arguments(&args);
        match paint {
            Some(p) => {
                s.set_foreground_paint(p);
            }
            None => {
                s.set_color(skia_safe::Color::new(color.0));
            }
        }
        s
    }
}

impl Text<'_> {
    /// The text of each drawn line, as laid out, with "…" where it was cut.
    pub fn drawn_lines(&self, p: &Paragraph, fit: &Fit) -> Vec<String> {
        let metrics = p.get_line_metrics();
        let last = metrics.len().saturating_sub(1);
        metrics
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let from = byte_index(&self.display, m.start_index);
                let to = byte_index(&self.display, m.end_excluding_whitespaces).max(from);
                let mut line = self.display[from..to].trim_end_matches('\n').to_owned();
                if i == last && fit.truncated && self.ellipsis {
                    line.push('…');
                }
                line
            })
            .collect()
    }
}

/// Skia's line indices count UTF-16 units; this maps one to a byte offset.
fn byte_index(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (byte, c) in text.char_indices() {
        if units >= utf16 {
            return byte;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// Splits `text` into maximal runs of one color. Ranges count characters;
/// a later range wins where ranges overlap.
fn color_runs<'t>(text: &'t str, ranges: &[Range], base: Color) -> Vec<(&'t str, Color)> {
    let color_at = |i: usize| {
        ranges
            .iter()
            .rev()
            .find(|r| (r.start..r.end).contains(&i))
            .map_or(base, |r| r.color)
    };
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;
    for (i, (byte, _)) in text.char_indices().enumerate() {
        let c = color_at(i);
        match current {
            Some(prev) if prev != c => {
                runs.push((&text[start..byte], prev));
                start = byte;
                current = Some(c);
            }
            None => current = Some(c),
            _ => {}
        }
    }
    if let Some(c) = current {
        runs.push((&text[start..], c));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text_layer(v: serde_json::Value) -> Layer {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn color_runs_split_on_character_ranges() {
        let red = Color(0xFFFF_0000);
        let black = Color(0xFF00_0000);
        let ranges = [Range {
            start: 1,
            end: 3,
            color: red,
        }];
        let runs = color_runs("héllo", &ranges, black);
        assert_eq!(runs, vec![("h", black), ("él", red), ("lo", black)]);
        assert_eq!(color_runs("", &ranges, black), vec![]);
    }

    #[test]
    fn auto_width_grows_with_text_and_scale() {
        let short = text_layer(json!({"type": "text", "text": "Hi", "fontSize": 40}));
        let long = text_layer(json!({"type": "text", "text": "Hello world", "fontSize": 40}));
        let (ws, hs) = Text::of(&short, 1.0).unwrap().natural_size(0.0, 0.0);
        let (wl, hl) = Text::of(&long, 1.0).unwrap().natural_size(0.0, 0.0);
        assert!(wl > ws * 2.0, "{wl} vs {ws}");
        assert_eq!(hs, hl);
        let (wh, hh) = Text::of(&long, 0.5).unwrap().natural_size(0.0, 0.0);
        assert!((wh - wl / 2.0).abs() < 2.0 && hh < hl);
    }

    #[test]
    fn auto_height_wraps() {
        let l = text_layer(
            json!({"type": "text", "text": "one two three four five six", "fontSize": 20, "resize": "auto-height", "width": 80}),
        );
        let t = Text::of(&l, 1.0).unwrap();
        let (w, h) = t.natural_size(80.0, 0.0);
        assert_eq!(w, 80.0);
        let (_, fit) = t.layout(w, h);
        assert!(fit.lines > 1 && !fit.overflow);
    }

    #[test]
    fn fixed_reports_overflow_and_truncate_ellipsizes() {
        let text = "one two three four five six seven eight";
        let fixed = text_layer(
            json!({"type": "text", "text": text, "fontSize": 20, "resize": "fixed", "width": 80, "height": 30}),
        );
        let (_, fit) = Text::of(&fixed, 1.0).unwrap().layout(80.0, 30.0);
        assert!(fit.overflow && !fit.truncated);
        // The server says what would fit, so the agent needn't guess.
        assert!(
            fit.need_height > 30.0 && fit.one_line_width > 80.0,
            "{fit:?}"
        );
        let (_, refit) = Text::of(&fixed, 1.0).unwrap().layout(80.0, fit.need_height);
        assert!(!refit.overflow, "{refit:?}");

        let trunc = text_layer(
            json!({"type": "text", "text": text, "fontSize": 20, "resize": "truncate", "width": 80, "height": 30}),
        );
        let (_, fit) = Text::of(&trunc, 1.0).unwrap().layout(80.0, 30.0);
        assert!(fit.truncated && !fit.overflow && fit.lines == 1, "{fit:?}");
    }

    #[test]
    fn max_lines_never_keeps_more_lines_than_fit() {
        let text = "one two three four five six seven eight";
        let trunc = text_layer(
            json!({"type": "text", "text": text, "fontSize": 20, "resize": "truncate", "maxLines": 5, "width": 80, "height": 30}),
        );
        let (_, fit) = Text::of(&trunc, 1.0).unwrap().layout(80.0, 30.0);
        assert!(fit.truncated && !fit.overflow && fit.lines == 1, "{fit:?}");
        let tiny = text_layer(
            json!({"type": "text", "text": text, "fontSize": 40, "minFontScale": 0.9, "maxLines": 5, "width": 200, "height": 50}),
        );
        let (_, fit) = Text::of(&tiny, 1.0).unwrap().layout(200.0, 50.0);
        assert!(fit.truncated && !fit.overflow, "{fit:?}");
    }

    #[test]
    fn fit_shrinks_the_font_like_uilabel() {
        let text = "one two three four five six seven eight";
        // Box given, no resize: fit. Fits at the requested size → unchanged.
        let roomy = text_layer(
            json!({"type": "text", "text": "Hi", "fontSize": 20, "width": 200, "height": 40}),
        );
        let (_, fit) = Text::of(&roomy, 1.0).unwrap().layout(200.0, 40.0);
        assert_eq!(
            (fit.font_size, fit.overflow, fit.truncated),
            (20.0, false, false)
        );

        // Too big → shrinks until it fits, staying above the minimum.
        let tight = text_layer(
            json!({"type": "text", "text": text, "fontSize": 40, "width": 200, "height": 100}),
        );
        let (p, fit) = Text::of(&tight, 1.0).unwrap().layout(200.0, 100.0);
        assert!(fit.font_size < 40.0 && fit.font_size >= 20.0, "{fit:?}");
        assert!(
            !fit.overflow && !fit.truncated && p.height() <= 100.5,
            "{fit:?}"
        );
        // The Scale tool scales the maximum too.
        let (_, half) = Text::of(&tight, 0.5).unwrap().layout(100.0, 50.0);
        assert!(half.font_size <= 20.0, "{half:?}");

        // Still too big at minFontScale → minimum size, then an ellipsis.
        let tiny = text_layer(
            json!({"type": "text", "text": text, "fontSize": 40, "minFontScale": 0.9, "width": 200, "height": 50}),
        );
        let (_, fit) = Text::of(&tiny, 1.0).unwrap().layout(200.0, 50.0);
        assert_eq!(fit.font_size, 36.0);
        assert!(fit.truncated && !fit.overflow, "{fit:?}");
    }

    #[test]
    fn drawn_lines_show_breaks_and_cuts() {
        let text = "héllo wörld 🎉 one two three four";
        let wrap = text_layer(json!({"type": "text", "text": text, "fontSize": 20, "width": 120}));
        let t = Text::of(&wrap, 1.0).unwrap();
        let (w, h) = t.natural_size(120.0, 0.0);
        let (p, fit) = t.layout(w, h);
        let lines = t.drawn_lines(&p, &fit);
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(lines.join(" "), text);

        let cut = text_layer(
            json!({"type": "text", "text": text, "fontSize": 20, "width": 120, "height": 24, "resize": "truncate"}),
        );
        let t = Text::of(&cut, 1.0).unwrap();
        let (p, fit) = t.layout(120.0, 24.0);
        let lines = t.drawn_lines(&p, &fit);
        assert!(lines.len() == 1 && lines[0].ends_with('…'), "{lines:?}");

        let clip = text_layer(
            json!({"type": "text", "text": text, "fontSize": 20, "width": 120, "height": 24, "resize": "truncate", "ellipsis": false}),
        );
        let t = Text::of(&clip, 1.0).unwrap();
        let (p, fit) = t.layout(120.0, 24.0);
        assert!(fit.truncated && !t.drawn_lines(&p, &fit)[0].ends_with('…'));
    }

    #[test]
    fn byte_index_maps_utf16_units() {
        assert_eq!(byte_index("aé🎉b", 0), 0);
        assert_eq!(byte_index("aé🎉b", 2), 3);
        assert_eq!(byte_index("aé🎉b", 4), 7);
        assert_eq!(byte_index("aé🎉b", 99), 8);
    }

    #[test]
    fn letter_spacing_case_and_line_height_change_the_layout() {
        let plain = text_layer(json!({"type": "text", "text": "Hello", "fontSize": 20}));
        let spaced = text_layer(
            json!({"type": "text", "text": "Hello", "fontSize": 20, "letterSpacing": 10}),
        );
        let (w0, h0) = Text::of(&plain, 1.0).unwrap().natural_size(0.0, 0.0);
        let (w1, _) = Text::of(&spaced, 1.0).unwrap().natural_size(0.0, 0.0);
        assert!(w1 >= w0 + 40.0, "{w0} → {w1}");

        let tall =
            text_layer(json!({"type": "text", "text": "Hello", "fontSize": 20, "lineHeight": 3}));
        let (_, h2) = Text::of(&tall, 1.0).unwrap().natural_size(0.0, 0.0);
        assert!((h2 - 60.0).abs() <= 1.0 && h2 > h0 * 2.0, "{h0} → {h2}");

        // Ranges count characters of the text as written, even when upper
        // case changes its length.
        let upper = text_layer(
            json!({"type": "text", "text": "straße sale", "textCase": "upper",
            "ranges": [{"start": 7, "end": 11, "color": "#FF0000"}]}),
        );
        let t = Text::of(&upper, 1.0).unwrap();
        assert_eq!(t.display, "STRASSE SALE");
        assert_eq!(&t.display[t.runs[1].0.clone()], "SALE");
    }

    #[test]
    fn unknown_font_families_are_rejected_with_the_list() {
        let s: crate::scene::Scene = serde_json::from_value(json!({
            "width": 10, "height": 10, "sizes": [{"id": "a", "width": 10, "height": 10}],
            "layers": [{"id": "t", "type": "text", "text": "x", "fontFamily": "Comic Sans"}]
        }))
        .unwrap();
        let err = s.validate().unwrap_err();
        // Other tests may register fonts too, so check the list includes Inter.
        assert!(
            err.contains("unknown fontFamily Comic Sans; available: "),
            "{err}"
        );
        assert!(err.contains("Inter"), "{err}");
        assert!(families().contains(&"Inter".to_string()));
    }

    #[test]
    fn every_weight_from_one_variable_font() {
        // Heavier weights draw wider glyphs, all the way to 900.
        let width = |w: u16| {
            let l =
                text_layer(json!({"type": "text", "text": "Weight", "fontSize": 40, "weight": w}));
            Text::of(&l, 1.0).unwrap().natural_size(0.0, 0.0).0
        };
        let widths: Vec<f32> = [100, 400, 700, 900].into_iter().map(width).collect();
        assert!(widths.windows(2).all(|p| p[1] > p[0]), "{widths:?}");
    }
}
