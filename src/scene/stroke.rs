//! Strokes and shadows.

use serde::{Deserialize, Serialize};

use super::defaults::{is_default, is_false, is_zero};
use super::{Color, Gradient};

/// An outline; `color` or `gradient` paints it (black when neither). A
/// color string is a 1 px stroke: `"stroke": "#000"`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    /// Width, px; `[top, right, bottom, left]` on rects for per-side borders.
    pub width: StrokeWidth,
    /// Solid color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Gradient; wins over `color`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// Inside, centered on, or outside the edge (inside; open lines center).
    #[serde(skip_serializing_if = "is_default")]
    pub align: StrokeAlign,
    /// Dash and gap lengths, px, repeating (solid when empty).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dash: Vec<f32>,
    /// Line ends: `butt` (default), `round` (dotted with a 0 dash), `square`.
    #[serde(skip_serializing_if = "is_default")]
    pub cap: Cap,
    /// Corners: `miter` (default), `round`, `bevel`.
    #[serde(skip_serializing_if = "is_default")]
    pub join: Join,
    /// Marker at the start of a line or path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<Marker>,
    /// Marker at the end of a line or path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<Marker>,
}

/// One stroke width, or one per side of a rect.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StrokeWidth {
    /// The same everywhere, px.
    All(f32),
    /// `[top, right, bottom, left]`, px.
    Sides([f32; 4]),
}

impl<'de> Deserialize<'de> for StrokeWidth {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        super::de::float(&v)
            .map(StrokeWidth::All)
            .or_else(|| super::de::floats(&v).map(StrokeWidth::Sides))
            .ok_or_else(|| super::de::expected("stroke width px or [top, right, bottom, left]", &v))
    }
}

impl StrokeWidth {
    /// The widest side, px.
    pub fn max(self) -> f32 {
        match self {
            StrokeWidth::All(w) => w,
            StrokeWidth::Sides(s) => s.into_iter().fold(0.0, f32::max),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrokeFields {
    width: StrokeWidth,
    #[serde(default)]
    color: Option<Color>,
    #[serde(default)]
    gradient: Option<Gradient>,
    #[serde(default)]
    align: StrokeAlign,
    #[serde(default)]
    dash: Vec<f32>,
    #[serde(default)]
    cap: Cap,
    #[serde(default)]
    join: Join,
    #[serde(default)]
    start: Option<Marker>,
    #[serde(default)]
    end: Option<Marker>,
}

impl<'de> Deserialize<'de> for Stroke {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = serde_json::Value::deserialize(d)?;
        if let serde_json::Value::String(s) = &v {
            let color =
                Color::parse(s).ok_or_else(|| D::Error::custom(format!("bad color {s}")))?;
            return Ok(Stroke::solid(1.0, color));
        }
        let f: StrokeFields =
            serde_json::from_value(v).map_err(|e| D::Error::custom(format!("stroke: {e}")))?;
        Ok(Stroke {
            width: f.width,
            color: f.color,
            gradient: f.gradient,
            align: f.align,
            dash: f.dash,
            cap: f.cap,
            join: f.join,
            start: f.start,
            end: f.end,
        })
    }
}

impl Stroke {
    /// A plain stroke of one color.
    pub fn solid(width: f32, color: Color) -> Stroke {
        Stroke {
            width: StrokeWidth::All(width),
            color: Some(color),
            gradient: None,
            align: StrokeAlign::default(),
            dash: Vec::new(),
            cap: Cap::default(),
            join: Join::default(),
            start: None,
            end: None,
        }
    }
}

/// Where a stroke sits relative to the edge; inside by default.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrokeAlign {
    /// Wholly inside the shape.
    #[default]
    Inside,
    /// Centered on the edge.
    Center,
    /// Wholly outside the shape.
    Outside,
}

/// How a stroke's ends are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cap {
    /// Flat at the end point.
    #[default]
    Butt,
    /// Rounded past the end point.
    Round,
    /// Square past the end point.
    Square,
}

/// How a stroke's corners are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    /// Sharp.
    #[default]
    Miter,
    /// Rounded.
    Round,
    /// Cut off.
    Bevel,
}

/// A shape at a line's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Marker {
    /// An open arrowhead.
    Arrow,
    /// A filled triangle.
    Triangle,
    /// A filled circle.
    Circle,
    /// A filled diamond.
    Diamond,
}

/// A shadow, CSS `box-shadow` style: offset `x`, `y`, `blur` and `spread`
/// in px; `inset` draws it inside the shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    /// Horizontal offset, px.
    #[serde(default)]
    pub x: f32,
    /// Vertical offset, px.
    #[serde(default)]
    pub y: f32,
    /// Blur radius, px.
    #[serde(default)]
    pub blur: f32,
    /// Grows (or, negative, shrinks) the shadow's shape, px (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spread: f32,
    /// Shadow color, usually translucent.
    pub color: Color,
    /// Inside the shape (an inner shadow) rather than behind it (false).
    #[serde(default, skip_serializing_if = "is_false")]
    pub inset: bool,
}

#[cfg(test)]
mod tests {
    use super::{Cap, Stroke, StrokeWidth};
    use serde_json::json;

    #[test]
    fn a_color_string_is_a_one_px_stroke() {
        let s: Stroke = serde_json::from_value(json!("#FF0000")).unwrap();
        assert_eq!(s.width, StrokeWidth::All(1.0));
        assert!(s.color.is_some());
    }

    #[test]
    fn strokes_read_dashes_caps_and_side_widths() {
        let s: Stroke = serde_json::from_value(
            json!({"width": [0, 0, 2, 0], "color": "#000", "dash": [0, 6], "cap": "round"}),
        )
        .unwrap();
        assert_eq!(s.width.max(), 2.0);
        assert_eq!(
            (s.dash.as_slice(), s.cap),
            ([0.0, 6.0].as_slice(), Cap::Round)
        );
        let e =
            serde_json::from_value::<Stroke>(json!({"width": 1, "colour": "#000"})).unwrap_err();
        assert!(
            e.to_string().starts_with("stroke: unknown field `colour`"),
            "{e}"
        );
    }

    #[test]
    fn mvp_strokes_round_trip_unchanged() {
        let v = json!({"width": 2.0, "color": "#D0202E", "align": "outside"});
        let s: Stroke = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&s).unwrap(), v);
    }
}
