//! Skia paints and conversions: fills, gradients, strokes and blend modes.

use skia_safe::{Paint, PaintStyle, RRect, TileMode, gradient};

use crate::layout::Rect;
use crate::scene::{BlendMode, Color, Gradient, Stroke, StrokeAlign};

/// Paint for a solid color or a gradient (the gradient wins); `None` when
/// the shape has no fill.
pub(super) fn fill_paint(
    color: Option<Color>,
    gradient: Option<&Gradient>,
    r: skia_safe::Rect,
) -> Option<Paint> {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    match (gradient, color) {
        (Some(g), _) => {
            let at = |[x, y]: [f32; 2]| (r.left + x * r.width(), r.top + y * r.height());
            let colors: Vec<skia_safe::Color4f> =
                g.stops.iter().map(|s| sk_color(s.color).into()).collect();
            let pos: Vec<f32> = g.stops.iter().map(|s| s.at).collect();
            let spec = gradient::Gradient::new(
                gradient::Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                gradient::Interpolation::default(),
            );
            p.set_shader(gradient::shaders::linear_gradient(
                (at(g.from), at(g.to)),
                &spec,
                None,
            ));
        }
        (None, Some(c)) => {
            p.set_color(sk_color(c));
        }
        (None, None) => return None,
    }
    Some(p)
}

/// Strokes the box edge; `inside` and `outside` shift the path by half the
/// width so the stroke lies wholly on one side, as design tools draw it.
pub(super) fn draw_stroke(
    canvas: &skia_safe::Canvas,
    s: &Stroke,
    r: skia_safe::Rect,
    radius: f32,
    k: f32,
) {
    let width = s.width * k;
    let shift = match s.align {
        StrokeAlign::Inside => -width / 2.0,
        StrokeAlign::Center => 0.0,
        StrokeAlign::Outside => width / 2.0,
    };
    let edge = r.with_outset((shift, shift));
    let color = s
        .color
        .or((s.gradient.is_none()).then_some(Color(0xFF00_0000)));
    if let Some(mut p) = fill_paint(color, s.gradient.as_ref(), edge) {
        p.set_style(PaintStyle::Stroke);
        p.set_stroke_width(width);
        canvas.draw_rrect(rrect(edge, (radius + shift).max(0.0)), &p);
    }
}

pub(super) fn sk_blend(m: BlendMode) -> skia_safe::BlendMode {
    use skia_safe::BlendMode as B;
    match m {
        BlendMode::Normal => B::SrcOver,
        BlendMode::Multiply => B::Multiply,
        BlendMode::Screen => B::Screen,
        BlendMode::Overlay => B::Overlay,
        BlendMode::Darken => B::Darken,
        BlendMode::Lighten => B::Lighten,
        BlendMode::ColorDodge => B::ColorDodge,
        BlendMode::ColorBurn => B::ColorBurn,
        BlendMode::HardLight => B::HardLight,
        BlendMode::SoftLight => B::SoftLight,
        BlendMode::Difference => B::Difference,
        BlendMode::Exclusion => B::Exclusion,
        BlendMode::Hue => B::Hue,
        BlendMode::Saturation => B::Saturation,
        BlendMode::Color => B::Color,
        BlendMode::Luminosity => B::Luminosity,
    }
}

pub(super) fn sk_color(c: Color) -> skia_safe::Color {
    skia_safe::Color::new(c.0)
}

pub(super) fn sk_rect(r: Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(r.x, r.y, r.w, r.h)
}

pub(super) fn rrect(r: skia_safe::Rect, radius: f32) -> RRect {
    RRect::new_rect_xy(r, radius, radius)
}
