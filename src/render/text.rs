//! Draws text layers: plain, filled with an image or a gradient, and outlined.

use anyhow::Result;
use skia_safe::{Paint, PaintStyle};

use super::Ctx;
use super::image::CENTER;
use super::paint::{fill_paint, sk_color};
use crate::layout::Placed;
use crate::scene::{Gradient, ImageFill, Outline};
use crate::text::Text;

impl Ctx<'_> {
    /// Draws text layer `p` (box `r`): its paragraph, or its letters filled
    /// with an image or gradient, then its outline.
    pub(super) fn draw_text(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        r: skia_safe::Rect,
        fill: Option<&ImageFill>,
        gradient: Option<&Gradient>,
        outline: Option<&Outline>,
    ) -> Result<()> {
        let l = p.layer;
        if let (Some((para, fit)), Some(t)) = (&p.text, Text::of(l, p.k)) {
            let origin = (p.rect.x, p.text_top());
            // The letters show an image or gradient laid over the text's box.
            let custom = match (fill, gradient) {
                (Some(f), _) => Some(self.image_paint(
                    &f.asset,
                    p.rect,
                    f.fit,
                    None,
                    f.tile_scale * p.k,
                    CENTER,
                )?),
                (None, Some(g)) => fill_paint(None, Some(g), r),
                (None, None) => None,
            };
            match custom {
                Some(paint) => t.repaint(fit, p.rect.w, &paint, true).paint(canvas, origin),
                None => para.paint(canvas, origin),
            }
            if let Some(o) = outline {
                let mut stroke = Paint::default();
                stroke.set_anti_alias(true);
                stroke.set_style(PaintStyle::Stroke);
                stroke.set_stroke_join(skia_safe::PaintJoin::Round);
                stroke.set_stroke_width(o.width * p.k * t.shrink(fit));
                stroke.set_color(sk_color(o.color));
                // Stroke the glyphs' merged outline: variable fonts
                // draw letters from overlapping contours, whose inner
                // edges would otherwise show as seams.
                let mut para = t.repaint(fit, p.rect.w, &Paint::default(), false);
                let merged = (0..para.line_number())
                    .map(|line| para.get_path_at(line).1)
                    .map(|path| path.simplify().unwrap_or(path))
                    .reduce(|all, line| all.op(&line, skia_safe::PathOp::Union).unwrap_or(all))
                    .unwrap_or_default();
                canvas.save();
                canvas.translate(origin);
                canvas.draw_path(&merged, &stroke);
                canvas.restore();
            }
        }
        Ok(())
    }
}
