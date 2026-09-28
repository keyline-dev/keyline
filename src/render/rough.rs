//! Hand-made looks: torn edges and rough (hand-drawn) strokes. Both are
//! seeded, so the same scene draws the same way every time.

use skia_safe::{Path, PathBuilder, PathEffect};

use crate::scene::{Edges, Side};

/// The layer's box `r` with the chosen sides torn `depth` px deep (× `k`).
/// Each torn side is a zigzag of random steps cutting in, never out.
pub(super) fn torn(r: skia_safe::Rect, e: &Edges, k: f32) -> Path {
    let depth = e.depth.max(0.0) * k;
    let mut rng = Rng::new(e.seed);
    let tears = |s: Side| e.sides.is_empty() || e.sides.contains(&s);
    let corners = [
        (r.left, r.top),
        (r.right, r.top),
        (r.right, r.bottom),
        (r.left, r.bottom),
    ];
    // Clockwise: top, right, bottom, left; `inward` points into the box.
    let sides = [
        (Side::Top, (0.0, 1.0)),
        (Side::Right, (-1.0, 0.0)),
        (Side::Bottom, (0.0, -1.0)),
        (Side::Left, (1.0, 0.0)),
    ];
    let mut b = PathBuilder::new();
    b.move_to(corners[0]);
    for (i, (side, inward)) in sides.into_iter().enumerate() {
        let (from, to) = (corners[i], corners[(i + 1) % 4]);
        if tears(side) && depth > 0.0 {
            let len = (to.0 - from.0).hypot(to.1 - from.1);
            let mut t = 0.0;
            loop {
                t += depth * (0.4 + rng.next() * 0.8);
                if t >= len {
                    break;
                }
                let f = t / len;
                let d = depth * rng.next();
                b.line_to((
                    from.0 + (to.0 - from.0) * f + inward.0 * d,
                    from.1 + (to.1 - from.1) * f + inward.1 * d,
                ));
            }
        }
        b.line_to(to);
    }
    b.close();
    b.detach()
}

/// A path effect that wobbles a stroke up to `rough` px off its line,
/// smoothed so it reads as a pen, not a saw.
pub(super) fn rough_effect(rough: f32, seed: u32) -> Option<PathEffect> {
    let wobble = PathEffect::discrete((rough * 4.0).max(2.0), rough, seed)?;
    Some(match PathEffect::corner_path(rough * 2.0) {
        Some(smooth) => PathEffect::compose(smooth, wobble),
        None => wobble,
    })
}

/// A small, fixed pseudo-random sequence (xorshift32): the same on every
/// platform, unlike the standard library's hashers.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9) | 1)
    }

    /// The next value, 0–1.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{Rng, torn};
    use crate::scene::Edges;

    #[test]
    fn tears_cut_in_only_on_the_chosen_sides_and_repeat_by_seed() {
        let e: Edges =
            serde_json::from_value(serde_json::json!({"sides": ["top"], "depth": 10})).unwrap();
        let r = skia_safe::Rect::from_xywh(0.0, 0.0, 200.0, 100.0);
        let p = torn(r, &e, 1.0);
        let b = p.bounds();
        assert!(b.left >= 0.0 && b.right <= 200.0 && b.bottom <= 100.0);
        // Only the top has extra points; they sit within 10 px of it.
        let pts = p.points();
        assert!(pts.len() > 10, "{}", pts.len());
        assert!(pts.iter().all(|q| q.y <= 10.0 || q.y == 100.0), "{pts:?}");
        assert_eq!(torn(r, &e, 1.0), p, "the same seed tears the same way");
        let mut a = Rng::new(1);
        assert!((0..100).map(|_| a.next()).all(|v| (0.0..1.0).contains(&v)));
    }
}
