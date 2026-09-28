//! v2 text settings: italic, decoration, wrapping, vertical alignment,
//! cap-height trim, highlights, direction, OpenType features, padding, and
//! the P2 extras (curve, leader, knockout). Flattened into the text type.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::defaults::{is_default, is_false};
use super::{Color, Padding};

/// Text settings beyond the MVP's.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMore {
    /// Italic (or oblique) face (false).
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    /// Underline or strike through (none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoration: Option<Decoration>,
    /// Line breaking: `balance` evens out line lengths, `pretty` avoids a
    /// lone last word (`wrap`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub text_wrap: TextWrap,
    /// Where text sits vertically in a box taller than it: `top`, `center`
    /// or `bottom` (center for fit/fixed/truncate boxes, as the MVP; top otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<VAlign>,
    /// `"cap"` trims the space above cap height and below the baseline, so
    /// text centers optically in pills and buttons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<Trim>,
    /// A box behind every line, e.g. `{"color": "#FFE600", "padding": 6}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Highlight>,
    /// Base direction: `auto` follows the first strong letter (`auto`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub direction: Direction,
    /// OpenType features, e.g. `{"tnum": 1}` for tabular figures.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub features: BTreeMap<String, u32>,
    /// Space between the box's edges and the text, px: one value, `[v, h]`
    /// or `[t, r, b, l]` (0).
    #[serde(default, skip_serializing_if = "is_zero_padding")]
    pub padding: Padding,
    /// Sets one line of text along a circular arc of this radius, px;
    /// positive bends it up (∩), negative down (∪).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<f32>,
    /// Fills the gap at each tab with this character, the part after the
    /// tab flush right: `"Espresso\t$3"` with `"."` (menu dot leaders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
    /// The letters cut through their parent frame, showing what's behind it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub knockout: bool,
}

fn is_zero_padding(p: &Padding) -> bool {
    p.sides() == [0.0; 4]
}

/// A line under or through text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decoration {
    /// Underline.
    Underline,
    /// Strike through (old prices).
    Strike,
}

/// How lines break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextWrap {
    /// Greedy: as much on each line as fits.
    #[default]
    Wrap,
    /// Even line lengths, same number of lines (CSS `text-wrap: balance`).
    Balance,
    /// No lone word on the last line (CSS `text-wrap: pretty`).
    Pretty,
}

/// Vertical placement of text in its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VAlign {
    /// At the top.
    Top,
    /// Centered; `middle` (CSS `vertical-align`) is read as this.
    #[serde(alias = "middle")]
    Center,
    /// At the bottom.
    Bottom,
}

/// What `trim` removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trim {
    /// Above cap height and below the baseline.
    Cap,
}

/// Base text direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// From the first strong letter (Hebrew or Arabic → rtl).
    #[default]
    Auto,
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
}

/// A box behind lines or a phrase of text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    /// Box color.
    pub color: Color,
    /// Space around the text, px (4).
    #[serde(skip_serializing_if = "is_four")]
    pub padding: f32,
    /// Corner radius, px (0).
    #[serde(skip_serializing_if = "is_zero")]
    pub radius: f32,
    /// `box` (default) or `brush` (a marker-pen stroke with rough edges).
    #[serde(skip_serializing_if = "is_default")]
    pub style: HighlightStyle,
}

fn is_four(v: &f32) -> bool {
    *v == 4.0
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// How a highlight box looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HighlightStyle {
    /// A plain box.
    #[default]
    Box,
    /// A marker-pen stroke.
    Brush,
}

impl<'de> Deserialize<'de> for Highlight {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Full {
            color: Color,
            #[serde(default = "four")]
            padding: f32,
            #[serde(default)]
            radius: f32,
            #[serde(default)]
            style: HighlightStyle,
        }
        fn four() -> f32 {
            4.0
        }
        // A bare color is the common case: `"highlight": "#FFE600"`.
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::String(s) => Color::parse(&s)
                .map(|color| Highlight {
                    color,
                    padding: 4.0,
                    radius: 0.0,
                    style: HighlightStyle::Box,
                })
                .ok_or_else(|| D::Error::custom(format!("bad color {s}"))),
            v => serde_json::from_value::<Full>(v)
                .map(|f| Highlight {
                    color: f.color,
                    padding: f.padding,
                    radius: f.radius,
                    style: f.style,
                })
                .map_err(|e| D::Error::custom(format!("highlight: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Highlight, HighlightStyle, TextMore, VAlign};
    use serde_json::json;

    #[test]
    fn vertical_align_reads_css_middle_as_center() {
        let v: VAlign = serde_json::from_value(json!("middle")).unwrap();
        assert_eq!(v, VAlign::Center);
        assert_eq!(serde_json::to_value(v).unwrap(), "center");
    }

    #[test]
    fn text_settings_read_their_short_forms() {
        let m: TextMore = serde_json::from_value(
            json!({"highlight": "#FFE600", "textWrap": "balance", "padding": [4, 8]}),
        )
        .unwrap();
        assert_eq!(m.highlight.as_ref().map(|h| h.padding), Some(4.0));
        assert_eq!(m.padding.sides(), [4.0, 8.0, 4.0, 8.0]);
        let h: Highlight =
            serde_json::from_value(json!({"color": "#000", "style": "brush"})).unwrap();
        assert_eq!(h.style, HighlightStyle::Brush);
        assert_eq!(
            serde_json::to_value(TextMore::default()).unwrap(),
            json!({})
        );
    }
}
