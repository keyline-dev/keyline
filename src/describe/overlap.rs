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
