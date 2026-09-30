//! Stacks (auto layout), like CSS flexbox: children sized (`fill` shares
//! the free space, low `priority` gives way first), wrapped into lines,
//! then spread by `justify` and placed across by `align`. One `arrange`
//! pass serves both a hugging stack's size and the final placement.

use crate::scene::{Dir, Kind, Layer, Length, Position, Scene, Stack, StackAlign};
use crate::text::Text;

use super::flex::{baselines, grow, shrink, spread};
use super::measure::{Forced, measure};

/// A child's box inside the stack's inside (padding excluded).
pub(super) struct Item<'a> {
    /// The child.
    pub layer: &'a Layer,
    /// Left and top, relative to the stack's inside.
    pub pos: (f32, f32),
    /// Width and height.
    pub size: (f32, f32),
}

/// The result of one arrange pass in one direction.
pub(super) struct Arranged<'a> {
    /// Flow children, in drawing order.
    pub items: Vec<Item<'a>>,
    /// Width and height the content needs, padding excluded.
    pub content: (f32, f32),
    /// The direction used.
    pub dir: Dir,
    /// Whether the content fits without shrinking anything.
    pub fits: bool,
}

/// A flow child while it's being sized.
pub(super) struct Flex<'a> {
    pub(super) layer: &'a Layer,
    pub(super) index: usize,
    pub(super) main: f32,
    pub(super) cross: f32,
    pub(super) cross_forced: Option<f32>,
    pub(super) fill: bool,
    pub(super) fixed_main: bool,
    pub(super) min_main: f32,
    pub(super) max_main: f32,
    pub(super) align: StackAlign,
}

/// Size of a stack that hugs its content on its unknown sides; `known`
/// are the frame's own sides, padding included.
pub(super) fn hug(
    scene: &Scene,
    children: &[Layer],
    stack: &Stack,
    k: f32,
    known: Forced,
) -> (f32, f32) {
    let [t, r, b, l] = stack.padding.sides().map(|p| p * k);
    let inner = (known.0.map(|w| w - l - r), known.1.map(|h| h - t - b));
    let a = choose(scene, children, stack, k, inner);
    (
        known.0.unwrap_or(a.content.0 + l + r),
        known.1.unwrap_or(a.content.1 + t + b),
    )
}

/// Arranges a stack in the first of its directions that fits, else the last.
pub(super) fn choose<'a>(
    scene: &Scene,
    children: &'a [Layer],
    stack: &Stack,
    k: f32,
    inner: Forced,
) -> Arranged<'a> {
    let dirs = stack.dir.options();
    let mut last = None;
    for (i, d) in dirs.iter().enumerate() {
        let a = arrange(scene, children, stack, *d, k, inner);
        if a.fits || i + 1 == dirs.len() {
            return a;
        }
        last = Some(a);
    }
    // An empty list can't be written (serde rejects it); fall back to a row.
    last.unwrap_or_else(|| arrange(scene, children, stack, Dir::Row, k, inner))
}

/// Sizes and places the flow children for one direction.
fn arrange<'a>(
    scene: &Scene,
    children: &'a [Layer],
    stack: &Stack,
    dir: Dir,
    k: f32,
    inner: Forced,
) -> Arranged<'a> {
    let row = dir.is_row();
    let (inner_main, inner_cross) = if row { inner } else { (inner.1, inner.0) };
    let (gap, line_gap) = stack.gap.main_cross(dir);
    let (gap, line_gap) = (gap * k, line_gap * k);
    let parent = (inner.0.unwrap_or(0.0), inner.1.unwrap_or(0.0));
    let mut flow: Vec<Flex> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.hidden && c.position != Position::Absolute)
        .map(|(index, c)| {
            flex_item(
                scene,
                c,
                index,
                stack,
                row,
                k,
                parent,
                (inner_main, inner_cross),
            )
        })
        .collect();
    if dir.is_reverse() {
        flow.reverse();
    }
    let lines = wrap_lines(&flow, stack.wrap, inner_main, gap);
    let mut fits = true;
    let mut items = Vec::with_capacity(flow.len());
    let (mut cross_at, mut content_main, mut content_cross) = (0.0_f32, 0.0_f32, 0.0_f32);
    let n_lines = lines.len();
    for range in lines {
        let line = &mut flow[range];
        let used = |line: &[Flex]| {
            line.iter().map(|f| f.main).sum::<f32>() + gap * (line.len() as f32 - 1.0).max(0.0)
        };
        if let Some(m) = inner_main {
            let free = m - used(line);
            if free < -0.5 {
                fits = false;
            }
            if free > 0.0 {
                grow(line, free);
            } else if free < 0.0 {
                shrink(line, -free);
            }
        }
        // Rows: text re-wraps at its final width, so its height follows.
        for f in line.iter_mut() {
            if row && f.cross_forced.is_none() && !f.fixed_main {
                f.cross = measure(scene, f.layer, k, parent, (Some(f.main), None), true).1;
            }
        }
        // A single line spans the stack's inside; wrapped lines hug.
        let line_cross = match inner_cross {
            Some(c) if n_lines == 1 => c,
            _ => line.iter().map(|f| f.cross).fold(0.0, f32::max),
        };
        for f in line.iter_mut() {
            if f.align == StackAlign::Stretch && f.cross_forced.is_none() {
                f.cross = line_cross;
            }
        }
        let total = used(line);
        content_main = content_main.max(total);
        let free = inner_main.map_or(0.0, |m| (m - total).max(0.0));
        let (mut at, extra) = spread(stack.justify, free, line.len());
        let base = baselines(line, k, row);
        for f in line.iter() {
            let across = match f.align {
                StackAlign::Start | StackAlign::Stretch => 0.0,
                StackAlign::Center => (line_cross - f.cross) / 2.0,
                StackAlign::End => line_cross - f.cross,
                StackAlign::Baseline => base.as_ref().map_or(0.0, |(max, of)| max - of(f)),
            };
            let (x, y) = if row {
                (at, cross_at + across)
            } else {
                (cross_at + across, at)
            };
            let size = if row {
                (f.main, f.cross)
            } else {
                (f.cross, f.main)
            };
            items.push((
                f.index,
                Item {
                    layer: f.layer,
                    pos: (x, y),
                    size,
                },
            ));
            at += f.main + gap + extra;
        }
        let line_extent = line.iter().map(|f| f.cross).fold(0.0, f32::max);
        content_cross = cross_at + line_extent;
        cross_at = content_cross + line_gap;
    }
    if let Some(c) = inner_cross
        && content_cross > c + 0.5
    {
        fits = false;
    }
    // Draw in the children's own order, whatever the direction.
    items.sort_by_key(|(i, _)| *i);
    let content = if row {
        (content_main, content_cross)
    } else {
        (content_cross, content_main)
    };
    Arranged {
        items: items.into_iter().map(|(_, it)| it).collect(),
        content,
        dir,
        fits,
    }
}

