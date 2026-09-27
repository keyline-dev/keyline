//! Sizing text to its box: natural size, the final layout, and the font
//! size `fit` shrinks to.

use skia_safe::textlayout::Paragraph;

use super::{Fit, Text};
use crate::scene::Resize;

impl Text<'_> {
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
}
