//! Stack (auto layout) settings for frames.

use serde::{Deserialize, Serialize};

/// A frame that places its children one after another, like CSS flexbox or
/// a design tool's Auto Layout. Children's `x`, `y` and `constraints` are
/// ignored unless they're `position: absolute`. Without a `width` or
/// `height`, the frame hugs its children on that axis.
#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    /// Main axis; a list is tried in order and the first that fits is used,
    /// e.g. `["row", "column"]`: a row where it fits, else a column
    /// (default row, as in CSS).
    pub dir: Dirs,
    /// Space between children, px; `[rowGap, columnGap]` for wrapped stacks
    /// (default 0).
    pub gap: Gap,
    /// Space inside the frame's edges, px: one value, `[vertical,
    /// horizontal]`, or `[top, right, bottom, left]` (default 0).
    pub padding: Padding,
    /// Where children sit across the main axis (default start).
    pub align: StackAlign,
    /// How children share leftover space along the main axis (default start).
    pub justify: Justify,
    /// Wrap children onto more lines when they don't fit (default false).
    pub wrap: bool,
}

/// One main axis, or several tried in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Dirs {
    /// Always this axis.
    One(Dir),
    /// The first axis whose layout fits; the last one otherwise.
    FirstFit(Vec<Dir>),
}

impl<'de> Deserialize<'de> for Dirs {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let what = "dir row|column|row-reverse|column-reverse, or a list of them";
        let parsed = match &v {
            serde_json::Value::Array(_) => serde_json::from_value(v.clone()).map(Dirs::FirstFit),
            _ => serde_json::from_value(v.clone()).map(Dirs::One),
        };
        parsed.map_err(|_| super::de::expected(what, &v))
    }
}

impl Dirs {
    /// The axes to try, in order.
    pub fn options(&self) -> &[Dir] {
        match self {
            Dirs::One(d) => std::slice::from_ref(d),
            Dirs::FirstFit(ds) => ds,
        }
    }
}

impl Default for Dirs {
    fn default() -> Self {
        Dirs::One(Dir::Row)
    }
}

impl From<Dir> for Dirs {
    fn from(d: Dir) -> Self {
        Dirs::One(d)
    }
}

/// A stack's main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Dir {
    /// Left to right.
    Row,
    /// Top to bottom.
    Column,
    /// Right to left.
    RowReverse,
    /// Bottom to top.
    ColumnReverse,
}

impl Dir {
    /// True for rows, reversed or not.
    pub fn is_row(self) -> bool {
        matches!(self, Dir::Row | Dir::RowReverse)
    }

    /// True for the reversed directions.
    pub fn is_reverse(self) -> bool {
        matches!(self, Dir::RowReverse | Dir::ColumnReverse)
    }

    /// The name as written in JSON.
    pub fn name(self) -> &'static str {
        match self {
            Dir::Row => "row",
            Dir::Column => "column",
            Dir::RowReverse => "row-reverse",
            Dir::ColumnReverse => "column-reverse",
        }
    }
}

/// Space between children.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Gap {
    /// The same gap between children and between wrapped lines, px.
    Both(f32),
    /// `[rowGap, columnGap]`, px, as in CSS: the space between rows, and
    /// between columns.
    Axes([f32; 2]),
}

impl Default for Gap {
    fn default() -> Self {
        Gap::Both(0.0)
    }
}

impl<'de> Deserialize<'de> for Gap {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        super::de::float(&v)
            .map(Gap::Both)
            .or_else(|| super::de::floats(&v).map(Gap::Axes))
            .ok_or_else(|| super::de::expected("gap px or [rowGap, columnGap]", &v))
    }
}

impl Gap {
    /// `(between children, between lines)` for a stack in `dir`.
    pub fn main_cross(self, dir: Dir) -> (f32, f32) {
        let (row_gap, column_gap) = match self {
            Gap::Both(g) => (g, g),
            Gap::Axes([r, c]) => (r, c),
        };
        if dir.is_row() {
            (column_gap, row_gap)
        } else {
            (row_gap, column_gap)
        }
    }

    pub(super) fn is_zero(&self) -> bool {
        *self == Gap::Both(0.0)
    }

