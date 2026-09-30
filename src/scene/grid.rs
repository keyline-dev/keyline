//! Grid layout settings for frames, like CSS grid.

use serde::{Deserialize, Serialize};

use super::{Gap, Padding};

/// A frame that places its children in rows and columns, like CSS grid.
/// Children take `area`, or `cell` and `span`, else fill the next free cell
/// row by row. They stretch to their cell unless they have a px size, then
/// they sit at its top-left. Without a `width` or `height`, the frame hugs.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// Column tracks, CSS style: `"1fr 2fr"`, `"200px auto 25%"`,
    /// `"repeat(3, 1fr)"`; or `{"min": 160}`, as many equal columns as fit
    /// at least that wide. Default: one `1fr` per `areas` column, else one.
    pub columns: Option<Tracks>,
    /// Row tracks, as `columns`; rows beyond them are `auto` (default).
    pub rows: Option<Tracks>,
    /// Space between cells, px; `[rowGap, columnGap]` (default 0).
    pub gap: Gap,
    /// Space inside the frame's edges, as a stack's (default 0).
    pub padding: Padding,
    /// Named areas, one string per row, a name per column; `.` is empty:
    /// `["hero hero side", "cta cta side"]`. Each name is a rectangle.
    pub areas: Vec<String>,
}

/// One track's size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// Fixed, px.
    Px(f32),
    /// A share of the grid's inside, 0–1.
    Pct(f32),
    /// A share of the space left after the other tracks.
    Fr(f32),
    /// As big as its largest single-cell child.
    Auto,
}

/// A grid's tracks on one axis.
#[derive(Debug, Clone, PartialEq)]
pub enum Tracks {
    /// These tracks, in order.
    List(Vec<Track>),
    /// As many `1fr` tracks as fit, each at least `min` px.
    Fit {
        /// Smallest track, px.
        min: f32,
    },
}

/// Most tracks a grid may have on one axis, and the largest `cell` or
/// `span`: far past any real layout, and it bounds the memory a scene can ask for.
pub const MAX_TRACKS: usize = 100;

impl Tracks {
    /// The listed tracks (none for `None` and `Fit`).
    pub fn list(&self) -> &[Track] {
        match self {
            Tracks::List(t) => t,
            _ => &[],
        }
    }
}

/// Parses CSS track syntax: `px`, `%`, `fr`, `auto`, bare numbers (px) and
/// `repeat(n, tracks)`.
fn parse_tracks(s: &str) -> Result<Vec<Track>, String> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("repeat(") {
            let end = r.find(')').ok_or("repeat( needs a closing )")?;
            let (n, inner) = r[..end]
                .split_once(',')
                .ok_or("repeat needs (count, tracks)")?;
            let n: usize = n
                .trim()
                .parse()
                .ok()
                .filter(|n| *n <= MAX_TRACKS)
                .ok_or_else(|| format!("bad repeat count {n}: 0–{MAX_TRACKS}"))?;
            let inner = parse_tracks(inner)?;
            for _ in 0..n {
                out.extend_from_slice(&inner);
            }
            rest = r[end + 1..].trim_start();
            continue;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (word, r) = rest.split_at(end);
        let num = |suffix: &str| {
            word.strip_suffix(suffix)
                .and_then(|n| n.parse::<f32>().ok())
                .filter(|n| *n >= 0.0)
        };
        out.push(if word == "auto" {
            Track::Auto
        } else if let Some(n) = num("fr") {
            Track::Fr(n)
        } else if let Some(n) = num("%") {
            Track::Pct(n / 100.0)
        } else if let Some(n) = num("px").or_else(|| num("")) {
            Track::Px(n)
        } else {
            return Err(format!("bad track {word}; use px, %, fr, auto or repeat()"));
        });
        rest = r.trim_start();
    }
    if out.len() > MAX_TRACKS {
        return Err(format!("at most {MAX_TRACKS} tracks"));
    }
    Ok(out)
}

impl<'de> Deserialize<'de> for Tracks {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let what =
            "CSS tracks like \"1fr 200px auto\" or \"repeat(auto-fill, minmax(160px, 1fr))\"";
        match &v {
            serde_json::Value::String(s) => match auto_fill(s) {
                Some(min) => Ok(Tracks::Fit { min }),
                None => parse_tracks(s)
                    .map(Tracks::List)
                    .map_err(serde::de::Error::custom),
            },
            _ => Err(super::de::expected(what, &v)),
        }
    }
}

