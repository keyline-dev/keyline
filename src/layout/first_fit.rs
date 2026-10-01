//! `firstFit`: of its children, only the first that fits is laid out and
//! drawn, like SwiftUI's `ViewThatFits`.

use crate::scene::{Kind, Layer, Scene};

use super::{Placed, Rect};
use crate::text::Text;

use super::measure::{Forced, measure};
use super::stack;

/// The chosen child's size and index, and why the first option was
/// skipped when it was (`headline 4L > maxLines 3`). `known` are the
/// firstFit's own sides; an open side never limits the choice.
pub(super) fn content(
    scene: &Scene,
    children: &[Layer],
    k: f32,
    parent: (f32, f32),
    known: Forced,
) -> ((f32, f32), usize, Option<String>) {
    let inside = (known.0.unwrap_or(parent.0), known.1.unwrap_or(parent.1));
    let visible: Vec<(usize, &Layer)> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.hidden)
        .collect();
    let mut last = ((0.0, 0.0), 0);
    let mut skipped = None;
    for (i, c) in &visible {
        let size = measure(scene, c, k, inside, (None, None), false);
        last = (size, *i);
        let within = |v: f32, limit: Option<f32>| limit.is_none_or(|l| v <= l + 0.5);
        // What's wrong inside it says more than its size: a text cut at
        // its maxLines needs the box it has, and more lines.
        let inner = fits_inside(scene, c, k, size).err();
        let why = if within(size.0, known.0) && within(size.1, known.1) {
            inner
        } else {
            inner.or_else(|| Some(format!("needs {}×{}", size.0.round(), size.1.round())))
        };
        match why {
            None => return (last.0, last.1, skipped),
            Some(why) if skipped.is_none() => {
                // The option itself needs no naming twice: `long: 4L > maxLines 3`.
                let why = why
                    .strip_prefix(&format!("{} ", c.id))
                    .map_or_else(|| why.clone(), str::to_owned);
                skipped = Some(format!("{}: {why}", c.id));
            }
            Some(_) => {}
        }
    }
    (last.0, last.1, skipped)
}

/// Whether a child fits its own box with nothing wrong inside it: no text
/// anywhere in it shrunk, overflowing or cut, no stack squeezed or
/// overflowing. The error says what's wrong, shortly.
fn fits_inside(scene: &Scene, c: &Layer, k: f32, size: (f32, f32)) -> Result<(), String> {
    if let Kind::Frame {
        children,
        layout: crate::scene::FrameLayout { stack: Some(s), .. },
        ..
    } = &c.kind
    {
        let [t, r, b, l] = s.padding.sides().map(|p| p * k);
        let inner = (Some(size.0 - l - r), Some(size.1 - t - b));
        if !stack::choose(scene, children, s, k, inner).fits {
            return Err(format!("{} overflows", c.id));
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
/// stack overflows; else the first that is.
fn sound(p: &Placed) -> Result<(), String> {
    if p.overflow.is_some() {
        return Err(format!("{} overflows", p.layer.id));
    }
    if let (Some((_, fit)), Kind::Text { max_lines, .. }) = (&p.text, &p.layer.kind) {
        let full = Text::of(p.layer, p.k).map_or(0.0, |t| t.font_size());
        let id = &p.layer.id;
        if let (true, Some(m)) = (fit.truncated, *max_lines)
            && fit.lines >= m
        {
            // Lines it would take uncut, from the height it needs.
            let per_line =
                (p.text.as_ref().map_or(0.0, |(para, _)| para.height()) / m as f32).max(1.0);
            let need = (fit.need_height / per_line).round().max(m as f32 + 1.0);
            return Err(format!("{id} {need}L > maxLines {m}"));
        }
        if fit.overflow || fit.truncated {
            return Err(format!("{id} cut"));
        }
        if fit.font_size < full - 0.01 {
            return Err(format!("{id} shrinks to {}px", fit.font_size.round()));
        }
    }
    p.children.iter().try_for_each(sound)
}
