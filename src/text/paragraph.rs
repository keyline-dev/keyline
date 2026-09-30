//! Building Skia paragraphs from a text's runs and style.

use skia_safe::{
    FontArguments, FontStyle, Paint, Path, PathBuilder,
    font_arguments::{VariationPosition, variation_position::Coordinate},
    font_style::{Slant, Weight, Width},
    textlayout::{
        Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign, TextDecoration, TextDirection,
        TextShadow, TextStyle,
    },
};

use super::registry::with_fonts;
use super::{Fit, Run, Text};
use crate::scene::{Align, Decoration, Shift};

impl Text<'_> {
    /// The same layout as `fit` came from, every glyph painted with `paint`
    /// (an image or gradient fill, or an outline's stroke). `shadow` keeps
    /// the drop shadow; an outline pass leaves it off so it isn't doubled.
    pub fn repaint(&self, fit: &Fit, width: f32, paint: &Paint, shadow: bool) -> Paragraph {
        let mut p = self.build(fit.font_size, fit.line_limit, Some(paint), shadow);
        super::wrap(&mut p, width);
        p
    }

    /// The same layout as `fit` came from, in the text's own colors: a
    /// counting text drawn with the number of the moment.
    pub fn redraw(&self, fit: &Fit, width: f32) -> Paragraph {
        let mut p = self.build(fit.font_size, fit.line_limit, None, true);
        super::wrap(&mut p, width);
        p
    }

    /// How much `fit` shrank the font; outlines shrink with it.
    pub fn shrink(&self, fit: &Fit) -> f32 {
        fit.font_size / self.font_size
    }

    /// `s` laid out on one line in the text's first run's style, left
    /// aligned: the parts of a leader line.
    pub fn plain_paragraph(&self, s: &str, size: f32, paint: Option<&Paint>) -> Paragraph {
        let mut style = ParagraphStyle::new();
        style.set_text_align(TextAlign::Left);
        let run = self.runs.first().map(|(_, r)| r.clone());
        let mut p = with_fonts(|fc| {
            let mut b = ParagraphBuilder::new(&style, fc.clone());
            if let Some(run) = &run {
                b.push_style(&self.style(size, run, paint, true));
            }
            b.add_text(s);
            b.build()
        });
        p.layout(f32::MAX);
        p
    }

    pub(super) fn paragraph(&self, size: f32, max_lines: Option<usize>) -> Paragraph {
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
            Align::Justify => TextAlign::Justify,
        });
        if self.rtl {
            style.set_text_direction(TextDirection::RTL);
        }
        if let Some(n) = max_lines {
            style.set_max_lines(n);
            if self.ellipsis {
                style.set_ellipsis("…");
            }
        }
        with_fonts(|fc| {
            let mut b = ParagraphBuilder::new(&style, fc.clone());
            for (range, run) in &self.runs {
                b.push_style(&self.style(size, run, paint, shadow));
                b.add_text(&self.display[range.clone()]);
                b.pop();
            }
            b.build()
        })
    }

    fn style(&self, size: f32, run: &Run, paint: Option<&Paint>, shadow: bool) -> TextStyle {
        // Spacing and shadow shrink with the font when `fit` shrinks it.
        let ratio = size / self.font_size;
        let mut px = run.size.map_or(size, |s| s * ratio);
        let mut s = TextStyle::new();
        s.set_font_families(&[run.family.as_deref().unwrap_or(self.family)]);
        if let Some(shift) = run.shift {
            // Raised or lowered, at about 60% size.
            let full = px;
            px *= 0.62;
            s.set_baseline_shift(match shift {
                Shift::Sup => -full * 0.38,
                Shift::Sub => full * 0.14,
            });
        }
        s.set_font_size(px);
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
            Weight::from(i32::from(run.weight)),
            Width::NORMAL,
            if run.italic {
                Slant::Italic
            } else {
                Slant::Upright
            },
        ));
        // Variable fonts (like the bundled Inter) take the weight on their
        // `wght` axis; static fonts ignore it and match by style instead.
        let wght = [Coordinate {
            axis: ('w', 'g', 'h', 't').into(),
            value: f32::from(run.weight),
        }];
        let args = FontArguments::new()
            .set_variation_design_position(VariationPosition { coordinates: &wght });
        s.set_font_arguments(&args);
        if let Some(d) = run.decoration {
            s.set_decoration_type(match d {
                Decoration::Underline => TextDecoration::UNDERLINE,
                Decoration::Strike => TextDecoration::LINE_THROUGH,
            });
            s.set_decoration_color(skia_safe::Color::new(run.color.0));
        }
        for (tag, value) in &self.features {
            s.add_font_feature(tag, i32::try_from(*value).unwrap_or(1));
        }
        match paint {
            Some(p) => {
                s.set_foreground_paint(p);
            }
            None => {
                s.set_color(skia_safe::Color::new(run.color.0));
            }
        }
        s
    }
}

/// Each line's glyph outlines, paragraph coordinates, built run by run:
/// skia's own `Paragraph::get_path_at` drops or shifts runs on a line that
/// changes font or size.
pub fn line_paths(p: &mut Paragraph) -> Vec<Path> {
    let mut lines: Vec<PathBuilder> = (0..p.line_number()).map(|_| PathBuilder::new()).collect();
    p.visit(|line, info| {
        let (Some(info), Some(path)) = (info, lines.get_mut(line)) else {
            return;
        };
        let o = info.origin();
        for (g, at) in info.glyphs().iter().zip(info.positions()) {
            if let Some(glyph) = info.font().get_path(*g) {
                path.add_path_with_offset(&glyph, (o.x + at.x, o.y + at.y), None);
            }
        }
    });
    lines.iter_mut().map(PathBuilder::detach).collect()
}
