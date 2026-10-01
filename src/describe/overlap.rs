//! Text ink and the `!overlaps` defect: texts whose glyphs collide.

use std::collections::HashMap;

use skia_safe::Matrix;

use super::intersect;
use crate::layout::{Placed, Rect};
use crate::text::Text;

/// The box the glyphs actually cover, canvas coordinates: usually smaller
/// than the layer's box, and than its lines' ascent-to-descent height.
pub(super) fn ink(p: &Placed) -> Option<Rect> {
    let (_, fit) = p.text.as_ref()?;
    // Lay the text out again to read its glyph outlines: the real ink, not
    // the line box, whose ascent and descent space would make tightly set
    // lines look like they collide.
    let text = Text::of(p.layer, p.k)?;
    let mut para = text.repaint(fit, fit.wrap_width, &skia_safe::Paint::default(), false);
    let bounds = crate::text::line_paths(&mut para)
        .iter()
        .map(|line| *line.bounds())
        .filter(|b| !b.is_empty())
        .reduce(skia_safe::Rect::join2)?;
    let (x, y) = p.text_origin();
    Some(Rect {
        x: x + bounds.left,
        y: y + bounds.top,
        w: bounds.width(),
        h: bounds.height(),
    })
}

/// `p`'s transform after its ancestors' `m`: what its box, and its
/// children's, are drawn through.
pub(super) fn through(m: &Matrix, p: &Placed) -> Matrix {
    crate::render::matrix(p).map_or(*m, |own| Matrix::concat(m, &own))
}

/// `r` as drawn through `m`: the box around it.
pub(super) fn mapped(m: &Matrix, r: Rect) -> Rect {
    let (b, _) = m.map_rect(skia_safe::Rect::from_xywh(r.x, r.y, r.w, r.h));
    Rect {
        x: b.left,
        y: b.top,
        w: b.width(),
        h: b.height(),
    }
}

/// Whether a text's letters show: not at `opacity` 0, and not `fill: []`
/// with no stroke or outline (a layer drawn only for its highlight).
fn visible(p: &Placed) -> bool {
    let l = p.layer;
    let unfilled = l
        .look
        .fills
        .as_ref()
        .is_some_and(|f| f.as_slice().is_empty());
    let outlined = l
        .look
        .strokes
        .as_ref()
        .is_some_and(|s| !s.as_slice().is_empty())
        || matches!(
            &l.kind,
            crate::scene::Kind::Text {
                outline: Some(_),
                ..
            }
        );
    l.opacity > 0.0 && (!unfilled || outlined)
}

/// Pairs of texts whose ink overlaps by more than a pixel each way, where
/// they're drawn (scaled, rotated or moved).
pub(super) fn overlaps<'s>(placed: &[Placed<'s>]) -> HashMap<&'s str, Vec<&'s str>> {
    fn collect<'s>(inks: &mut Vec<(&'s str, Rect)>, placed: &[Placed<'s>], m: &Matrix) {
        for p in placed {
            let own = through(m, p);
            if let Some(r) = ink(p).filter(|_| visible(p)) {
                inks.push((&p.layer.id, mapped(&own, r)));
            }
            collect(inks, &p.children, &own);
        }
    }
    let mut inks = Vec::new();
    collect(&mut inks, placed, &Matrix::new_identity());
    // ponytail: O(n²) over texts; a sweep line if scenes grow to hundreds.
    let mut out: HashMap<&str, Vec<&str>> = HashMap::new();
    for (i, (a, ra)) in inks.iter().enumerate() {
        for (b, rb) in &inks[i + 1..] {
            if intersect(*ra, *rb).is_some_and(|x| x.w > 1.0 && x.h > 1.0) {
                out.entry(a).or_default().push(b);
                out.entry(b).or_default().push(a);
            }
        }
    }
    out
}

