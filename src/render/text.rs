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
    /// Draws text layer `p` (box `r`): its highlights, its letters filled
    /// by `fills` (or the short-form `fill`/`gradient`/`color`), then its
    /// strokes (or `outline`).
    pub(super) fn draw_text(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        r: skia_safe::Rect,
    ) -> Result<()> {
        self.draw_text_with(canvas, p, r, true)
    }

    /// [`Self::draw_text`], without the highlights when `highlights` is
    /// false (split text draws them once, not per piece).
    pub(super) fn draw_text_with(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        r: skia_safe::Rect,
        highlights: bool,
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
        let (Some((para, fit)), Some(t)) = (&p.text, Text::drawn(l, p.k)) else {
            return Ok(());
        };
        let origin = p.text_origin();
        let width = fit.wrap_width;
        let Kind::Text { more, .. } = &l.kind else {
            return Ok(());
        };
        if highlights {
            super::text_extras::highlights(canvas, &t, para, origin, p.k, l);
        }
        // Knockout letters erase their parent frame instead of painting.
        let knock = |mut paint: Paint| {
            if more.knockout {
                paint.set_blend_mode(skia_safe::BlendMode::DstOut);
            }
            paint
        };
        if let Some(radius) = more.curve {
            let base = t.base_run().map_or(Color(0xFF00_0000), |r| r.color);
            let paint = match l.look.fills.as_ref().and_then(|f| f.as_slice().first()) {
                Some(f) => self.fill(f, p.rect, p.k)?.unwrap_or_default(),
                None => {
                    let mut paint = Paint::default();
                    paint.set_anti_alias(true);
                    paint.set_color(sk_color(base));
                    paint
                }
            };
            super::text_extras::curved(canvas, &t, radius * p.k, r, &knock(paint));
            return Ok(());
        }
        // ponytail: leader lines draw in their runs' colours only; route
        // them through the fill and stroke passes if menus need paints.
        if let Some(leader) = &more.leader
            && super::text_extras::leaders(canvas, &t, fit, leader, origin.0, origin.1, width)
        {
            return Ok(());
        }
        match &l.look.fills {
            // Each paint fills the letters in turn; `[]` leaves them empty.
            Some(fs) => {
                for f in fs.as_slice() {
                    if let Some(paint) = self.fill(f, p.rect, p.k)? {
                        t.repaint(fit, width, &knock(paint), true)
                            .paint(canvas, origin);
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
                    (None, None) if more.knockout => Some(Paint::default()),
                    (None, None) => None,
                };
                match custom {
                    Some(paint) => t
                        .repaint(fit, width, &knock(paint), true)
                        .paint(canvas, origin),
                    // Layout measured a counting text's widest number; draw this moment's.
                    None if l.time.count.is_some() => t.redraw(fit, width).paint(canvas, origin),
                    None => para.paint(canvas, origin),
                }
            }
        }
        match &l.look.strokes {
            Some(ss) => {
                for s in ss.as_slice() {
                    let paint = glyph_stroke(
                        s.width.max() * p.k * t.shrink(fit),
                        s,
                        r,
                        crate::anim::drawn(l),
                    );
                    outline_glyphs(canvas, &t, fit, width, origin, &paint);
                }
            }
            None => {
                if let Some(o) = outline {
                    let s = Stroke::solid(o.width, o.color);
                    let paint =
                        glyph_stroke(o.width * p.k * t.shrink(fit), &s, r, crate::anim::drawn(l));
                    outline_glyphs(canvas, &t, fit, width, origin, &paint);
                }
            }
        }
        Ok(())
    }
}

/// A round-joined stroke paint for glyph outlines, `drawn` of their length
/// drawn: the letters write themselves one after another.
fn glyph_stroke(width: f32, s: &Stroke, r: skia_safe::Rect, drawn: Option<f32>) -> Paint {
    let mut paint = Paint::default();
    if let Some(trim) = drawn.and_then(|d| skia_safe::PathEffect::trim(0.0, d, None)) {
        paint.set_path_effect(trim);
    }
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
    let merged = crate::text::line_paths(&mut para)
        .into_iter()
        .map(|path| path.simplify().unwrap_or(path))
        .reduce(|all, line| all.op(&line, skia_safe::PathOp::Union).unwrap_or(all))
        .unwrap_or_default();
    canvas.save();
    canvas.translate(origin);
    canvas.draw_path(&merged, paint);
    canvas.restore();
}
