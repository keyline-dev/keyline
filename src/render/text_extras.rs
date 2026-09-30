//! Text extras drawn around or instead of the paragraph: highlight boxes
//! behind lines or spans, menu dot leaders, and text set along an arc.

use skia_safe::{
    Font, PathEffect, Point, RSXform, TextBlob,
    textlayout::{Paragraph, RectHeightStyle, RectWidthStyle},
};

use super::paint::sk_color;
use crate::layout::Placed;
use crate::scene::{HighlightStyle, Kind, Layer};
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
/// flush right, each in its own markup, sharing a baseline, and the leader
/// character filling the gap. Returns false when the text has no tab (the
/// caller draws it normally).
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
    let mut top = y;
    for line in leader_lines(t, fit, leader, width) {
        let base = top + line.baseline;
        line.left
            .paint(canvas, (x, base - line.left.alphabetic_baseline()));
        line.right.paint(
            canvas,
            (
                x + width - line.right.max_intrinsic_width(),
                base - line.right.alphabetic_baseline(),
            ),
        );
        if let Some((dots, at)) = &line.dots {
            dots.paint(canvas, (x + at, base - dots.alphabetic_baseline()));
        }
        top += line.height;
    }
    true
}

/// The lines of leader text `p` whose two parts run into each other:
/// `(left, right)` as written, for the `!leader … meets …` defect.
pub fn leader_clashes(p: &Placed) -> Vec<(String, String)> {
    let Kind::Text { more, .. } = &p.layer.kind else {
        return Vec::new();
    };
    let (Some(leader), Some((_, fit)), Some(t)) =
        (&more.leader, &p.text, Text::drawn(p.layer, p.k))
    else {
        return Vec::new();
    };
    if !t.display().contains('\t') || leader.is_empty() {
        return Vec::new();
    }
    leader_lines(&t, fit, leader, fit.wrap_width)
        .iter()
        .zip(t.display().split('\n'))
        .filter(|(l, text)| text.contains('\t') && l.room < l.gap)
        .map(|(_, text)| {
            let (left, right) = text.split_once('\t').unwrap_or((text, ""));
            (left.to_owned(), right.to_owned())
        })
        .collect()
}

/// One line of a leader text, laid out.
struct LeaderLine {
    /// The part before the tab.
    left: Paragraph,
    /// The part after it.
    right: Paragraph,
    /// The leader's run and where it starts, px from the left; none when
    /// there's no room for one.
    dots: Option<(Paragraph, f32)>,
    /// The shared baseline, px from the line's top.
    baseline: f32,
    /// The line's height, px.
    height: f32,
    /// Room between the two parts, px: under the leader's gap, they meet.
    room: f32,
    /// The gap kept on each side of the leader, px.
    gap: f32,
}

/// Each line of leader text `t` (each `\n`-separated part) in a box
/// `width` px wide.
fn leader_lines(t: &Text, fit: &Fit, leader: &str, width: f32) -> Vec<LeaderLine> {
    let size = fit.font_size;
    let gap = size * 0.25;
    let width_of = |n: usize| {
        t.plain_paragraph(&leader.repeat(n), size, None)
            .max_intrinsic_width()
    };
    let one = width_of(1).max(1.0);
    // Kerning can set a run of dots wider or narrower than one dot × n.
    let step = (width_of(2) - one).max(1.0);
    let display = t.display();
    let mut out = Vec::new();
    let mut start = 0;
    for line in display.split('\n') {
        let end = start + line.len();
        let tab = line.find('\t').map_or(end, |i| start + i);
        let left = t.slice_paragraph(start..tab, size);
        let right = t.slice_paragraph((tab + 1).min(end)..end, size);
        let (lw, rw) = (left.max_intrinsic_width(), right.max_intrinsic_width());
        let room = width - lw - rw;
        let space = room - 2.0 * gap;
        let dots = (tab + 1 < end && space >= one).then(|| {
            let mut n = ((space - one) / step).floor() as usize + 1;
            let mut dots = t.plain_paragraph(&leader.repeat(n), size, None);
            while n > 1 && dots.max_intrinsic_width() > space {
                n -= 1;
                dots = t.plain_paragraph(&leader.repeat(n), size, None);
            }
            let at = width - rw - gap - dots.max_intrinsic_width();
            (dots, at)
        });
        let parts = [Some(&left), Some(&right), dots.as_ref().map(|d| &d.0)];
        let baseline = parts
            .iter()
            .flatten()
            .map(|p| p.alphabetic_baseline())
            .fold(0.0, f32::max);
        let below = parts
            .iter()
            .flatten()
            .map(|p| p.height() - p.alphabetic_baseline())
            .fold(0.0, f32::max);
        out.push(LeaderLine {
            left,
            right,
            dots,
            baseline,
            height: baseline + below,
            room,
            gap,
        });
        start = end + 1;
    }
    out
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
