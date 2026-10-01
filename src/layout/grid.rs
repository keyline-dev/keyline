//! Grids, like CSS grid: children placed in cells (by area, by cell, or
//! the next free one), tracks sized (px, %, `auto` to the content, `fr`
//! sharing what's left), then each child stretched to its cell.

use crate::scene::{Area, Dir, Grid, Layer, Length, MAX_TRACKS, Position, Scene, Track, Tracks};

use super::measure::{Forced, clamp, measure};
use super::stack::Item;

/// A grid's children in their cells, and the content's size (padding
/// excluded). `inner` are the known sides of the grid's inside.
pub(super) fn arrange<'a>(
    scene: &Scene,
    children: &'a [Layer],
    grid: &Grid,
    k: f32,
    inner: Forced,
) -> (Vec<Item<'a>>, (f32, f32)) {
    let (col_gap, row_gap) = grid.gap.main_cross(Dir::Row);
    let (col_gap, row_gap) = (col_gap * k, row_gap * k);
    let flow: Vec<&Layer> = children
        .iter()
        .filter(|c| !c.hidden && c.position != Position::Absolute)
        .collect();
    let mut cols = match &grid.columns {
        Some(Tracks::Fit { min }) => {
            // As many as fit, no more than there are children (CSS auto-fit
            // collapses the empty ones); hugging, one per child.
            let n = inner.0.map_or(flow.len(), |w| {
                ((w + col_gap) / (min * k + col_gap)).floor().max(1.0) as usize
            });
            vec![Track::Fr(1.0); n.min(flow.len()).clamp(1, MAX_TRACKS)]
        }
        Some(Tracks::List(t)) => t.clone(),
        None => vec![Track::Fr(1.0); grid.area_columns().max(1)],
    };
    let rows_given = grid.rows.as_ref().map_or(&[][..], Tracks::list);
    let slots = place(&flow, grid, cols.len());
    let n_cols = slots.iter().map(|s| s.1 + s.3).max().unwrap_or(0);
    cols.resize(cols.len().max(n_cols), Track::Auto);
    let n_rows = slots
        .iter()
        .map(|s| s.0 + s.2)
        .chain([rows_given.len(), grid.areas.len()])
        .max()
        .unwrap_or(0);
    let mut rows = rows_given.to_vec();
    rows.resize(n_rows, Track::Auto);
    let parent = (inner.0.unwrap_or(0.0), inner.1.unwrap_or(0.0));
    let mut col_w = sizes(&cols, inner.0, col_gap, k, |i| {
        flow.iter()
            .zip(&slots)
            .filter(|(_, s)| s.1 == i && s.3 == 1)
            .map(|(c, _)| measure(scene, c, k, parent, (None, None), true).0)
            .fold(0.0, f32::max)
    });
    let span = |sizes: &[f32], at: usize, n: usize, gap: f32| {
        sizes[at..at + n].iter().sum::<f32>() + gap * (n - 1) as f32
    };
    for (c, s) in flow.iter().zip(&slots).filter(|(_, s)| s.3 > 1) {
        let need = measure(scene, c, k, parent, (None, None), true).0;
        widen(&mut col_w, &cols, (s.1, s.3), col_gap, need);
    }
    // A row is as tall as its items at the width they're drawn: their own
    // px width, else the cell's.
    let drawn_width = |c: &Layer, cell: f32| c.width.and_then(Length::px).map_or(cell, |v| v * k);
    let mut row_h = sizes(&rows, inner.1, row_gap, k, |i| {
        flow.iter()
            .zip(&slots)
            .filter(|(_, s)| s.0 == i && s.2 == 1)
            .map(|(c, s)| {
                let w = drawn_width(c, span(&col_w, s.1, s.3, col_gap));
                measure(scene, c, k, parent, (Some(w), None), true).1
            })
            .fold(0.0, f32::max)
    });
    // An item spanning rows needs their sum: what's missing goes to the
    // `auto` rows it spans, as in CSS.
    for (c, s) in flow.iter().zip(&slots).filter(|(_, s)| s.2 > 1) {
        let w = drawn_width(c, span(&col_w, s.1, s.3, col_gap));
        let need = measure(scene, c, k, parent, (Some(w), None), true).1;
        widen(&mut row_h, &rows, (s.0, s.2), row_gap, need);
    }
    let start =
        |sizes: &[f32], at: usize, gap: f32| sizes[..at].iter().sum::<f32>() + gap * at as f32;
    let items = flow
        .iter()
        .zip(&slots)
        .map(|(c, &(r, col, rs, cs))| {
            let cell = (span(&col_w, col, cs, col_gap), span(&row_h, r, rs, row_gap));
            // A px side keeps its size; anything else fills the cell.
            let own =
                |len: Option<Length>, cell: f32| len.and_then(Length::px).map_or(cell, |v| v * k);
            Item {
                layer: c,
                pos: (start(&col_w, col, col_gap), start(&row_h, r, row_gap)),
                size: clamp(c, k, (own(c.width, cell.0), own(c.height, cell.1))),
            }
        })
        .collect();
    let total = |sizes: &[f32], gap: f32| {
        sizes.iter().sum::<f32>() + gap * sizes.len().saturating_sub(1) as f32
    };
    (items, (total(&col_w, col_gap), total(&row_h, row_gap)))
}

