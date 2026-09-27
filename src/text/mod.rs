//! Text shaping and measurement on Skia's paragraph module. Fonts are the
//! bundled Inter plus any font files added at startup or later (downloaded
//! web fonts, see `fonts`); the machine's own fonts are never used, so output
//! doesn't depend on them.

mod fit;
mod lines;
pub mod markup;
mod paragraph;
mod registry;
mod runs;
#[cfg(test)]
mod tests;

use crate::scene::{Align, Color, Direction, Highlight, Kind, Layer, Resize, TextWrap};

pub use registry::{add_fonts, families, load_fonts, typeface};
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
        let (display, runs) = runs::build(text, ranges, &base, *text_case, k);
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
            features: more.features.iter().map(|(k, v)| (k.clone(), *v)).collect(),
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

    /// The first run's style: what leaders and curved text draw with.
    pub fn base_run(&self) -> Option<&Run> {
        self.runs.first().map(|(_, r)| r)
    }

    /// The font family.
    pub fn family(&self) -> &str {
        self.family
    }
}

/// True when the first letter with a strong direction is Hebrew or Arabic.
fn first_strong_is_rtl(s: &str) -> bool {
    s.chars().find(|c| c.is_alphabetic()).is_some_and(
        |c| matches!(u32::from(c), 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF),
    )
}

#[cfg(test)]
mod direction_tests {
    #[test]
    fn direction_follows_the_first_strong_letter() {
        assert!(super::first_strong_is_rtl("2026 שלום world"));
        assert!(!super::first_strong_is_rtl("hello שלום"));
        assert!(super::first_strong_is_rtl("مرحبا"));
        assert!(!super::first_strong_is_rtl("123"));
    }
}