impl Serialize for Tracks {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Tracks::Fit { min } => {
                s.serialize_str(&format!("repeat(auto-fill, minmax({min}px, 1fr))"))
            }
            Tracks::List(t) => {
                let words: Vec<String> = t
                    .iter()
                    .map(|t| match t {
                        Track::Px(v) => format!("{v}px"),
                        Track::Pct(v) => format!("{}%", v * 100.0),
                        Track::Fr(v) => format!("{v}fr"),
                        Track::Auto => "auto".into(),
                    })
                    .collect();
                s.serialize_str(&words.join(" "))
            }
        }
    }
}

/// The smallest width in `repeat(auto-fill, minmax(160px, 1fr))` (or
/// `auto-fit`): as many equal tracks as fit, each at least that wide.
fn auto_fill(s: &str) -> Option<f32> {
    let inner = s.trim().strip_prefix("repeat(")?.strip_suffix(')')?;
    let (count, size) = inner.split_once(',')?;
    if !matches!(count.trim(), "auto-fill" | "auto-fit") {
        return None;
    }
    let min = size.trim().strip_prefix("minmax(")?.split(',').next()?;
    min.trim()
        .strip_suffix("px")?
        .trim()
        .parse()
        .ok()
        .filter(|m| *m > 0.0)
}

/// A named area's cells: `(row, column, rows, columns)`, 0-based.
pub type Area = (usize, usize, usize, usize);

impl Grid {
    /// The `areas` template as rows of names.
    fn template(&self) -> Vec<Vec<&str>> {
        self.areas
            .iter()
            .map(|r| r.split_whitespace().collect())
            .collect()
    }

    /// Columns in the `areas` template.
    pub fn area_columns(&self) -> usize {
        self.template().first().map_or(0, Vec::len)
    }

    /// Where the area `name` sits, if the template has it.
    pub fn area(&self, name: &str) -> Option<Area> {
        let t = self.template();
        let cells = t.iter().enumerate().flat_map(|(r, row)| {
            row.iter()
                .enumerate()
                .filter(|(_, n)| **n == name)
                .map(move |(c, _)| (r, c))
        });
        let (mut r0, mut c0, mut r1, mut c1) = (usize::MAX, usize::MAX, 0, 0);
        for (r, c) in cells {
            (r0, c0, r1, c1) = (r0.min(r), c0.min(c), r1.max(r), c1.max(c));
        }
        (r0 != usize::MAX).then(|| (r0, c0, r1 - r0 + 1, c1 - c0 + 1))
    }