/// Each child's slot: named areas and explicit cells first, then the rest
/// in order into the first free cells, row by row, in their given row or
/// column when only one is.
fn place(flow: &[&Layer], grid: &Grid, n_cols: usize) -> Vec<Area> {
    let spans = |l: &Layer| l.span().map(|n| usize::from(n.max(1)));
    let fixed: Vec<Option<Area>> = flow
        .iter()
        .map(|l| {
            let [rs, cs] = spans(l);
            l.area.as_deref().and_then(|a| grid.area(a)).or_else(|| {
                let [r, c] = l.cell();
                Some((line(r?), line(c?), rs, cs))
            })
        })
        .collect();
    let mut taken = Taken::default();
    for s in fixed.iter().flatten() {
        taken.take(*s);
    }
    let n_cols = n_cols.max(1);
    flow.iter()
        .zip(fixed)
        .map(|(l, f)| {
            f.unwrap_or_else(|| {
                let [rs, cs] = spans(l);
                let cs = cs.min(n_cols);
                let [row, col] = l.cell().map(|v| v.map(line));
                let rows = row.map_or(0..=usize::MAX, |r| r..=r);
                let cols = col.map_or(0..=n_cols - cs, |c| c..=c);
                // ponytail: first free cell from the top (CSS "dense"), not a moving cursor.
                let slot = rows
                    .flat_map(|r| cols.clone().map(move |c| (r, c, rs, cs)))
                    .find(|s| taken.free(*s))
                    .unwrap_or((row.unwrap_or(0), col.unwrap_or(0), rs, cs));
                taken.take(slot);
                slot
            })
        })
        .collect()
}

/// A grid line counted from 1, as an index from 0.
fn line(n: u16) -> usize {
    usize::from(n.max(1)) - 1
}

/// Which cells are taken, by row.
#[derive(Default)]
struct Taken(Vec<Vec<bool>>);

impl Taken {
    fn take(&mut self, (r, c, rs, cs): Area) {
        if self.0.len() < r + rs {
            self.0.resize(r + rs, Vec::new());
        }
        for row in &mut self.0[r..r + rs] {
            if row.len() < c + cs {
                row.resize(c + cs, false);
            }
            row[c..c + cs].fill(true);
        }
    }

    fn free(&self, (r, c, rs, cs): Area) -> bool {
        (r..r + rs).all(|row| {
            (c..c + cs).all(|col| {
                !self
                    .0
                    .get(row)
                    .and_then(|t| t.get(col))
                    .copied()
                    .unwrap_or(false)
            })
        })
    }
}

/// Grows the `auto` tracks among `n` from `at`, evenly, until they and
/// their gaps are `need` long.
fn widen(sizes: &mut [f32], tracks: &[Track], (at, n): (usize, usize), gap: f32, need: f32) {
    let have = sizes[at..at + n].iter().sum::<f32>() + gap * (n - 1) as f32;
    let autos: Vec<usize> = (at..at + n)
        .filter(|&i| matches!(tracks.get(i), Some(Track::Auto)))
        .collect();
    if need > have && !autos.is_empty() {
        let each = (need - have) / autos.len() as f32;
        for i in autos {
            sizes[i] += each;
        }
    }
}

/// Track sizes on one axis: px and % first, `auto` to its content, then
/// `fr` tracks share what's left of `avail`. Hugging (`avail` unknown), an
/// `fr` track sizes like `auto`.
fn sizes(
    tracks: &[Track],
    avail: Option<f32>,
    gap: f32,
    k: f32,
    content: impl Fn(usize) -> f32,
) -> Vec<f32> {
    let mut out: Vec<f32> = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| match (t, avail) {
            (Track::Px(v), _) => v * k,
            (Track::Pct(p), Some(a)) => p * a,
            (Track::Fr(_), Some(_)) => 0.0,
            _ => content(i),
        })
        .collect();
    let fr: f32 = tracks
        .iter()
        .map(|t| if let Track::Fr(f) = t { *f } else { 0.0 })
        .sum();
    if let Some(a) = avail.filter(|_| fr > 0.0) {
        let used: f32 = out.iter().sum::<f32>() + gap * tracks.len().saturating_sub(1) as f32;
        let left = (a - used).max(0.0);
        for (o, t) in out.iter_mut().zip(tracks) {
            if let Track::Fr(f) = t {
                *o = left * f / fr;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sizes;
    use crate::scene::Track;

    #[test]
    fn fr_tracks_share_what_px_pct_and_auto_leave() {
        let t = [
            Track::Px(100.0),
            Track::Fr(1.0),
            Track::Auto,
            Track::Fr(2.0),
            Track::Pct(0.1),
        ];
        // 1000 - 100 - 50 (auto) - 100 (10%) - 4 × 10 gaps = 710, split 1:2.
        let s = sizes(&t, Some(1000.0), 10.0, 1.0, |_| 50.0);
        assert_eq!(s, [100.0, 710.0 / 3.0, 50.0, 1420.0 / 3.0, 100.0]);
        // Hugging: fr and % size to the content.
        assert_eq!(
            sizes(&t, None, 10.0, 2.0, |i| i as f32),
            [200.0, 1.0, 2.0, 3.0, 4.0]
        );
    }
}