/// What a layer drawn over text can be: a layer with paint (a photo, a
/// shape, a filled frame), or another text's highlight boxes.
enum Over<'s> {
    Text(&'s str, Rect, Vec<Rect>),
    Paint(&'s str, Rect),
    /// A stroke-only shape: only its stroke covers anything.
    Outline(&'s str, skia_safe::Path),
}

/// Texts whose ink a later-drawn layer covers by more than a pixel each
/// way: a non-text layer with paint, or another text's highlight. Each
/// coverer once, with the share of the text's ink box it covers, largest
/// first: `toast 62%`, `name highlight 30%`. Text on text is `!overlaps`.
pub(super) fn covers<'s>(
    scene: &crate::scene::Scene,
    placed: &[Placed<'s>],
) -> HashMap<&'s str, Vec<(String, f32)>> {
    fn collect<'s>(order: &mut Vec<Over<'s>>, masks: &[String], placed: &[Placed<'s>], m: &Matrix) {
        for p in placed {
            let own = through(m, p);
            let l = p.layer;
            let shown = !l.hidden && l.opacity > 0.0 && !masks.contains(&l.id);
            if shown && let Some(ink) = ink(p).filter(|_| visible(p)) {
                order.push(Over::Text(
                    &l.id,
                    mapped(&own, ink),
                    highlight_boxes(p, &own),
                ));
            } else if shown && painted(p) {
                match outline_only(p)
                    .then(|| crate::render::stroke_ink(p))
                    .flatten()
                {
                    Some(ink) => order.push(Over::Outline(&l.id, ink.make_transform(&own))),
                    None => order.push(Over::Paint(&l.id, mapped(&own, p.rect))),
                }
            }
            if shown {
                collect(order, masks, &p.children, &own);
            }
        }
    }
    let mut masks = Vec::new();
    scene.walk(&mut |l| {
        if let Some(crate::scene::Mask {
            source: crate::scene::MaskSource::Layer(id),
            ..
        }) = &l.mask
        {
            masks.push(id.clone());
        }
    });
    let mut order = Vec::new();
    collect(&mut order, &masks, placed, &Matrix::new_identity());
    let share = |ink: Rect, r: Rect| {
        intersect(ink, r)
            .filter(|x| x.w > 1.0 && x.h > 1.0)
            .map(|x| x.w * x.h / (ink.w * ink.h).max(1.0))
    };
    let mut out: HashMap<&str, Vec<(String, f32)>> = HashMap::new();
    for (i, below) in order.iter().enumerate() {
        let Over::Text(id, ink, _) = below else {
            continue;
        };
        let mut by: Vec<(String, f32)> = Vec::new();
        for above in &order[i + 1..] {
            let hit = match above {
                Over::Paint(other, r) => share(*ink, *r).map(|s| ((*other).to_owned(), s)),
                Over::Outline(other, stroke) => {
                    // The share of the ink box the stroke takes, sampled:
                    // a diagonal crossing covers far less than its box.
                    const GRID: usize = 24;
                    let hits = (0..GRID * GRID)
                        .filter(|i| {
                            let x = ink.x + ink.w * ((i % GRID) as f32 + 0.5) / GRID as f32;
                            let y = ink.y + ink.h * ((i / GRID) as f32 + 0.5) / GRID as f32;
                            stroke.contains((x, y))
                        })
                        .count();
                    (hits > 0).then(|| ((*other).to_owned(), hits as f32 / (GRID * GRID) as f32))
                }
                Over::Text(other, _, boxes) => boxes
                    .iter()
                    .filter_map(|b| share(*ink, *b))
                    .reduce(f32::max)
                    .map(|s| (format!("{other} highlight"), s)),
            };
            if let Some((name, s)) = hit {
                match by.iter_mut().find(|b| b.0 == name) {
                    Some(b) => b.1 = b.1.max(s),
                    None => by.push((name, s)),
                }
            }
        }
        if !by.is_empty() {
            by.sort_by(|a, b| b.1.total_cmp(&a.1));
            out.insert(id, by);
        }
    }
    out
}

/// Whether a non-text layer puts paint on the canvas: a filled frame, a
/// shape, a photo, an icon. A frame without a fill, a spacer, or a layer
/// whose fills are all clear draws nothing of its own.
fn painted(p: &Placed) -> bool {
    use crate::scene::Kind;
    let fills = p
        .layer
        .look
        .fills
        .as_ref()
        .map(crate::scene::OneOrMany::as_slice);
    let clear = fills.is_some_and(|f| {
        f.iter()
            .all(|paint| matches!(paint, crate::scene::Paint::Solid(s) if s.color.0 >> 24 == 0))
    });
    let filled = fills.is_some_and(|f| !f.is_empty()) && !clear;
    let stroked = p
        .layer
        .look
        .strokes
        .as_ref()
        .is_some_and(|s| !s.as_slice().is_empty());
    match &p.layer.kind {
        Kind::Text { .. } | Kind::Spacer { .. } | Kind::FirstFit { .. } => false,
        Kind::Frame { .. } => filled,
        // A shape draws only the fill and stroke it's given.
        _ if shape(p) => filled || stroked,
        _ => true,
    }
}

/// Whether a shape draws only its stroke: no fill given, or clear ones.
fn outline_only(p: &Placed) -> bool {
    let l = &p.layer.look;
    // No fill given, or only clear ones.
    let unfilled = l.fills.as_ref().is_none_or(|f| {
        f.as_slice()
            .iter()
            .all(|paint| matches!(paint, crate::scene::Paint::Solid(s) if s.color.0 >> 24 == 0))
    });
    shape(p) && unfilled && l.strokes.as_ref().is_some_and(|s| !s.as_slice().is_empty())
}

/// A rect, ellipse, polygon or path: drawn only by its fill and stroke
/// (a line draws its own stroke).
fn shape(p: &Placed) -> bool {
    use crate::scene::Kind;
    matches!(
        p.layer.kind,
        Kind::Rect { .. } | Kind::Ellipse { .. } | Kind::Polygon { .. } | Kind::Path { .. }
    )
}

/// A text's highlight boxes where they're drawn, padding included.
fn highlight_boxes(p: &Placed, m: &Matrix) -> Vec<Rect> {
    let (Some((para, _)), Some(text)) = (&p.text, Text::of(p.layer, p.k)) else {
        return Vec::new();
    };
    let (x, y) = p.text_origin();
    text.highlights()
        .into_iter()
        .flat_map(|(range, h)| {
            let pad = h.padding * p.k;
            para.get_rects_for_range(
                range,
                skia_safe::textlayout::RectHeightStyle::Tight,
                skia_safe::textlayout::RectWidthStyle::Tight,
            )
            .into_iter()
            .map(move |b| {
                let r = b.rect.with_offset((x, y)).with_outset((pad, pad * 0.5));
                Rect {
                    x: r.left,
                    y: r.top,
                    w: r.width(),
                    h: r.height(),
                }
            })
        })
        .map(|r| mapped(m, r))
        .collect()
}

/// Texts whose ink comes closer to a neighbour's than 15% of the smaller
/// font size, touching included (short of the overlap `!overlaps` reports), one above the other or side by side (caps
/// that overshoot a tight `lineHeight`): the upper or left text, the gap
/// in px and the neighbour, the closest one per text.
pub(super) fn tight<'s>(placed: &[Placed<'s>]) -> HashMap<&'s str, (f32, &'s str)> {
    fn collect<'s>(inks: &mut Vec<(&'s str, Rect, f32)>, placed: &[Placed<'s>], m: &Matrix) {
        for p in placed {
            let own = through(m, p);
            if let (Some(r), Some((_, fit))) = (ink(p).filter(|_| visible(p)), &p.text) {
                inks.push((&p.layer.id, mapped(&own, r), fit.font_size));
            }
            collect(inks, &p.children, &own);
        }
    }
    let mut inks = Vec::new();
    collect(&mut inks, placed, &Matrix::new_identity());
    let mut out: HashMap<&str, (f32, &str)> = HashMap::new();
    // ponytail: O(n²) over texts, as `overlaps`.
    for (i, (a, ra, pa)) in inks.iter().enumerate() {
        for (b, rb, pb) in &inks[i + 1..] {
            let floor = 0.15 * pa.min(*pb);
            let span = |a0: f32, a1: f32, b0: f32, b1: f32| a1.min(b1) - a0.max(b0);
            // One above the other: they share some width; else side by side.
            let (gap, first) = if span(ra.x, ra.right(), rb.x, rb.right()) > 1.0 {
                if ra.y <= rb.y {
                    (rb.y - ra.bottom(), true)
                } else {
                    (ra.y - rb.bottom(), false)
                }
            } else if span(ra.y, ra.bottom(), rb.y, rb.bottom()) > 1.0 {
                if ra.x <= rb.x {
                    (rb.x - ra.right(), true)
                } else {
                    (ra.x - rb.right(), false)
                }
            } else {
                continue;
            };
            // Touching, or overlapping by up to the pixel `!overlaps` lets
            // pass, counts as no gap.
            if !(-1.0..floor).contains(&gap) {
                continue;
            }
            let gap = gap.max(0.0);
            let (who, other) = if first { (*a, *b) } else { (*b, *a) };
            if out.get(who).is_none_or(|(g, _)| gap < *g) {
                out.insert(who, (gap, other));
            }
        }
    }
    out
}
