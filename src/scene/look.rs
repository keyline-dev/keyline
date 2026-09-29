//! What every visible layer shares beyond its box: fills, strokes and
//! shadows (Figma's paint model), blur, corner radius, and transforms that
//! apply after layout (they never move siblings).

use serde::{Deserialize, Serialize};

use super::defaults::{is_false, is_one, is_zero, one};
use super::{Paint, Shadow, Stroke};

/// One item or several: `x` or `[x, y]`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    /// A single item.
    One(T),
    /// Several, bottom to top.
    Many(Vec<T>),
}

// Not `untagged`: that would replace each item's precise error with "data
// did not match any variant".
impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for OneOrMany<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::Array(items) => items
                .into_iter()
                .map(|v| serde_json::from_value(v).map_err(D::Error::custom))
                .collect::<Result<_, _>>()
                .map(OneOrMany::Many),
            v => serde_json::from_value(v)
                .map(OneOrMany::One)
                .map_err(D::Error::custom),
        }
    }
}

impl<T> OneOrMany<T> {
    /// The items, bottom to top.
    pub fn as_slice(&self) -> &[T] {
        match self {
            OneOrMany::One(t) => std::slice::from_ref(t),
            OneOrMany::Many(v) => v,
        }
    }
}

/// Paint, effects, corners and transforms: fields every layer may carry.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    /// Paints filling the shape (or the letters), bottom to top: a color
    /// string, `{color}`, `{gradient}`, `{image}`, `{pattern}` or `{noise}`.
    /// `[]` fills nothing (outlined text). Wins over `color`/`gradient`.
    #[serde(rename = "fill", default, skip_serializing_if = "Option::is_none")]
    pub fills: Option<OneOrMany<Paint>>,
    /// Strokes around the shape (or the letters), bottom to top; a color
    /// string is a 1 px stroke. Wins over `stroke`/`outline`.
    #[serde(rename = "stroke", default, skip_serializing_if = "Option::is_none")]
    pub strokes: Option<OneOrMany<Stroke>>,
    /// CSS box-shadow style shadows `{x, y, blur, spread, color, inset}`;
    /// they follow the layer's own alpha (a cutout's outline, the letters).
    #[serde(rename = "shadow", default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<OneOrMany<Shadow>>,
    /// Gaussian blur of the whole layer, px (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub blur: f32,
    /// Blur of what's behind the layer, within its shape (frosted glass), px (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub backdrop_blur: f32,
    /// Corner radius of rects, frames and images: px, `[tl, tr, br, bl]`,
    /// or `"full"` for a capsule. Wins over `cornerRadius`.
    #[serde(
        rename = "borderRadius",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub radius: Option<Radius>,
    /// Visual scale about the box center, after layout (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f32,
    /// Visual shift after layout, px `[x, y]`; siblings don't move ([0, 0]).
    #[serde(rename = "translate", default, skip_serializing_if = "is_origin")]
    pub offset: [f32; 2],
    /// Skew in degrees `[x, y]` about the box center ([0, 0]).
    #[serde(default, skip_serializing_if = "is_origin")]
    pub skew: [f32; 2],
    /// Mirror horizontally (false).
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_x: bool,
    /// Mirror vertically (false).
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_y: bool,
    /// Torn or ragged sides: `{sides, depth, seed}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edges: Option<Edges>,
}

/// Torn edges: the chosen sides of the layer's box, ripped like paper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edges {
    /// The only style so far: `torn` (default).
    #[serde(default, skip_serializing_if = "is_torn")]
    pub style: EdgeStyle,
    /// Which sides tear: `top`, `right`, `bottom`, `left` (default all).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sides: Vec<Side>,
    /// How deep the tears cut in, px (12).
    #[serde(default = "twelve")]
    pub depth: f32,
    /// Varies the tear; the same seed tears the same way (0).
    #[serde(default)]
    pub seed: u32,
}

/// How edges are roughened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeStyle {
    /// Irregular tears, like ripped paper.
    #[default]
    Torn,
}

fn is_torn(s: &EdgeStyle) -> bool {
    *s == EdgeStyle::Torn
}

fn twelve() -> f32 {
    12.0
}

/// A side of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// The top edge.
    Top,
    /// The right edge.
    Right,
    /// The bottom edge.
    Bottom,
    /// The left edge.
    Left,
}

fn is_origin(v: &[f32; 2]) -> bool {
    *v == [0.0, 0.0]
}

impl Look {
    /// True when a transform other than rotation is set.
    pub fn transforms(&self) -> bool {
        self.scale != 1.0
            || self.offset != [0.0, 0.0]
            || self.skew != [0.0, 0.0]
            || self.flip_x
            || self.flip_y
    }
}

/// Corner radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Radius {
    /// The same on every corner, px.
    All(f32),
    /// `[top-left, top-right, bottom-right, bottom-left]`, px.
    Corners([f32; 4]),
    /// Half the shorter side: a capsule or circle at every size.
    Full,
}

impl Radius {
    /// Per-corner radii for a `w` × `h` box, scaled by `k`.
    pub fn corners(self, w: f32, h: f32, k: f32) -> [f32; 4] {
        match self {
            Radius::All(r) => [r * k; 4],
            Radius::Corners(c) => c.map(|r| r * k),
            Radius::Full => [w.min(h) / 2.0; 4],
        }
    }
}

impl Serialize for Radius {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Radius::All(r) => s.serialize_f32(*r),
            Radius::Corners(c) => c.serialize(s),
            Radius::Full => s.serialize_str("full"),
        }
    }
}

impl<'de> Deserialize<'de> for Radius {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        if v.as_str() == Some("full") {
            return Ok(Radius::Full);
        }
        super::de::float(&v)
            .map(Radius::All)
            .or_else(|| super::de::floats(&v).map(Radius::Corners))
            .ok_or_else(|| super::de::expected("radius px, [tl, tr, br, bl] or \"full\"", &v))
    }
}

#[cfg(test)]
mod tests {
    use super::{Look, Radius};
    use serde_json::json;

    #[test]
    fn radius_reads_px_corners_and_full() {
        let r = |v| serde_json::from_value::<Radius>(v).unwrap();
        assert_eq!(r(json!(8)).corners(100.0, 40.0, 0.5), [4.0; 4]);
        assert_eq!(
            r(json!([1, 2, 3, 4])).corners(10.0, 10.0, 1.0),
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(r(json!("full")).corners(100.0, 40.0, 1.0), [20.0; 4]);
        assert!(serde_json::from_value::<Radius>(json!("round")).is_err());
    }

    #[test]
    fn looks_take_one_or_many_and_omit_defaults() {
        let l: Look = serde_json::from_value(
            json!({"fill": "#FF0000", "shadow": [{"y": 4, "blur": 8, "color": "#0004"}]}),
        )
        .unwrap();
        assert_eq!(l.fills.as_ref().unwrap().as_slice().len(), 1);
        assert_eq!(l.shadows.as_ref().unwrap().as_slice().len(), 1);
        let empty: Look = serde_json::from_value(json!({})).unwrap();
        assert_eq!(serde_json::to_value(&empty).unwrap(), json!({}));
        assert!(!empty.transforms());
    }
}