    /// True when any gap is negative.
    pub fn is_negative(self) -> bool {
        let (a, b) = self.main_cross(Dir::Row);
        a < 0.0 || b < 0.0
    }
}

/// Space inside a frame's edges, like CSS `padding`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Padding {
    /// The same on every side, px.
    All(f32),
    /// `[vertical, horizontal]`, px.
    Axes([f32; 2]),
    /// `[top, right, bottom, left]`, px.
    Sides([f32; 4]),
}

impl Default for Padding {
    fn default() -> Self {
        Padding::All(0.0)
    }
}

impl<'de> Deserialize<'de> for Padding {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        super::de::float(&v)
            .map(Padding::All)
            .or_else(|| super::de::floats(&v).map(Padding::Axes))
            .or_else(|| super::de::floats(&v).map(Padding::Sides))
            .ok_or_else(|| {
                super::de::expected(
                    "padding px, [vertical, horizontal] or [top, right, bottom, left]",
                    &v,
                )
            })
    }
}

impl Padding {
    /// `[top, right, bottom, left]`, px.
    pub fn sides(self) -> [f32; 4] {
        match self {
            Padding::All(p) => [p; 4],
            Padding::Axes([v, h]) => [v, h, v, h],
            Padding::Sides(s) => s,
        }
    }

    pub(super) fn is_zero(&self) -> bool {
        self.sides() == [0.0; 4]
    }
}

/// Cross-axis placement of a stack's children: CSS `align-items`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StackAlign {
    /// Top of a row, left of a column.
    #[serde(rename = "flex-start")]
    Start,
    /// Centered.
    Center,
    /// Bottom of a row, right of a column.
    #[serde(rename = "flex-end")]
    End,
    /// As wide (column) or tall (row) as the stack's inside, unless the child
    /// has a fixed size on that axis; the default, as in CSS.
    #[default]
    Stretch,
    /// Rows only: children's first text baselines line up; other children
    /// sit on the baseline by their bottom edge.
    Baseline,
}

/// Main-axis distribution of a stack's children: CSS `justify-content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Justify {
    /// Packed at the start.
    #[default]
    #[serde(rename = "flex-start")]
    Start,
    /// Packed in the middle.
    #[serde(rename = "center")]
    Center,
    /// Packed at the end.
    #[serde(rename = "flex-end")]
    End,
    /// First and last at the edges, the rest spread evenly between.
    #[serde(rename = "space-between")]
    Between,
    /// Equal space on both sides of every child (half a gap at the edges).
    #[serde(rename = "space-around")]
    Around,
    /// Equal space around every child, edges included.
    #[serde(rename = "space-evenly")]
    Evenly,
}

#[cfg(test)]
mod tests {
    use super::{Dir, Dirs, Gap, Padding, Stack};
    use crate::scene::FrameLayout;
    use serde_json::json;

    fn stack(v: serde_json::Value) -> Stack {
        serde_json::from_value::<FrameLayout>(v)
            .unwrap()
            .stack
            .unwrap()
    }

    #[test]
    fn stacks_read_shorthands() {
        let s =
            stack(json!({"flexDirection": ["row", "column"], "gap": [4, 8], "padding": [10, 20]}));
        assert_eq!(s.dir, Dirs::FirstFit(vec![Dir::Row, Dir::Column]));
        assert_eq!(s.gap.main_cross(Dir::Row), (8.0, 4.0));
        assert_eq!(s.gap.main_cross(Dir::Column), (4.0, 8.0));
        assert_eq!(s.padding.sides(), [10.0, 20.0, 10.0, 20.0]);
        assert_eq!(
            Padding::Sides([1.0, 2.0, 3.0, 4.0]).sides(),
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(Gap::Both(5.0).main_cross(Dir::ColumnReverse), (5.0, 5.0));
    }

    #[test]
    fn short_form_stacks_round_trip_unchanged() {
        let v = json!({"flexDirection": "row", "justifyContent": "space-evenly", "gap": 10.0, "padding": 20.0});
        let l: FrameLayout = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&l).unwrap(), v);
    }
}
