//! Text settings: resize modes, alignment, case and colored ranges.

use serde::{Deserialize, Serialize};

use super::{Color, Kind, Layer, Length};

/// Case transform for drawn text.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextCase {
    /// As written.
    #[default]
    None,
    /// All upper case.
    Uppercase,
    /// All lower case.
    Lowercase,
    /// The first letter of every word in upper case.
    Capitalize,
}

/// How a text box and its font size relate, after iOS `UILabel`: `fit` is
/// `adjustsFontSizeToFitWidth` with `minimumScaleFactor`, then tail
/// truncation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resize {
    /// One line; the box grows to the text.
    AutoWidth,
    /// Fixed width, wraps; the box grows down.
    AutoHeight,
    /// Fixed box; the font shrinks to fit, then the text is ellipsized.
    Fit,
    /// Fixed box and font; text may overflow.
    Fixed,
    /// Fixed box and font; ellipsis after the lines that fit.
    Truncate,
}

impl Layer {
    /// A text layer's resize mode; when omitted it follows from the box:
    /// width and height → `fit`, width only → `auto-height`, else
    /// `auto-width`. A `"hug"` side counts as unset.
    pub fn text_resize(&self) -> Option<Resize> {
        let Kind::Text { resize, .. } = &self.kind else {
            return None;
        };
        let set = |l: Option<Length>| l.is_some_and(|l| l != Length::Hug);
        Some(resize.unwrap_or(match (set(self.width), set(self.height)) {
            (true, true) => Resize::Fit,
            (true, false) => Resize::AutoHeight,
            _ => Resize::AutoWidth,
        }))
    }
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    /// Flush left.
    #[default]
    Left,
    /// Centered.
    Center,
    /// Flush right.
    Right,
    /// Both edges flush (the last line flush left).
    Justify,
}

/// A styled span of text. `start` and `end` count Unicode characters
/// (code points) of the text as displayed (markup tags removed), end
/// exclusive. Every field but the bounds is optional.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Range {
    /// First character, counted in Unicode code points.
    pub start: usize,
    /// One past the last character.
    pub end: usize,
    /// Color of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Weight, 100–900.
    #[serde(
        rename = "fontWeight",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub weight: Option<u16>,
    /// `fontStyle`: `"italic"` or `"normal"`.
    #[serde(
        rename = "fontStyle",
        default,
        skip_serializing_if = "Option::is_none",
        with = "super::text_more::font_style::opt"
    )]
    pub italic: Option<bool>,
    /// Font size, px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    /// Font family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// `underline` or `line-through`.
    #[serde(
        rename = "textDecoration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub decoration: Option<super::Decoration>,
    /// A box behind the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<super::Highlight>,
    /// Raised (superscript) or lowered (subscript), smaller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<Shift>,
}

/// A span raised or lowered from the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift {
    /// Superscript (™, prices' cents).
    Sup,
    /// Subscript.
    Sub,
}

#[cfg(test)]
mod tests {
    use crate::scene::{Layer, Resize};
    use serde_json::json;

    #[test]
    fn text_resize_follows_the_box_like_uilabel() {
        let mode = |v: serde_json::Value| serde_json::from_value::<Layer>(v).unwrap().text_resize();
        assert_eq!(
            mode(json!({"type": "text", "text": "a"})),
            Some(Resize::AutoWidth)
        );
        assert_eq!(
            mode(json!({"type": "text", "text": "a", "width": 9})),
            Some(Resize::AutoHeight)
        );
        assert_eq!(
            mode(json!({"type": "text", "text": "a", "width": "fill"})),
            Some(Resize::AutoHeight)
        );
        assert_eq!(
            mode(json!({"type": "text", "text": "a", "width": "hug", "height": 9})),
            Some(Resize::AutoWidth)
        );
        assert_eq!(
            mode(json!({"type": "text", "text": "a", "width": 9, "height": 9})),
            Some(Resize::Fit)
        );
        assert_eq!(mode(json!({"type": "rect"})), None);
    }
}
