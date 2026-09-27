//! Building Skia paragraphs from a text's runs and style.

use skia_safe::{
    FontArguments, FontStyle, Paint,
    font_arguments::{VariationPosition, variation_position::Coordinate},
    font_style::{Slant, Weight, Width},
    textlayout::{Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign, TextShadow, TextStyle},
};

use super::registry::with_fonts;
use super::{Fit, Text};
use crate::scene::{Align, Color};

impl Text<'_> {
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
