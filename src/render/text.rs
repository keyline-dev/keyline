//! Draws text layers: plain, filled with paints (colors, gradients, images,
//! patterns), and outlined. Shadows are drawn around them by the caller.

use anyhow::Result;
use skia_safe::{Paint, PaintStyle};

use super::Ctx;
use super::fills::gradient_shader;
use super::image::CENTER;
use super::paint::sk_color;
use crate::layout::Placed;
use crate::scene::{Color, Kind, Stroke};
use crate::text::{Fit, Text};

impl Ctx<'_> {
    /// Draws text layer `p` (box `r`): its letters filled by `fills` (or the
    /// MVP `fill`/`gradient`/`color`), then its strokes (or `outline`).
    pub(super) fn draw_text(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        r: skia_safe::Rect,
    ) -> Result<()> {
        let l = p.layer;
        let Kind::Text {
            fill,
            gradient,
            outline,
            ..
        } = &l.kind
        else {
            return Ok(());
        };
        let (Some((para, fit)), Some(t)) = (&p.text, Text::of(l, p.k)) else {
            return Ok(());
        };
        let origin = (p.rect.x, p.text_top());
        match &l.look.fills {
            // Each paint fills the letters in turn; `[]` leaves them empty.
            Some(fs) => {
                for f in fs.as_slice() {
                    if let Some(paint) = self.fill(f, p.rect, p.k)? {
                        t.repaint(fit, p.rect.w, &paint, true).paint(canvas, origin);
                    }
                }
            }
            None => {
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
                    (None, Some(g)) => gradient_shader(g, r).map(|sh| {
                        let mut paint = Paint::default();
                        paint.set_anti_alias(true);
                        paint.set_shader(sh);
                        paint
                    }),
                    (None, None) => None,
                };
                match custom {
                    Some(paint) => t.repaint(fit, p.rect.w, &paint, true).paint(canvas, origin),
                    None => para.paint(canvas, origin),
                }
            }
        }
        match &l.look.strokes {
            Some(ss) => {
                for s in ss.as_slice() {
                    let paint = glyph_stroke(s.width.max() * p.k * t.shrink(fit), s, r);
                    outline_glyphs(canvas, &t, fit, p.rect.w, origin, &paint);
                }
            }
            None => {
                if let Some(o) = outline {
                    let s = Stroke::solid(o.width, o.color);
                    let paint = glyph_stroke(o.width * p.k * t.shrink(fit), &s, r);
                    outline_glyphs(canvas, &t, fit, p.rect.w, origin, &paint);
                }
            }
        }
        Ok(())
    }
}

/// A round-joined stroke paint for glyph outlines.
fn glyph_stroke(width: f32, s: &Stroke, r: skia_safe::Rect) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Stroke);
    paint.set_stroke_join(skia_safe::PaintJoin::Round);
    paint.set_stroke_width(width);
    match s.gradient.as_ref().and_then(|g| gradient_shader(g, r)) {
        Some(sh) => {
            paint.set_shader(sh);
        }
        None => {
            paint.set_color(sk_color(s.color.unwrap_or(Color(0xFF00_0000))));
        }
    }
    paint
}

/// Strokes the glyphs' merged outline: variable fonts draw letters from
/// overlapping contours, whose inner edges would otherwise show as seams.
fn outline_glyphs(
    canvas: &skia_safe::Canvas,
    t: &Text,
    fit: &Fit,
    width: f32,
    origin: (f32, f32),
    paint: &Paint,
) {
    let mut para = t.repaint(fit, width, &Paint::default(), false);
    let merged = (0..para.line_number())
        .map(|line| para.get_path_at(line).1)
        .map(|path| path.simplify().unwrap_or(path))
        .reduce(|all, line| all.op(&line, skia_safe::PathOp::Union).unwrap_or(all))
        .unwrap_or_default();
    canvas.save();
    canvas.translate(origin);
    canvas.draw_path(&merged, paint);
    canvas.restore();
}
