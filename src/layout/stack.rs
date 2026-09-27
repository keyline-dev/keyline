//! Stacks (auto layout): children placed one after another, and the size
//! of a stack that hugs them.

use super::{Placed, Rect, finish, natural_size};
use crate::scene::{Dir, Justify, Layer, Scene, Stack, StackAlign};
use crate::text::Text;

/// Places a stack's children one after another inside `frame`.
pub(super) fn place_stack<'a>(
    scene: &'a Scene,
    children: &'a [Layer],
    stack: &Stack,
    frame: Rect,
    k: f32,
) -> Vec<Placed<'a>> {
    let row = stack.dir == Dir::Row;
    let (pad, mut gap) = (stack.padding * k, stack.gap * k);
    // (main, cross) of the frame's inside.
    let inner = if row {
        (frame.w - 2.0 * pad, frame.h - 2.0 * pad)
    } else {
        (frame.h - 2.0 * pad, frame.w - 2.0 * pad)
    };
    let sized: Vec<_> = children
        .iter()
        .map(|c| {
            let text = Text::of(c, k);
            let size = natural_size(scene, c, k, text.as_ref());
            (c, text, size)
        })
        .collect();
    let main_of = |(w, h): (f32, f32)| if row { w } else { h };
    let cross_of = |(w, h): (f32, f32)| if row { h } else { w };
    let n = sized.len() as f32;
    let used = sized.iter().map(|s| main_of(s.2)).sum::<f32>() + gap * (n - 1.0).max(0.0);
    let free = inner.0 - used;
    let mut at = match stack.justify {
        Justify::Start | Justify::Between => 0.0,
        Justify::Center => free / 2.0,
        Justify::End => free,
        Justify::Evenly => free / (n + 1.0),
    };
    match stack.justify {
        Justify::Between if n > 1.0 => gap += free / (n - 1.0),
        Justify::Evenly => gap += free / (n + 1.0),
        Justify::Start | Justify::Center | Justify::End | Justify::Between => {}
    }
    sized
        .into_iter()
        .map(|(c, text, size)| {
            let cross = match stack.align {
                StackAlign::Start => 0.0,
                StackAlign::Center => (inner.1 - cross_of(size)) / 2.0,
                StackAlign::End => inner.1 - cross_of(size),
            };
            let (dx, dy) = if row { (at, cross) } else { (cross, at) };
            at += main_of(size) + gap;
            let rect = Rect {
                x: frame.x + pad + dx,
                y: frame.y + pad + dy,
                w: size.0,
                h: size.1,
            };
            finish(scene, c, text, size, rect, k)
        })
        .collect()
}

// ponytail: hug and place_stack each measure the children, so nested
// hugging stacks re-measure per level; cache sizes if scenes get deep.
/// A stack's size when it hugs its children: `(main, cross)` summed and
/// maxed, plus gaps and padding, in scaled px.
pub(super) fn hug(scene: &Scene, children: &[Layer], stack: &Stack, k: f32) -> (f32, f32) {
    let sizes: Vec<_> = children
        .iter()
        .map(|c| natural_size(scene, c, k, Text::of(c, k).as_ref()))
        .collect();
    let row = stack.dir == Dir::Row;
    let (main, cross) = sizes.iter().fold((0.0_f32, 0.0_f32), |(m, c), &(w, h)| {
        if row {
            (m + w, c.max(h))
        } else {
            (m + h, c.max(w))
        }
    });
    let gaps = stack.gap * k * (sizes.len() as f32 - 1.0).max(0.0);
    let pad = 2.0 * stack.padding * k;
    let (main, cross) = (main + gaps + pad, cross + pad);
    if row { (main, cross) } else { (cross, main) }
}
