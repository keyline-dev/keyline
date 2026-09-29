//! Lengths and positions: px, a share of the parent (`"40%"`), or, for
//! sizes, `"hug"` (fit the content) and `"fill"` (take the free space).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A size or position along one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Pixels at the master size (scaled by the size's `scale`).
    Px(f32),
    /// As big as the content (Figma "hug", CSS `fit-content`).
    Hug,
    /// The free space in a stack, or the rest of the parent in free layout
    /// (Figma "fill", SwiftUI `maxWidth: .infinity`).
    Fill,
    /// A share of the parent's inner size, 0–1 (written `"40%"`).
    Pct(f32),
}

impl Default for Length {
    fn default() -> Self {
        Length::Px(0.0)
    }
}

impl Length {
    /// Pixels, when the length is a fixed px value.
    pub fn px(self) -> Option<f32> {
        match self {
            Length::Px(v) => Some(v),
            Length::Hug | Length::Fill | Length::Pct(_) => None,
        }
    }

    /// True for `0` px, the default position.
    pub(super) fn is_zero(&self) -> bool {
        *self == Length::Px(0.0)
    }
}

impl From<f32> for Length {
    fn from(v: f32) -> Self {
        Length::Px(v)
    }
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Length::Px(v) => s.serialize_f32(*v),
            Length::Hug => s.serialize_str("hug"),
            Length::Fill => s.serialize_str("fill"),
            Length::Pct(p) => s.serialize_str(&format!("{}%", (p * 100_000.0).round() / 1000.0)),
        }
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::Number(_) => Ok(Length::Px(super::de::float(&v).unwrap_or(0.0))),
            serde_json::Value::String(t) => t.parse().map_err(serde::de::Error::custom),
            _ => Err(super::de::expected("px, \"hug\", \"fill\" or \"40%\"", &v)),
        }
    }
}

impl std::str::FromStr for Length {
    type Err = String;

    fn from_str(t: &str) -> Result<Self, String> {
        match t {
            "hug" => Ok(Length::Hug),
            "fill" => Ok(Length::Fill),
            _ => t
                .strip_suffix('%')
                .and_then(|n| n.trim().parse::<f32>().ok())
                .filter(|p| p.is_finite())
                .map(|p| Length::Pct(p / 100.0))
                .ok_or_else(|| format!("bad length {t:?}: use px, \"hug\", \"fill\" or \"40%\"")),
        }
    }
}

/// A placed layer's distance from its parent's edges: one value for both
/// axes, or `[x, y]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Inset {
    /// The same inset on both axes, px.
    Both(f32),
    /// Horizontal and vertical insets, px.
    Axes([f32; 2]),
}

impl<'de> Deserialize<'de> for Inset {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        super::de::float(&v)
            .map(Inset::Both)
            .or_else(|| super::de::floats(&v).map(Inset::Axes))
            .ok_or_else(|| super::de::expected("margin px or [x, y]", &v))
    }
}

impl Inset {
    /// `(x, y)` insets, px.
    pub fn xy(self) -> (f32, f32) {
        match self {
            Inset::Both(v) => (v, v),
            Inset::Axes([x, y]) => (x, y),
        }
    }
}

/// One of nine spots in the parent a layer can be pinned to with `place`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    /// Top-left corner.
    TopLeft,
    /// Middle of the top edge.
    Top,
    /// Top-right corner.
    TopRight,
    /// Middle of the left edge.
    Left,
    /// Center.
    Center,
    /// Middle of the right edge.
    Right,
    /// Bottom-left corner.
    BottomLeft,
    /// Middle of the bottom edge.
    Bottom,
    /// Bottom-right corner.
    BottomRight,
}

/// Where a spot sits along one axis: start, middle or end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// Left or top.
    Start,
    /// Center.
    Middle,
    /// Right or bottom.
    End,
}

impl Place {
    /// The spot on each axis, `(horizontal, vertical)`.
    pub fn spots(self) -> (Spot, Spot) {
        use Spot::{End, Middle, Start};
        match self {
            Place::TopLeft => (Start, Start),
            Place::Top => (Middle, Start),
            Place::TopRight => (End, Start),
            Place::Left => (Start, Middle),
            Place::Center => (Middle, Middle),
            Place::Right => (End, Middle),
            Place::BottomLeft => (Start, End),
            Place::Bottom => (Middle, End),
            Place::BottomRight => (End, End),
        }
    }
}

/// How a stack child takes part in the stack's flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    /// Placed in order by the stack.
    #[default]
    Auto,
    /// Taken out of the flow and placed like a free child of the frame
    /// (`x`, `y`, `constraints`, `place`), e.g. a badge over a card's corner.
    Absolute,
}

#[cfg(test)]
mod tests {
    use super::Length;
    use serde_json::json;

    #[test]
    fn lengths_read_px_hug_fill_and_percent_and_write_them_back() {
        for (v, want) in [
            (json!(120.0), Length::Px(120.0)),
            (json!("hug"), Length::Hug),
            (json!("fill"), Length::Fill),
            (json!("40%"), Length::Pct(0.4)),
            (json!("33.3%"), Length::Pct(0.333)),
        ] {
            let l: Length = serde_json::from_value(v.clone()).unwrap();
            assert_eq!(l, want);
            assert_eq!(serde_json::to_value(l).unwrap(), v);
        }
    }

    #[test]
    fn bad_lengths_say_what_is_allowed() {
        let e = serde_json::from_value::<Length>(json!("wide")).unwrap_err();
        assert!(
            e.to_string().contains("\"hug\", \"fill\" or \"40%\""),
            "{e}"
        );
    }
}
