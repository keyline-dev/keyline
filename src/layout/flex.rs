//! Sharing space along a stack line: `fill` growth, `priority` shrinking,
//! `justify` spacing, and baseline alignment.

use crate::scene::{Justify, Kind, StackAlign};
use crate::text::Text;

use super::stack::Flex;

/// Shares `free` among `fill` children by `grow`, up to their maximums.
pub(super) fn grow(line: &mut [Flex], mut free: f32) {
    for _ in 0..line.len() {
        let weight: f32 = line
            .iter()
            .filter(|f| f.fill && f.main < f.max_main)
            .map(|f| f.layer.grow.max(0.0))
            .sum();
        if free <= 0.01 || weight <= 0.0 {
            return;
        }
        let mut given = 0.0;
        for f in line.iter_mut().filter(|f| f.fill && f.main < f.max_main) {
            let add = (free * f.layer.grow.max(0.0) / weight).min(f.max_main - f.main);
            f.main += add;
            given += add;
        }
        free -= given;
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
/// (a text's first baseline; anything else sits on it by its bottom).
pub(super) fn baselines<'l>(
    line: &'l [Flex],
    k: f32,
    row: bool,
) -> Option<(f32, impl Fn(&Flex) -> f32 + 'l)> {
    if !row || !line.iter().any(|f| f.align == StackAlign::Baseline) {
        return None;
    }
    let of = move |f: &Flex| match &f.layer.kind {
        Kind::Text { .. } => Text::of(f.layer, k).map_or(f.cross, |t| t.first_baseline(f.main)),
        _ => f.cross,
    };
    let max = line
        .iter()
        .filter(|f| f.align == StackAlign::Baseline)
        .map(&of)
        .fold(0.0, f32::max);
    Some((max, of))
}
