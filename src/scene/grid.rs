//! Grid layout settings for frames, like CSS grid.

use serde::{Deserialize, Serialize};

use super::{Gap, Padding};

/// A frame that places its children in rows and columns, like CSS grid.
/// Children take `area`, or `cell` and `span`, else fill the next free cell
/// row by row. They stretch to their cell unless they have a px size, then
/// they sit at its top-left. Without a `width` or `height`, the frame hugs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grid {
    /// Column tracks, CSS style: `"1fr 2fr"`, `"200px auto 25%"`,
    /// `"repeat(3, 1fr)"`; or `{"min": 160}`, as many equal columns as fit
    /// at least that wide. Default: one `1fr` per `areas` column, else one.
    #[serde(default, skip_serializing_if = "Tracks::is_empty")]
    pub columns: Tracks,
    /// Row tracks, as `columns`; rows beyond them are `auto` (default).
    #[serde(default, skip_serializing_if = "Tracks::is_empty")]
    pub rows: Tracks,
    /// Space between cells, px; `[rowGap, columnGap]` (default 0).
    #[serde(default, skip_serializing_if = "Gap::is_zero")]
    pub gap: Gap,
    /// Space inside the frame's edges, as a stack's (default 0).
    #[serde(default, skip_serializing_if = "Padding::is_zero")]
    pub padding: Padding,
    /// Named areas, one string per row, a name per column; `.` is empty:
    /// `["hero hero side", "cta cta side"]`. Each name is a rectangle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Tracks {
    /// Not given: the default applies.
    #[default]
    None,
    /// These tracks, in order.
    List(Vec<Track>),
    /// As many `1fr` tracks as fit, each at least `min` px.
    Fit {
        /// Smallest track, px.
        min: f32,
    },
}

impl Tracks {
    fn is_empty(&self) -> bool {
        matches!(self, Tracks::None)
    }

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
                .map_err(|_| format!("bad repeat count {n}"))?;
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
    Ok(out)
}

impl<'de> Deserialize<'de> for Tracks {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let what = "tracks like \"1fr 200px auto\", a count, or {\"min\": px}";
        match &v {
            serde_json::Value::String(s) => parse_tracks(s)
                .map(Tracks::List)
                .map_err(serde::de::Error::custom),
            // A count is that many equal columns: 3 → "repeat(3, 1fr)".
            serde_json::Value::Number(n) => n
                .as_u64()
                .filter(|n| (1..=100).contains(n))
                .map(|n| Tracks::List(vec![Track::Fr(1.0); n as usize]))
                .ok_or_else(|| super::de::expected(what, &v)),
            serde_json::Value::Object(o) if o.len() == 1 => o
                .get("min")
                .and_then(super::de::float)
                .filter(|m| *m > 0.0)
                .map(|min| Tracks::Fit { min })
                .ok_or_else(|| super::de::expected(what, &v)),
            _ => Err(super::de::expected(what, &v)),
        }
    }
}

impl Serialize for Tracks {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Tracks::None => s.serialize_str(""),
            Tracks::Fit { min } => serde_json::json!({ "min": min }).serialize(s),
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
    use serde_json::json;

    #[test]
    fn tracks_read_css_syntax_counts_and_min() {
        let g: Grid = serde_json::from_value(
            json!({"columns": "200px repeat(2, 1fr) auto 25% 10", "rows": 2}),
        )
        .unwrap();
        assert_eq!(
            g.columns.list(),
            [
                Track::Px(200.0),
                Track::Fr(1.0),
                Track::Fr(1.0),
                Track::Auto,
                Track::Pct(0.25),
                Track::Px(10.0)
            ]
        );
        assert_eq!(g.rows.list(), [Track::Fr(1.0), Track::Fr(1.0)]);
        let v = serde_json::to_value(&g).unwrap();
        assert_eq!(v["columns"], "200px 1fr 1fr auto 25% 10px");
        let g: Grid = serde_json::from_value(json!({"columns": {"min": 160}})).unwrap();
        assert_eq!(g.columns, Tracks::Fit { min: 160.0 });
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            json!({"columns": {"min": 160.0}})
        );
        let e = serde_json::from_value::<Grid>(json!({"columns": "1fr 2em"})).unwrap_err();
        assert!(e.to_string().contains("bad track 2em"), "{e}");
    }

    #[test]
    fn areas_are_rectangles() {
        let g: Grid =
            serde_json::from_value(json!({"areas": ["hero hero side", "cta cta side"]})).unwrap();
        g.check().unwrap();
        assert_eq!(g.area("hero"), Some((0, 0, 1, 2)));
        assert_eq!(g.area("side"), Some((0, 2, 2, 1)));
        assert_eq!(g.area("nope"), None);
        let bad: Grid = serde_json::from_value(json!({"areas": ["a a", "a b"]})).unwrap();
        assert_eq!(bad.check().unwrap_err(), "grid area a isn't a rectangle");
        let bad: Grid = serde_json::from_value(json!({"areas": ["a a", "b"]})).unwrap();
        assert!(bad.check().unwrap_err().contains("same number"));
    }
}
