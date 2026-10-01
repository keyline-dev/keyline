//! Text shaping and measurement on Skia's paragraph module. Fonts are the
//! bundled Inter plus any font files added at startup or later (downloaded
//! web fonts, see `fonts`); the machine's own fonts are never used, so output
//! doesn't depend on them.

mod fit;
mod lines;
pub mod markup;
mod paragraph;
pub use paragraph::line_paths;
mod registry;
mod runs;
#[cfg(test)]
mod tests;

use crate::scene::{Align, Color, Direction, Highlight, Kind, Layer, Resize, TextWrap};

/// Room added to every wrap width, px: boxes built from a measured width
/// can come back a float's hair narrower (padding added, then taken away),
/// which would wrap text that fits exactly. Far below a visible pixel.
const WRAP_SLACK: f32 = 0.01;

/// Lays `p` out wrapped at `width`, with [`WRAP_SLACK`].
pub(crate) fn wrap(p: &mut skia_safe::textlayout::Paragraph, width: f32) {
    p.layout(width + WRAP_SLACK);
}

pub use registry::{add_fonts, drawn_weight, families, load_fonts, typeface};
pub use runs::Run;

/// A text layer's content and style at a given scale factor.
pub struct Text<'a> {
    /// The text as drawn: markup removed, `textCase` applied.
    display: String,
    /// Byte ranges of `display` and their styles.
    runs: Vec<(std::ops::Range<usize>, Run)>,
    font_size: f32,
    family: &'a str,
    align: Align,
    resize: Resize,
    max_lines: Option<usize>,
    min_font_scale: f32,
    ellipsis: bool,
    /// Scaled with the font size, so `fit` shrinks them together.
    letter_spacing: f32,
    line_height: Option<f32>,
    shadow: Option<(Color, f32, f32, f32)>,
    wrap: TextWrap,
    rtl: bool,
    features: Vec<(String, u32)>,
    /// A box behind every line.
    highlight: Option<Highlight>,
}

/// How a laid-out paragraph fits its box.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fit {
    /// Font size used, px; below the requested size when `fit` shrank it.
    pub font_size: f32,
    /// When it shrank, what stopped it being bigger: `width` (a word too
    /// wide), `height` or `maxLines`.
    pub bound: Option<&'static str>,
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
    /// Width the lines wrap at: the box's, or narrower for `balance`/`pretty`.
    pub wrap_width: f32,
}

impl<'a> Text<'a> {
    /// `k` is the size's Scale-tool factor; it multiplies the font size. A
    /// counting text's `{{n}}` is its widest value, so layout holds still.
    pub fn of(layer: &'a Layer, k: f32) -> Option<Self> {
        Self::of_at(layer, k, false)
    }

    /// Like [`Text::of`], with a counting text's `{{n}}` the number of the
    /// moment being drawn.
    pub fn drawn(layer: &'a Layer, k: f32) -> Option<Self> {
        Self::of_at(layer, k, true)
    }

    fn of_at(layer: &'a Layer, k: f32, now: bool) -> Option<Self> {
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
            more,
            ..
        } = &layer.kind
        else {
            return None;
        };
        let base = Run {
            color: *color,
            weight: *weight,
            italic: more.italic,
            size: None,
            family: None,
            decoration: more.decoration,
            highlight: None,
            shift: None,
        };
        let counted = crate::anim::count::text(layer, now);
        let (display, runs) = runs::build(
            counted.as_deref().unwrap_or(text),
            ranges,
            &base,
            *text_case,
            k,
        );
        let mut features: Vec<(String, u32)> =
            more.features.iter().map(|(k, v)| (k.clone(), *v)).collect();
        // A counting number keeps its width: tabular figures, where the font has them.
        if counted.is_some() && !features.iter().any(|(tag, _)| tag == "tnum") {
            features.push(("tnum".into(), 1));
        }
        let rtl = match more.direction {
            Direction::Rtl => true,
            Direction::Ltr => false,
            Direction::Auto => first_strong_is_rtl(&display),
        };
        Some(Text {
            display,
            runs,
            font_size: font_size * k,
            family: font_family,
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
            wrap: more.text_wrap,
            rtl,
            features,
            highlight: more.highlight.clone(),
        })
    }

    /// Font size, scaled px, before any fitting.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// The sizing mode in effect.
    pub fn resize(&self) -> Resize {
        self.resize
    }

    /// The text as drawn.
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Horizontal alignment.
    pub fn align(&self) -> Align {
        self.align
    }

    /// Highlight boxes to draw: `(UTF-16 range in the display text, highlight)`,
    /// the layer's own over the whole text first.
    pub fn highlights(&self) -> Vec<(std::ops::Range<usize>, &Highlight)> {
        let utf16 = |byte: usize| self.display[..byte].encode_utf16().count();
        let mut out = Vec::new();
        if let Some(h) = &self.highlight {
            out.push((0..utf16(self.display.len()), h));
        }
        for (range, run) in &self.runs {
            if let Some(h) = &run.highlight {
                out.push((utf16(range.start)..utf16(range.end), h));
            }
        }
        out
    }

    /// Each run's letter color: `(UTF-16 range in the display text, color)`.
    pub fn colors(&self) -> Vec<(std::ops::Range<usize>, Color)> {
        let utf16 = |byte: usize| self.display[..byte].encode_utf16().count();
        self.runs
            .iter()
            .map(|(range, run)| (utf16(range.start)..utf16(range.end), run.color))
            .collect()
    }

    /// The first run's style: what leaders and curved text draw with.
    pub fn base_run(&self) -> Option<&Run> {
        self.runs.first().map(|(_, r)| r)
    }

    /// The font family.
    pub fn family(&self) -> &str {
        self.family
    }
}

/// True when the first letter with a strong direction is right-to-left
/// (Hebrew, Arabic, Syriac, Thaana, N'Ko, …), as Unicode's bidi rules say.
fn first_strong_is_rtl(s: &str) -> bool {
    unicode_bidi::get_base_direction_full(s) == unicode_bidi::Direction::Rtl
}

#[cfg(test)]
mod direction_tests {
    #[test]
    fn direction_follows_the_first_strong_letter() {
        assert!(super::first_strong_is_rtl("2026 שלום world"));
        assert!(!super::first_strong_is_rtl("hello שלום"));
        assert!(super::first_strong_is_rtl("مرحبا"));
        assert!(!super::first_strong_is_rtl("123"));
        // Syriac and Thaana, and a right-to-left line after a neutral one.
        assert!(super::first_strong_is_rtl("ܫܠܡܐ"));
        assert!(super::first_strong_is_rtl("ދިވެހި"));
        assert!(super::first_strong_is_rtl("2026\nשלום"));
    }
}
