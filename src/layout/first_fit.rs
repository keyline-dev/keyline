//! `firstFit`: of its children, only the first that fits is laid out and
//! drawn, like SwiftUI's `ViewThatFits`.

use crate::scene::{Kind, Layer, Scene};

use super::{Placed, Rect};
use crate::text::Text;

use super::measure::{Forced, measure};
use super::stack;

/// The chosen child's size and index. `known` are the firstFit's own sides;
/// an open side never limits the choice.
pub(super) fn content(
    scene: &Scene,
    children: &[Layer],
    k: f32,
    parent: (f32, f32),
    known: Forced,
) -> ((f32, f32), usize) {
    let inside = (known.0.unwrap_or(parent.0), known.1.unwrap_or(parent.1));
    let visible: Vec<(usize, &Layer)> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.hidden)
        .collect();
    let mut last = ((0.0, 0.0), 0);
    for (i, c) in &visible {
        let size = measure(scene, c, k, inside, (None, None), false);
        last = (size, *i);
        let within = |v: f32, limit: Option<f32>| limit.is_none_or(|l| v <= l + 0.5);
        if within(size.0, known.0) && within(size.1, known.1) && fits_inside(scene, c, k, size) {
            return last;
        }
    }
    last
}

/// Whether a child fits its own box with nothing wrong inside it: no text
/// anywhere in it shrunk, overflowing or cut, no stack squeezed or
/// overflowing.
fn fits_inside(scene: &Scene, c: &Layer, k: f32, size: (f32, f32)) -> bool {
    if let Kind::Frame {
        children,
        layout: crate::scene::FrameLayout { stack: Some(s), .. },
        ..
    } = &c.kind
    {
        let [t, r, b, l] = s.padding.sides().map(|p| p * k);
        let inner = (Some(size.0 - l - r), Some(size.1 - t - b));
        if !stack::choose(scene, children, s, k, inner).fits {
            return false;
        }
    }
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        w: size.0,
        h: size.1,
    };
    sound(&super::finish(scene, c, size, rect, k, false))
}

/// No text in `p` or below it is shrunk, overflows or is cut, and no
/// stack overflows.
fn sound(p: &Placed) -> bool {
    let text_ok = p.text.as_ref().is_none_or(|(_, fit)| {
        let full = Text::of(p.layer, p.k).map_or(0.0, |t| t.font_size());
        !fit.overflow && !fit.truncated && fit.font_size >= full - 0.01
    });
    p.overflow.is_none() && text_ok && p.children.iter().all(sound)
}