/// Measures one flow child before any space is shared out.
#[expect(
    clippy::too_many_arguments,
    reason = "one call site; a struct would only rename them"
)]
fn flex_item<'a>(
    scene: &Scene,
    c: &'a Layer,
    index: usize,
    stack: &Stack,
    row: bool,
    k: f32,
    parent: (f32, f32),
    (inner_main, inner_cross): Forced,
) -> Flex<'a> {
    let (len_main, len_cross) = if row {
        (c.width, c.height)
    } else {
        (c.height, c.width)
    };
    let align = c.align_self.unwrap_or(stack.align);
    let align = if align == StackAlign::Baseline && !row {
        StackAlign::Start
    } else {
        align
    };
    let cross_forced = match len_cross {
        Some(Length::Px(v)) => Some(v * k),
        Some(Length::Pct(p)) => inner_cross.map(|m| p * m),
        Some(Length::Fill) => inner_cross,
        _ if align == StackAlign::Stretch => inner_cross,
        _ => None,
    };
    let main_forced = match len_main {
        Some(Length::Px(v)) => Some(v * k),
        Some(Length::Pct(p)) => inner_main.map(|m| p * m),
        _ => None,
    };
    let spacer = match c.kind {
        Kind::Spacer { min_length } => Some(min_length * k),
        _ => None,
    };
    // `flexGrow` makes it fill: from nothing with no size along the stack,
    // else from that size, CSS's flex-basis (`width: 0, flexGrow: 1`
    // shares a row evenly).
    let grows = c.grow.is_some_and(|g| g > 0.0)
        && matches!(len_main, None | Some(Length::Px(_) | Length::Pct(_)));
    let fill =
        (len_main == Some(Length::Fill) || spacer.is_some() || grows) && inner_main.is_some();
    let (min_side, max_side) = if row {
        (c.min_width, c.max_width)
    } else {
        (c.min_height, c.max_height)
    };
    let forced = if row {
        (main_forced, cross_forced)
    } else {
        (cross_forced, main_forced)
    };
    let natural = measure(scene, c, k, parent, forced, true);
    let (nat_main, cross) = if row { natural } else { (natural.1, natural.0) };
    // Like CSS's `min-height: auto`: in a column, text and frames sized by
    // their content keep its height; text in a row gives way down to its
    // longest word. A `fill` or empty frame can shrink to nothing.
    let by_content = matches!(len_main, None | Some(Length::Hug));
    let min_main = spacer
        .or(min_side.map(|v| v * k))
        .unwrap_or_else(|| match (&c.kind, row) {
            (Kind::Text { .. }, true) => Text::of(c, k).map_or(0.0, |t| t.min_width()),
            (Kind::Text { .. }, false) if by_content => nat_main,
            (Kind::Frame { children, .. }, false) if by_content && !children.is_empty() => nat_main,
            _ => 0.0,
        });
    Flex {
        layer: c,
        index,
        main: if fill {
            main_forced.unwrap_or(0.0).max(min_main)
        } else {
            nat_main.max(min_main)
        },
        cross,
        cross_forced,
        fill,
        fixed_main: main_forced.is_some() && !grows,
        min_main,
        max_main: max_side.map_or(f32::MAX, |v| v * k),
        align,
    }
}

/// Splits the flow into lines: one, unless `wrap` and a known main size.
fn wrap_lines(
    flow: &[Flex],
    wrap: bool,
    inner_main: Option<f32>,
    gap: f32,
) -> Vec<std::ops::Range<usize>> {
    let Some(m) = inner_main.filter(|_| wrap) else {
        return std::iter::once(0..flow.len()).collect();
    };
    let mut lines = Vec::new();
    let (mut start, mut used) = (0, 0.0_f32);
    for (i, f) in flow.iter().enumerate() {
        let next = if i == start {
            f.main
        } else {
            used + gap + f.main
        };
        if i > start && next > m + 0.5 {
            lines.push(start..i);
            start = i;
            used = f.main;
        } else {
            used = next;
        }
    }
    lines.push(start..flow.len());
    lines
}
