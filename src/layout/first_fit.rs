//! `firstFit`: of its children, only the first that fits is laid out and
//! drawn, like SwiftUI's `ViewThatFits`.

use crate::scene::{Kind, Layer, Scene};
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

/// Whether a child fits its own box without shrinking or cutting anything:
/// text at its full font size with nothing cut, stacks without squeezing.
fn fits_inside(scene: &Scene, c: &Layer, k: f32, size: (f32, f32)) -> bool {
    match &c.kind {
        Kind::Text { .. } => Text::of(c, k).is_none_or(|t| {
            let (_, fit) = t.layout(size.0, size.1);
            !fit.overflow && !fit.truncated && fit.font_size >= t.font_size() - 0.01
        }),
        Kind::Frame {
            children,
            layout: crate::scene::FrameLayout { stack: Some(s), .. },
            ..
        } => {
            let [t, r, b, l] = s.padding.sides().map(|p| p * k);
            stack::choose(
                scene,
                children,
                s,
                k,
                (Some(size.0 - l - r), Some(size.1 - t - b)),
            )
            .fits
        }
        _ => true,
    }
}
