//! Strokes around shapes: inside, centered or outside, dashed, capped,
//! per side on rects, with markers at line ends.

use skia_safe::{
    ClipOp, PaintCap, PaintJoin, PaintStyle, Path, PathBuilder, PathEffect, Point, RRect,
};

use super::fills::gradient_shader;
use super::paint::sk_color;
use super::shape::{Shape, corner_radii, rrect_corners};
use crate::scene::{Cap, Color, Join, Marker, Stroke, StrokeAlign, StrokeWidth};

/// Draws `s` around `shape`; `k` scales widths and dashes. `drawn` is the
/// share of its length drawn (all when `None`), from its start: a line's
/// first end, the top of an ellipse, clockwise.
pub(super) fn draw_stroke(
    canvas: &skia_safe::Canvas,
    shape: &Shape,
    s: &Stroke,
    k: f32,
    drawn: Option<f32>,
) {
    let width = s.width.max() * k;
    if width <= 0.0 || drawn.is_some_and(|d| d <= 0.0) {
        return;
    }
    // A line shortens itself, so its end markers follow the tip.
    let trim = drawn
        .filter(|_| !matches!(shape, Shape::Line(..)))
        .and_then(|d| PathEffect::trim(0.0, d, None));
    let stroke_paint = |s: &Stroke, r: skia_safe::Rect, width: f32, k: f32| {
        let mut p = stroke_paint(s, r, width, k);
        if let Some(t) = &trim {
            let both = match p.path_effect() {
                Some(e) => PathEffect::compose(e, t.clone()),
                None => t.clone(),
            };
            p.set_path_effect(both);
        }
        p
    };
    let mut p = stroke_paint(s, shape.bounds(), width, k);
    match (shape, s.width) {
        // ponytail: per-side borders are filled strips and ignore `draw`; trim them if asked.
        (Shape::Rect(rr), StrokeWidth::Sides(sides)) => {
            side_borders(canvas, *rr.rect(), sides.map(|w| w * k), s.align, &p);
        }
        (Shape::Rect(rr), _) => {
            // Shift the edge by half the width so the stroke lies wholly on
            // one side, as design tools draw it.
            let shift = shift(s.align, width);
            let r = rr.rect().with_outset((shift, shift));
            let radii = corner_radii(rr).map(|v| (v + shift).max(0.0));
            if s.gradient.is_some() {
                p = stroke_paint(s, r, width, k);
            }
            canvas.draw_rrect(rrect_corners(r, radii), &p);
        }
        (Shape::Oval(r), _) => {
            let shift = shift(s.align, width);
            let edge = r.with_outset((shift, shift));
            if s.gradient.is_some() {
                p = stroke_paint(s, edge, width, k);
            }
            if trim.is_some() {
                // Drawn from the top, clockwise: a progress ring.
                canvas.draw_path(
                    &Path::oval_with_start_index(edge, skia_safe::PathDirection::CW, 0),
                    &p,
                );
            } else {
                // Radii past half the box make Skia draw an ellipse.
                let radius = (r.width().max(r.height()) + shift).max(0.0);
                canvas.draw_rrect(RRect::new_rect_xy(edge, radius, radius), &p);
            }
        }
        (Shape::Line(a, b), _) => {
            let d = drawn.unwrap_or(1.0);
            let tip = Point::new(a.x + (b.x - a.x) * d, a.y + (b.y - a.y) * d);
            canvas.draw_line(*a, tip, &p);
            markers(canvas, *a, tip, s, width);
        }
        (Shape::Path(path), _) => {
            canvas.save();
            match s.align {
                StrokeAlign::Center => {}
                StrokeAlign::Inside => {
                    canvas.clip_path(path, ClipOp::Intersect, true);
                    p.set_stroke_width(width * 2.0);
                }
                StrokeAlign::Outside => {
                    canvas.clip_path(path, ClipOp::Difference, true);
                    p.set_stroke_width(width * 2.0);
                }
            }
            canvas.draw_path(path, &p);
            canvas.restore();
        }
    }
}

fn shift(align: StrokeAlign, width: f32) -> f32 {
    match align {
        StrokeAlign::Inside => -width / 2.0,
        StrokeAlign::Center => 0.0,
        StrokeAlign::Outside => width / 2.0,
    }
}