    /// Checks gaps, padding and that `areas` is a grid of rectangles.
    ///
    /// # Errors
    /// What's wrong, in one line.
    pub fn check(&self) -> Result<(), String> {
        if self.gap.is_negative() || self.padding.sides().iter().any(|p| *p < 0.0) {
            return Err("grid gap and padding must be >= 0".into());
        }
        let t = self.template();
        if t.iter().any(|r| r.len() != self.area_columns()) {
            return Err("grid areas: every row needs the same number of names".into());
        }
        for name in t.iter().flatten().filter(|n| **n != ".") {
            let (r, c, rs, cs) = self.area(name).unwrap_or_default();
            let whole = (r..r + rs).all(|r| (c..c + cs).all(|c| t[r][c] == *name));
            if !whole {
                return Err(format!("grid area {name} isn't a rectangle"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Grid, Track, Tracks};
    use crate::scene::FrameLayout;
    use serde_json::json;

    fn grid(v: serde_json::Value) -> Result<Grid, serde_json::Error> {
        serde_json::from_value::<FrameLayout>(v)
            .map(|l| l.grid.unwrap_or_else(|| panic!("not a grid")))
    }

    #[test]
    fn tracks_read_css_syntax() {
        let g = grid(
            json!({"gridTemplateColumns": "200px repeat(2, 1fr) auto 25% 10",
            "gridTemplateRows": "repeat(2, 1fr)"}),
        )
        .unwrap();
        assert_eq!(
            g.columns.as_ref().unwrap().list(),
            [
                Track::Px(200.0),
                Track::Fr(1.0),
                Track::Fr(1.0),
                Track::Auto,
                Track::Pct(0.25),
                Track::Px(10.0)
            ]
        );
        assert_eq!(
            g.rows.as_ref().unwrap().list(),
            [Track::Fr(1.0), Track::Fr(1.0)]
        );
        let l = FrameLayout {
            grid: Some(Grid { rows: None, ..g }),
            ..FrameLayout::default()
        };
        let v = serde_json::to_value(&l).unwrap();
        assert_eq!(
            v,
            json!({"gridTemplateColumns": "200px 1fr 1fr auto 25% 10px"})
        );
        let fit = json!({"gridTemplateColumns": "repeat(auto-fill, minmax(160px, 1fr))"});
        let g = grid(fit.clone()).unwrap();
        assert_eq!(g.columns, Some(Tracks::Fit { min: 160.0 }));
        let l = FrameLayout {
            grid: Some(g),
            ..FrameLayout::default()
        };
        assert_eq!(serde_json::to_value(&l).unwrap(), fit);
        let e = grid(json!({"gridTemplateColumns": "1fr 2em"})).unwrap_err();
        assert!(e.to_string().contains("bad track 2em"), "{e}");
        for huge in [
            json!({"gridTemplateColumns": "repeat(100000, 1fr)"}),
            json!({"gridTemplateColumns": "repeat(60, 1fr) repeat(60, 1fr)"}),
            json!({"gridTemplateColumns": 3}),
        ] {
            assert!(serde_json::from_value::<FrameLayout>(huge).is_err());
        }
    }

    #[test]
    fn areas_are_rectangles() {
        let g = grid(json!({"gridTemplateAreas": ["hero hero side", "cta cta side"]})).unwrap();
        g.check().unwrap();
        assert_eq!(g.area("hero"), Some((0, 0, 1, 2)));
        assert_eq!(g.area("side"), Some((0, 2, 2, 1)));
        assert_eq!(g.area("nope"), None);
        let bad = grid(json!({"gridTemplateAreas": ["a a", "a b"]})).unwrap();
        assert_eq!(bad.check().unwrap_err(), "grid area a isn't a rectangle");
        let bad = grid(json!({"gridTemplateAreas": ["a a", "b"]})).unwrap();
        assert!(bad.check().unwrap_err().contains("same number"));
    }
}

/// Where a grid child sits on one axis, CSS `grid-row` / `grid-column`:
/// `2`, `"1 / span 2"`, `"1 / 3"` or `"span 2"`, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridLine {
    /// The first line, from 1; the next free cell when absent.
    pub start: Option<u16>,
    /// How many tracks it covers (1).
    pub span: u16,
}

impl<'de> Deserialize<'de> for GridLine {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let what = "a line like 2, \"1 / span 2\", \"1 / 3\" or \"span 2\"";
        let bad = || super::de::expected(what, &v);
        let num = |s: &str| s.trim().parse::<u16>().ok().filter(|n| *n >= 1);
        let span = |s: &str| s.trim().strip_prefix("span").and_then(num);
        match &v {
            serde_json::Value::Number(n) => n
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .filter(|n| *n >= 1)
                .map(|n| GridLine {
                    start: Some(n),
                    span: 1,
                })
                .ok_or_else(bad),
            serde_json::Value::String(s) => match s.split_once('/') {
                None => span(s)
                    .map(|n| GridLine {
                        start: None,
                        span: n,
                    })
                    .or_else(|| {
                        num(s).map(|n| GridLine {
                            start: Some(n),
                            span: 1,
                        })
                    })
                    .ok_or_else(bad),
                Some((a, b)) => {
                    let start = num(a).ok_or_else(bad)?;
                    let n = span(b)
                        .or_else(|| {
                            num(b)
                                .and_then(|end| end.checked_sub(start))
                                .filter(|n| *n >= 1)
                        })
                        .ok_or_else(bad)?;
                    Ok(GridLine {
                        start: Some(start),
                        span: n,
                    })
                }
            },
            _ => Err(bad()),
        }
    }
}

impl Serialize for GridLine {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match (self.start, self.span) {
            (Some(n), 1) => s.serialize_u16(n),
            (Some(n), m) => s.serialize_str(&format!("{n} / span {m}")),
            (None, m) => s.serialize_str(&format!("span {m}")),
        }
    }
}

#[cfg(test)]
mod line_tests {
    use super::GridLine;
    use serde_json::json;

    #[test]
    fn grid_lines_read_as_css_writes_them() {
        let line = |v| serde_json::from_value::<GridLine>(v).unwrap();
        assert_eq!(
            line(json!(2)),
            GridLine {
                start: Some(2),
                span: 1
            }
        );
        assert_eq!(
            line(json!("1 / span 2")),
            GridLine {
                start: Some(1),
                span: 2
            }
        );
        assert_eq!(
            line(json!("2 / 4")),
            GridLine {
                start: Some(2),
                span: 2
            }
        );
        assert_eq!(
            line(json!("span 3")),
            GridLine {
                start: None,
                span: 3
            }
        );
        assert!(serde_json::from_value::<GridLine>(json!("3 / 2")).is_err());
        assert_eq!(
            serde_json::to_value(line(json!("1 / span 2"))).unwrap(),
            json!("1 / span 2")
        );
    }
}
