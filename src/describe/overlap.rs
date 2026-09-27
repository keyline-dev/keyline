//! Text ink and the `!overlaps` defect: texts whose glyphs collide.

use std::collections::HashMap;

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
    let mut para = text.repaint(fit, p.rect.w, &skia_safe::Paint::default(), false);
    let bounds = (0..para.line_number())
        .map(|line| *para.get_path_at(line).1.bounds())
        .filter(|b| !b.is_empty())
        .reduce(skia_safe::Rect::join2)?;
    Some(Rect {
        x: p.rect.x + bounds.left,
        y: p.text_top() + bounds.top,
        w: bounds.width(),
        h: bounds.height(),
    })
}

/// Pairs of texts whose ink overlaps by more than a pixel each way.
pub(super) fn overlaps<'s>(placed: &[Placed<'s>]) -> HashMap<&'s str, Vec<&'s str>> {
    fn collect<'s>(inks: &mut Vec<(&'s str, Rect)>, placed: &[Placed<'s>]) {
        for p in placed {
            if let Some(r) = ink(p) {
                inks.push((&p.layer.id, r));
            }
            collect(inks, &p.children);
        }
    }
    let mut inks = Vec::new();
    collect(&mut inks, placed);
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
