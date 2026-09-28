//! Text extras drawn around or instead of the paragraph: highlight boxes
//! behind lines or spans, menu dot leaders, and text set along an arc.

use skia_safe::{
    Font, PathEffect, Point, RSXform, TextBlob,
    textlayout::{Paragraph, RectHeightStyle, RectWidthStyle},
};

use super::paint::sk_color;
use crate::scene::{HighlightStyle, Layer};
use crate::text::{Fit, Text};

/// Boxes behind the highlighted lines and spans of `para`, drawn at `origin`.
pub(super) fn highlights(
    canvas: &skia_safe::Canvas,
    t: &Text,
    para: &Paragraph,
    origin: (f32, f32),
    k: f32,
    layer: &Layer,
) {
    for (range, h) in t.highlights() {
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(sk_color(h.color));
        if h.style == HighlightStyle::Brush {
            // A marker pen: ragged edges, the same every render.
            paint.set_path_effect(PathEffect::discrete(
                6.0 * k,
                h.padding * k * 0.35,
                seed(&layer.id),
            ));
        }
        let pad = h.padding * k;
        for b in para.get_rects_for_range(range, RectHeightStyle::Tight, RectWidthStyle::Tight) {
            let r = b.rect.with_offset(origin).with_outset((pad, pad * 0.5));
            let radius = h.radius * k;
            canvas.draw_rrect(skia_safe::RRect::new_rect_xy(r, radius, radius), &paint);
        }
    }
}

/// Menu lines with a tab: the part before it flush left, the part after
/// flush right, and the leader character filling the gap. Returns false
/// when the text has no tab (the caller draws it normally).
pub(super) fn leaders(
    canvas: &skia_safe::Canvas,
    t: &Text,
    fit: &Fit,
    leader: &str,
    x: f32,
    y: f32,
    width: f32,
) -> bool {
    if !t.display().contains('\t') || leader.is_empty() {
        return false;
    }
    let size = fit.font_size;
    let dot = t.plain_paragraph(leader, size, None);
    let dot_w = dot.max_intrinsic_width().max(1.0);
    let gap = size * 0.25;
    let mut top = y;
    for line in t.display().split('\n') {
        let (left, right) = line.split_once('\t').unwrap_or((line, ""));
        let l = t.plain_paragraph(left, size, None);
        let r = t.plain_paragraph(right, size, None);
        let (lw, rw) = (l.max_intrinsic_width(), r.max_intrinsic_width());
        l.paint(canvas, (x, top));
        r.paint(canvas, (x + width - rw, top));
        let room = width - lw - rw - 2.0 * gap;
        if !right.is_empty() && room > dot_w {
            let n = (room / dot_w).floor() as usize;
            let dots = t.plain_paragraph(&leader.repeat(n), size, None);
            dots.paint(
                canvas,
                (x + width - rw - gap - dots.max_intrinsic_width(), top),
            );
        }
        top += l.height();
    }
    true
}

/// One line of text set along a circle of radius `r` px (∩ when positive,
/// ∪ when negative), centered in the box, painted with `paint`.
pub(super) fn curved(
    canvas: &skia_safe::Canvas,
    t: &Text,
    r: f32,
    rect: skia_safe::Rect,
    paint: &skia_safe::Paint,
) {
    let Some(run) = t.base_run() else { return };
    let Some(tf) = crate::text::typeface(t.family(), run.weight, run.italic) else {
        return;
    };
    let font = Font::from_typeface(tf, t.font_size());
    let text = t.display().replace('\n', " ");
    let glyphs = font.text_to_glyphs_vec(&text);
    let mut widths = vec![0.0; glyphs.len()];
    font.get_widths(&glyphs, &mut widths);
    let total: f32 = widths.iter().sum();
    let (_, m) = font.metrics();
    let radius = r.abs().max(1.0);
    let half = total / 2.0 / radius;
    let sag = radius - radius * half.cos();
    let cx = rect.center_x();
    let mut at = 0.0;
    let xforms: Vec<RSXform> = widths
        .iter()
        .map(|w| {
            let theta = (at + w / 2.0 - total / 2.0) / radius;
            at += w;
            let (s, c) = theta.sin_cos();
            // The glyph's baseline center on the circle, then back half its width.
            let (px, py, rot) = if r > 0.0 {
                let cy = rect.top - m.ascent + radius;
                (cx + radius * s, cy - radius * c, theta)
            } else {
                let cy = rect.top - m.ascent + sag - radius;
                (cx + radius * s, cy + radius * c, -theta)
            };
            let (rs, rc) = rot.sin_cos();
            RSXform::new(rc, rs, (px - rc * w / 2.0, py - rs * w / 2.0))
        })
        .collect();
    if let Some(blob) = TextBlob::from_rsxform(glyphs.as_slice(), &xforms, &font) {
        canvas.draw_text_blob(&blob, Point::new(0.0, 0.0), paint);
    }
}

/// The extra height curving adds: the arc's sagitta for the line's width.
pub fn curve_sagitta(total_width: f32, r: f32) -> f32 {
    let radius = r.abs().max(1.0);
    let half = (total_width / 2.0 / radius).min(std::f32::consts::FRAC_PI_2);
    radius - radius * half.cos()
}

/// A stable seed from a layer id, so rough edges repeat render to render.
fn seed(id: &str) -> u32 {
    id.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    })
}
