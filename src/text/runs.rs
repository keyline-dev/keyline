//! Styled runs: the displayed text (markup removed, case applied) split
//! into spans that share one style. Each character starts with the layer's
//! style, then markup spans apply, then explicit `ranges` (later wins).

use crate::scene::{Color, Decoration, Highlight, Range, Shift, TextCase};

use super::markup;

/// The style of one run of text.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// Fill color.
    pub color: Color,
    /// Weight, 100–900.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Font size, scaled px; the layer's when `None`.
    pub size: Option<f32>,
    /// Font family; the layer's when `None`.
    pub family: Option<String>,
    /// Underline or strike.
    pub decoration: Option<Decoration>,
    /// A box behind the run.
    pub highlight: Option<Highlight>,
    /// Superscript or subscript.
    pub shift: Option<Shift>,
}

impl Run {
    /// This style with a span's fields laid over it.
    fn with(&self, r: &Range, k: f32) -> Run {
        Run {
            color: r.color.unwrap_or(self.color),
            weight: r.weight.unwrap_or(self.weight),
            italic: r.italic.unwrap_or(self.italic),
            size: r.font_size.map(|s| s * k).or(self.size),
            family: r.font_family.clone().or_else(|| self.family.clone()),
            decoration: r.decoration.or(self.decoration),
            highlight: r.highlight.clone().or_else(|| self.highlight.clone()),
            shift: r.shift.or(self.shift),
        }
    }
}

/// The displayed text and its runs as byte ranges.
pub(super) fn build(
    text: &str,
    ranges: &[Range],
    base: &Run,
    case: TextCase,
    k: f32,
) -> (String, Vec<(std::ops::Range<usize>, Run)>) {
    let (plain, spans) = markup::parse(text);
    let styles: Vec<Run> = (0..plain.chars().count())
        .map(|i| {
            spans
                .iter()
                .chain(ranges)
                .filter(|r| (r.start..r.end).contains(&i))
                .fold(base.clone(), |style, r| style.with(r, k))
        })
        .collect();
    // Group characters with the same style, then apply the case per run, so
    // ranges keep counting characters as written even when casing changes
    // a run's length (ß → SS).
    let mut display = String::new();
    let mut runs: Vec<(std::ops::Range<usize>, Run)> = Vec::new();
    let mut word_start = true;
    let mut chars = plain.chars().zip(styles).peekable();
    while let Some((c, style)) = chars.next() {
        let mut run_text = String::from(c);
        while let Some((next, _)) = chars.next_if(|(_, s)| *s == style) {
            run_text.push(next);
        }
        let start = display.len();
        match case {
            TextCase::None => display.push_str(&run_text),
            TextCase::Upper => display.push_str(&run_text.to_uppercase()),
            TextCase::Lower => display.push_str(&run_text.to_lowercase()),
            TextCase::Capitalize => {
                for ch in run_text.chars() {
                    if word_start && ch.is_alphabetic() {
                        display.extend(ch.to_uppercase());
                    } else {
                        display.push(ch);
                    }
                    word_start = ch.is_whitespace() || (word_start && !ch.is_alphabetic());
                }
            }
        }
        runs.push((start..display.len(), style));
    }
    (display, runs)
}

#[cfg(test)]
mod tests {
    use super::{Run, build};
    use crate::scene::{Color, Range, TextCase};

    fn base() -> Run {
        Run {
            color: Color(0xFF00_0000),
            weight: 400,
            italic: false,
            size: None,
            family: None,
            decoration: None,
            highlight: None,
            shift: None,
        }
    }

    #[test]
    fn markup_and_ranges_style_runs() {
        let red = Color(0xFFFF_0000);
        let ranges = [Range {
            start: 0,
            end: 6,
            color: Some(red),
            ..Range::default()
        }];
        let (text, runs) = build(
            "Proven <b>RESULTS</b>",
            &ranges,
            &base(),
            TextCase::None,
            1.0,
        );
        assert_eq!(text, "Proven RESULTS");
        assert_eq!(runs.len(), 3);
        assert_eq!((runs[0].0.clone(), runs[0].1.color), (0..6, red));
        assert_eq!((runs[2].0.clone(), runs[2].1.weight), (7..14, 700));
    }

    #[test]
    fn capitalize_uppercases_word_starts_across_runs() {
        let (text, _) = build(
            "hello <b>big</b> world",
            &[],
            &base(),
            TextCase::Capitalize,
            1.0,
        );
        assert_eq!(text, "Hello Big World");
        let (text, _) = build("straße", &[], &base(), TextCase::Upper, 1.0);
        assert_eq!(text, "STRASSE");
    }
}
