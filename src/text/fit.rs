//! Sizing text to its box: natural size, the final layout, and the font
//! size `fit` shrinks to.

use skia_safe::textlayout::Paragraph;

use super::{Fit, Text};
use crate::scene::{Resize, TextWrap};

impl Text<'_> {
    /// Box size the text wants before constraints. `width`/`height` are the
    /// layer's (already scaled) box, used by the modes that fix them.
    pub fn natural_size(&self, width: f32, height: f32) -> (f32, f32) {
        match self.resize {
            Resize::AutoWidth => {
                let mut p = self.paragraph(self.font_size, None);
                p.layout(f32::MAX);
                let w = p.max_intrinsic_width().ceil();
                super::wrap(&mut p, w);
                (w, p.height().ceil())
            }
            Resize::AutoHeight => {
                let mut p = self.paragraph(self.font_size, self.max_lines);
                super::wrap(&mut p, width);
                (width, p.height().ceil())
            }
            Resize::Fit | Resize::Fixed | Resize::Truncate => (width, height),
        }
    }

    /// An unlaid paragraph at the text's font size (lay it out at `width`).
    pub fn paragraph_at(&self, _width: f32) -> Paragraph {
        self.paragraph(self.font_size, None)
    }

    /// Height of the text wrapped at `width`, at its font size.
    pub fn height_at(&self, width: f32) -> f32 {
        let mut p = self.paragraph(self.font_size, self.max_lines);
        super::wrap(&mut p, width);
        p.height().ceil()
    }

    /// For `trim: "cap"`: the space above the first line's cap height and
    /// below the last line's baseline, in paragraph `p` at `size` px.
    pub fn cap_trim(&self, p: &Paragraph, size: f32) -> (f32, f32) {
        let metrics = p.get_line_metrics();
        let (Some(first), Some(last)) = (metrics.first(), metrics.last()) else {
            return (0.0, 0.0);
        };
        let cap = super::registry::cap_height(self.family, size).unwrap_or(size * 0.72);
        let top = (first.baseline as f32 - cap).max(0.0);
        let bottom = (p.height() - last.baseline as f32).max(0.0);
        (top, bottom)
    }

    /// Narrowest width the text can wrap to without breaking a word.
    pub fn min_width(&self) -> f32 {
        let mut p = self.paragraph(self.font_size, None);
        p.layout(f32::MAX);
        p.min_intrinsic_width().ceil()
    }

    /// Distance from the top of the box to the first line's baseline, with
    /// the text wrapped at `width`.
    pub fn first_baseline(&self, width: f32) -> f32 {
        let mut p = self.paragraph(self.font_size, None);
        super::wrap(&mut p, width);
        p.get_line_metrics()
            .first()
            .map_or_else(|| p.alphabetic_baseline(), |m| m.baseline as f32)
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
            // Its own box: `maxLines` still cuts it, with an ellipsis.
            _ => (self.font_size, self.max_lines),
        };
        let mut p = self.paragraph(size, max_lines);
        super::wrap(&mut p, width);
        let wrap_width = self.wrap_width(&p, size, max_lines, width);
        if wrap_width < width {
            super::wrap(&mut p, wrap_width);
        }
        let truncated = max_lines.is_some() && p.did_exceed_max_lines();
        // Cut text reports what it needs uncut, at its size: the fix.
        let whole = truncated.then(|| {
            let mut q = self.paragraph(size, None);
            super::wrap(&mut q, wrap_width);
            q
        });
        let whole = whole.as_ref().unwrap_or(&p);
        // What made it shrink: tried a hair bigger, what doesn't fit.
        let bound = (size < self.font_size - 0.01).then(|| {
            let mut q = self.paragraph(size + 1.0 / 32.0, self.max_lines);
            super::wrap(&mut q, width);
            if q.min_intrinsic_width() > width + 0.5 {
                "width"
            } else if q.did_exceed_max_lines() {
                "maxLines"
            } else {
                "height"
            }
        });
        let fit = Fit {
            font_size: size,
            bound,
            overflow: matches!(self.resize, Resize::Fit | Resize::Fixed | Resize::Truncate)
                && (p.height() > height + 0.5 || p.longest_line() > width + 0.5),
            truncated,
            lines: p.line_number(),
            need_height: whole.height().ceil(),
            one_line_width: whole.max_intrinsic_width().ceil(),
            line_limit: max_lines,
            wrap_width,
        };
        (p, fit)
    }

    /// The width to wrap at: the box's, or for `balance` the narrowest that
    /// keeps the same number of lines (even lengths), and for `pretty` the
    /// widest below the box's whose last line has two or more words.
    fn wrap_width(&self, p: &Paragraph, size: f32, max_lines: Option<usize>, width: f32) -> f32 {
        let lines = p.line_number();
        if self.wrap == TextWrap::Wrap || lines < 2 || !width.is_finite() {
            return width;
        }
        let at = |w: f32| {
            let mut q = self.paragraph(size, max_lines);
            super::wrap(&mut q, w);
            q
        };
        match self.wrap {
            TextWrap::Balance => {
                // Bisect down to the narrowest width with the same line count.
                let (mut lo, mut hi) = (p.min_intrinsic_width().ceil().min(width), width);
                while hi - lo > 1.0 {
                    let mid = (lo + hi) / 2.0;
                    if at(mid).line_number() == lines {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                hi.ceil().min(width)
            }
            TextWrap::Pretty => {
                let last_words = |q: &Paragraph| {
                    let m = q.get_line_metrics();
                    let Some(last) = m.last() else { return 0 };
                    let start = super::lines::byte_index(&self.display, last.start_index);
                    let end = super::lines::byte_index(&self.display, last.end_index);
                    self.display
                        .get(start..end)
                        .map_or(0, |t| t.split_whitespace().count())
                };
                if last_words(p) >= 2 {
                    return width;
                }
                // Narrow the lines a step at a time, never adding one.
                let mut w = width;
                while w > width * 0.75 {
                    w -= (width * 0.02).max(1.0);
                    let q = at(w);
                    if q.line_number() > lines {
                        break;
                    }
                    if last_words(&q) >= 2 {
                        return w;
                    }
                }
                width
            }
            TextWrap::Wrap => width,
        }
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
        super::wrap(&mut p, width);
        p.height() <= height + 0.5 && p.longest_line() <= width + 0.5 && !p.did_exceed_max_lines()
    }

    /// Lines to keep before the ellipsis: `maxLines`, or as many as fit.
    fn truncated_lines(&self, size: f32, width: f32, height: f32) -> usize {
        let fitting = self.lines_fitting(size, width, height);
        self.max_lines.map_or(fitting, |n| n.min(fitting))
    }

    fn lines_fitting(&self, size: f32, width: f32, height: f32) -> usize {
        let mut p = self.paragraph(size, None);
        super::wrap(&mut p, width);
        let n = p
            .get_line_metrics()
            .iter()
            .take_while(|m| (m.baseline + m.descent) as f32 <= height + 0.5)
            .count();
        n.max(1)
    }
}
