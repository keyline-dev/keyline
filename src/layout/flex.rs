//! Sharing space along a stack line: `fill` growth, `priority` shrinking,
//! `justify` spacing, and baseline alignment.

use crate::scene::{Justify, Kind, StackAlign};

use super::stack::Flex;

/// Shares `free` among `fill` children by `grow`, as CSS resolves flexible
/// lengths: each grows from its basis, and one its share would take past
/// its minimum or maximum is held there while the rest share again.
pub(super) fn grow(line: &mut [Flex], free: f32) {
    let weight = |f: &Flex| f.layer.grow.unwrap_or(1.0).max(0.0);
    // The space the fill children share: theirs now, plus what's free.
    let space = free + line.iter().filter(|f| f.fill).map(|f| f.main).sum::<f32>();
    let mut open: Vec<bool> = line.iter().map(|f| f.fill && weight(f) > 0.0).collect();
    for _ in 0..line.len() {
        let held: f32 = line
            .iter()
            .zip(&open)
            .filter(|(f, o)| f.fill && !**o)
            .map(|(f, _)| f.main)
            .sum();
        let (basis, total): (f32, f32) = line
            .iter()
            .zip(&open)
            .filter(|(_, o)| **o)
            .fold((0.0, 0.0), |(b, w), (f, _)| (b + f.basis, w + weight(f)));
        if total <= 0.0 {
            return;
        }
        let left = space - held - basis;
        let want = |f: &Flex| f.basis + left * weight(f) / total;
        let off: f32 = line
            .iter()
            .zip(&open)
            .filter(|(_, o)| **o)
            .map(|(f, _)| want(f).clamp(f.min_main, f.max_main.max(f.min_main)) - want(f))
            .sum();
        for (f, o) in line.iter_mut().zip(open.iter_mut()).filter(|(_, o)| **o) {
            let w = want(f);
            f.main = w.clamp(f.min_main, f.max_main.max(f.min_main));
            // Held at a minimum when the shares ran short, at a maximum
            // when they ran over; the others share again.
            let at_min = f.main > w + 0.01;
            let at_max = f.main < w - 0.01;
            if (off > 0.01 && at_min) || (off < -0.01 && at_max) {
                *o = false;
            }
        }
        if off.abs() <= 0.01 {
            return;
        }
    }
}

/// Takes `over` back from children that can give way, lowest `priority`
/// first, each down to its minimum, in proportion to what it can give.
pub(super) fn shrink(line: &mut [Flex], mut over: f32) {
    let mut levels: Vec<f32> = line.iter().map(|f| f.layer.priority).collect();
    levels.sort_by(f32::total_cmp);
    levels.dedup();
    for level in levels {
        let give = |f: &Flex| {
            let flexible = !f.fixed_main
                && matches!(
                    f.layer.kind,
                    Kind::Text { .. }
                        | Kind::Frame { .. }
                        | Kind::Spacer { .. }
                        | Kind::FirstFit { .. }
                );
            if flexible && f.layer.priority == level {
                (f.main - f.min_main).max(0.0)
            } else {
                0.0
            }
        };
        let room: f32 = line.iter().map(give).sum();
        if room <= 0.0 {
            continue;
        }
        let take = over.min(room);
        for f in line.iter_mut() {
            let g = give(f);
            f.main -= take * g / room;
        }
        over -= take;
        if over <= 0.01 {
            return;
        }
    }
}

/// Where the first child sits and the extra space after each, for `justify`.
pub(super) fn spread(justify: Justify, free: f32, n: usize) -> (f32, f32) {
    let n = n as f32;
    match justify {
        Justify::Start => (0.0, 0.0),
        Justify::Center => (free / 2.0, 0.0),
        Justify::End => (free, 0.0),
        Justify::Between if n > 1.0 => (0.0, free / (n - 1.0)),
        Justify::Between => (0.0, 0.0),
        Justify::Around => (free / (2.0 * n), free / n),
        Justify::Evenly => (free / (n + 1.0), free / (n + 1.0)),
    }
}

/// For baseline-aligned rows: the lowest baseline, and each child's own
/// (a text's first baseline as drawn; anything else sits on it by its bottom).
pub(super) fn baselines<'l>(
    line: &'l [Flex],
    k: f32,
    row: bool,
) -> Option<(f32, impl Fn(&Flex) -> f32 + 'l)> {
    if !row || !line.iter().any(|f| f.align == StackAlign::Baseline) {
        return None;
    }
    let of = move |f: &Flex| match &f.layer.kind {
        Kind::Text { .. } => super::text_baseline(f.layer, k, (f.main, f.cross)).unwrap_or(f.cross),
        _ => f.cross,
    };
    let max = line
        .iter()
        .filter(|f| f.align == StackAlign::Baseline)
        .map(&of)
        .fold(0.0, f32::max);
    Some((max, of))
}
