//! Text shaping and measurement on Skia's paragraph module. Fonts are the
//! bundled Inter plus any font files added at startup or later (downloaded
//! web fonts, see `fonts`); the machine's own fonts are never used, so output
//! doesn't depend on them.

mod fit;
mod lines;
mod paragraph;
mod registry;
#[cfg(test)]
mod tests;

use crate::scene::{Align, Color, Kind, Layer, Range, Resize, TextCase};

pub use registry::{add_fonts, families, load_fonts};

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

    /// Font size, scaled px, before any fitting.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// The sizing mode in effect.
    pub fn resize(&self) -> Resize {
        self.resize
    }
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
