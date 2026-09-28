//! Effects around a layer: shadows (behind and inset), background blur,
//! and the transforms applied after layout.

use skia_safe::{
    BlurStyle, ClipOp, MaskFilter, PathBuilder, PathFillType, canvas::SaveLayerRec, image_filters,
};

use super::paint::{sk_color, sk_rect};
use super::shape::{Shape, corner_radii, rrect_corners};
use crate::layout::Placed;
use crate::scene::Shadow;

/// CSS blur radius to a Gaussian sigma.
pub(super) fn sigma(blur: f32) -> f32 {
    blur / 2.0
}

/// Applies rotation, skew, scale, flips and offset about the box center.
/// Rotation alone keeps the MVP's single call.
pub(super) fn transform(canvas: &skia_safe::Canvas, p: &Placed) {
    let l = p.layer;
    let c = sk_rect(p.rect).center();
    if !l.look.transforms() {
        if l.rotation != 0.0 {
            canvas.rotate(l.rotation, Some(c));
        }
        return;
    }
    let [ox, oy] = l.look.offset;
    canvas.translate((ox * p.k, oy * p.k));
    canvas.translate((c.x, c.y));
    if l.rotation != 0.0 {
        canvas.rotate(l.rotation, None);
    }
    let [sx, sy] = l.look.skew;
    if sx != 0.0 || sy != 0.0 {
        canvas.skew((sx.to_radians().tan(), sy.to_radians().tan()));
    }
    let fx = if l.look.flip_x { -1.0 } else { 1.0 };
    let fy = if l.look.flip_y { -1.0 } else { 1.0 };
    canvas.scale((l.look.scale * fx, l.look.scale * fy));
    canvas.translate((-c.x, -c.y));
}

/// Blurs what's already drawn behind `shape` (frosted glass).
pub(super) fn backdrop_blur(canvas: &skia_safe::Canvas, shape: &Shape, blur: f32) {
    let Some(filter) = image_filters::blur((sigma(blur), sigma(blur)), None, None, None) else {
        return;
    };
    canvas.save();
    shape.clip(canvas, ClipOp::Intersect);
    canvas.save_layer(&SaveLayerRec::default().backdrop(&filter));
    canvas.restore();
    canvas.restore();
}

/// A drop shadow from a shape: the shape grown by `spread`, offset and blurred.
pub(super) fn shape_shadow(canvas: &skia_safe::Canvas, shape: &Shape, s: &Shadow, k: f32) {
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(sk_color(s.color));
    if s.blur > 0.0 {
        paint.set_mask_filter(MaskFilter::blur(
            BlurStyle::Normal,
            sigma(s.blur * k),
            false,
        ));
    }
    let grown = grow(shape, s.spread * k);
    canvas.save();
    canvas.translate((s.x * k, s.y * k));
    grown.fill(canvas, &paint);
    canvas.restore();
}

/// The paint for a layer group whose content casts a drop shadow of its own
/// alpha (text glyphs, a cutout photo): the shadow only, not the content.
pub(super) fn alpha_shadow_paint(s: &Shadow, k: f32) -> Option<skia_safe::Paint> {
    let spread = s.spread * k;
    let input = if spread > 0.0 {
        image_filters::dilate((spread, spread), None, None)
    } else if spread < 0.0 {
        image_filters::erode((-spread, -spread), None, None)
    } else {
        None
    };
    let color: skia_safe::Color4f = sk_color(s.color).into();
    let filter = image_filters::drop_shadow_only(
        (s.x * k, s.y * k),
        (sigma(s.blur * k), sigma(s.blur * k)),
        color,
        None,
        input,
        None,
    )?;
    let mut p = skia_safe::Paint::default();
    p.set_image_filter(filter);
    Some(p)
}