/// The paint for a stroke of `width` over box `r` (a gradient's box).
fn stroke_paint(s: &Stroke, r: skia_safe::Rect, width: f32, k: f32) -> skia_safe::Paint {
    let mut p = skia_safe::Paint::default();
    p.set_anti_alias(true);
    match s.gradient.as_ref().and_then(|g| gradient_shader(g, r)) {
        Some(sh) => {
            p.set_shader(sh);
        }
        None => {
            p.set_color(sk_color(s.color.unwrap_or(Color(0xFF00_0000))));
        }
    }
    p.set_style(PaintStyle::Stroke);
    p.set_stroke_width(width);
    if s.cap != Cap::Butt {
        p.set_stroke_cap(match s.cap {
            Cap::Round => PaintCap::Round,
            Cap::Square | Cap::Butt => PaintCap::Square,
        });
    }
    if s.join != Join::Miter {
        p.set_stroke_join(match s.join {
            Join::Round => PaintJoin::Round,
            Join::Bevel | Join::Miter => PaintJoin::Bevel,
        });
    }
    let rough = (s.rough > 0.0)
        .then(|| super::rough::rough_effect(s.rough * k, s.seed))
        .flatten();
    let dash = (s.dash.len() >= 2)
        .then(|| {
            let intervals: Vec<f32> = s.dash.iter().map(|d| d * k).collect();
            PathEffect::dash(&intervals, 0.0)
        })
        .flatten();
    match (dash, rough) {
        (Some(d), Some(r)) => {
            p.set_path_effect(PathEffect::compose(d, r));
        }
        (Some(e), None) | (None, Some(e)) => {
            p.set_path_effect(e);
        }
        (None, None) => {}
    }
    p
}

/// Per-side borders: a strip along each side with a non-zero width.
fn side_borders(
    canvas: &skia_safe::Canvas,
    r: skia_safe::Rect,
    [t, rt, b, l]: [f32; 4],
    align: StrokeAlign,
    p: &skia_safe::Paint,
) {
    let mut fill = p.clone();
    fill.set_style(PaintStyle::Fill);
    fill.set_path_effect(None);
    // How far each strip reaches outside the box: none inside, half centered, all outside.
    let out = |w: f32| match align {
        StrokeAlign::Inside => 0.0,
        StrokeAlign::Center => w / 2.0,
        StrokeAlign::Outside => w,
    };
    let strips = [
        (
            t,
            skia_safe::Rect::new(r.left, r.top - out(t), r.right, r.top - out(t) + t),
        ),
        (
            b,
            skia_safe::Rect::new(r.left, r.bottom + out(b) - b, r.right, r.bottom + out(b)),
        ),
        (
            l,
            skia_safe::Rect::new(r.left - out(l), r.top, r.left - out(l) + l, r.bottom),
        ),
        (
            rt,
            skia_safe::Rect::new(r.right + out(rt) - rt, r.top, r.right + out(rt), r.bottom),
        ),
    ];
    for (w, strip) in strips {
        if w > 0.0 {
            canvas.draw_rect(strip, &fill);
        }
    }
}

/// Arrowheads and dots at a line's ends, sized from the stroke width.
fn markers(canvas: &skia_safe::Canvas, a: Point, b: Point, s: &Stroke, width: f32) {
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    if len <= 0.0 {
        return;
    }
    let dir = Point::new((b.x - a.x) / len, (b.y - a.y) / len);
    let mut fill = stroke_paint(s, skia_safe::Rect::new(a.x, a.y, b.x, b.y), width, 1.0);
    fill.set_path_effect(None);
    for (marker, at, d) in [(s.start, a, Point::new(-dir.x, -dir.y)), (s.end, b, dir)] {
        let Some(m) = marker else { continue };
        let size = (width * 4.0).max(10.0);
        let (n, back) = (
            Point::new(-d.y, d.x),
            Point::new(at.x - d.x * size, at.y - d.y * size),
        );
        let mut path = PathBuilder::new();
        match m {
            Marker::Arrow | Marker::Triangle => {
                path.move_to(Point::new(
                    back.x + n.x * size / 2.0,
                    back.y + n.y * size / 2.0,
                ))
                .line_to(at)
                .line_to(Point::new(
                    back.x - n.x * size / 2.0,
                    back.y - n.y * size / 2.0,
                ));
                if m == Marker::Triangle {
                    path.close();
                }
            }
            Marker::Circle => {
                path.add_circle(at, size / 2.0, None);
            }
            Marker::Diamond => {
                let h = size / 2.0;
                path.move_to(Point::new(at.x + d.x * h, at.y + d.y * h))
                    .line_to(Point::new(at.x + n.x * h, at.y + n.y * h))
                    .line_to(Point::new(at.x - d.x * h, at.y - d.y * h))
                    .line_to(Point::new(at.x - n.x * h, at.y - n.y * h))
                    .close();
            }
        }
        let mut paint = fill.clone();
        paint.set_style(if m == Marker::Arrow {
            PaintStyle::Stroke
        } else {
            PaintStyle::Fill
        });
        canvas.draw_path(&path.detach(), &paint);
    }
}
