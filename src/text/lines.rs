//! The text of each line as drawn, for `render`'s text report.

use skia_safe::textlayout::Paragraph;

use super::{Fit, Text};

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
pub(super) fn byte_index(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (byte, c) in text.char_indices() {
        if units >= utf16 {
            return byte;
        }
        units += c.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_index_maps_utf16_units() {
        assert_eq!(byte_index("aé🎉b", 0), 0);
        assert_eq!(byte_index("aé🎉b", 2), 3);
        assert_eq!(byte_index("aé🎉b", 4), 7);
        assert_eq!(byte_index("aé🎉b", 99), 8);
    }
}