/// An inner shadow: everything outside the shape, offset and shrunk by
/// `spread`, blurred, and clipped to the shape.
pub(super) fn inset_shadow(canvas: &skia_safe::Canvas, shape: &Shape, s: &Shadow, k: f32) {
    let b = shape.bounds();
    let reach = (s.blur + s.spread.abs()) * k + s.x.abs().max(s.y.abs()) * k + 2.0;
    let hole = grow(shape, -s.spread * k)
        .path()
        .make_offset((s.x * k, s.y * k));
    let mut b2 = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    b2.add_rect(b.with_outset((reach, reach)), None, None);
    b2.add_path(&hole, None);
    let path = b2.detach();
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(sk_color(s.color));
    if s.blur > 0.0 {
        paint.set_mask_filter(MaskFilter::blur(
            BlurStyle::Normal,
            sigma(s.blur * k),
            false,
        ));
    }
    canvas.save();
    shape.clip(canvas, ClipOp::Intersect);
    canvas.draw_path(&path, &paint);
    canvas.restore();
}

/// The shape grown outward by `by` px (inward when negative).
fn grow(shape: &Shape, by: f32) -> Shape {
    if by == 0.0 {
        return match shape {
            Shape::Rect(rr) => Shape::Rect(*rr),
            Shape::Oval(r) => Shape::Oval(*r),
            Shape::Path(p) => Shape::Path(p.clone()),
            Shape::Line(a, b) => Shape::Line(*a, *b),
        };
    }
    match shape {
        Shape::Rect(rr) => {
            let r = rr.rect().with_outset((by, by));
            let radii = corner_radii(rr).map(|v| (v + by).max(0.0));
            Shape::Rect(rrect_corners(r, radii))
        }
        Shape::Oval(r) => Shape::Oval(r.with_outset((by, by))),
        // Paths and lines: offset the outline with a stroke of twice the spread.
        other => {
            let mut stroke = skia_safe::Paint::default();
            stroke.set_style(skia_safe::PaintStyle::Stroke);
            stroke.set_stroke_width(by.abs() * 2.0);
            let base = other.path();
            let mut out = PathBuilder::new();
            if skia_safe::path_utils::fill_path_with_paint(&base, &stroke, &mut out, None, None) {
                let op = if by > 0.0 {
                    skia_safe::PathOp::Union
                } else {
                    skia_safe::PathOp::Difference
                };
                Shape::Path(base.op(&out.detach(), op).unwrap_or(base))
            } else {
                Shape::Path(base)
            }
        }
    }
}

/// Dots on a `spacing` px grid from `origin`, each as big as its cell is
/// dark, sampling the child image at the cell's center.
const HALFTONE: &str = "
uniform shader image;
uniform float spacing;
uniform float2 origin;
half4 main(float2 p) {
    float2 center = (floor((p - origin) / spacing) + 0.5) * spacing + origin;
    half4 c = image.eval(center);
    float lum = c.a > 0 ? dot(c.rgb / c.a, half3(0.299, 0.587, 0.114)) : 1;
    float r = spacing * 0.71 * sqrt(1 - lum);
    float a = clamp(r - distance(p, center) + 0.5, 0, 1) * c.a;
    return half4(0, 0, 0, a);
}";

/// Replaces `paint`'s image shader with its halftone.
pub(super) fn halftone(paint: &mut skia_safe::Paint, spacing: f32, origin: (f32, f32)) {
    let Some(image) = paint.shader() else { return };
    // ponytail: compiled per draw; cache the effect if halftones get common.
    let Ok(effect) = skia_safe::RuntimeEffect::make_for_shader(HALFTONE, None) else {
        return;
    };
    let uniforms: Vec<u8> = [spacing.max(1.0), origin.0, origin.1]
        .iter()
        .flat_map(|f| f.to_ne_bytes())
        .collect();
    if let Some(sh) =
        effect.make_shader(skia_safe::Data::new_copy(&uniforms), &[image.into()], None)
    {
        paint.set_shader(sh);
    }
}
