//! Text settings: resize modes, alignment, case and colored ranges.

use serde::{Deserialize, Serialize};

use super::{Color, Kind, Layer};

/// Case transform for drawn text.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextCase {
    /// As written.
    #[default]
    None,
    /// All upper case.
    Upper,
    /// All lower case.
    Lower,
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
    /// width and height → `fit`, width only → `auto-height`, else `auto-width`.
    pub fn text_resize(&self) -> Option<Resize> {
        let Kind::Text { resize, .. } = &self.kind else {
            return None;
        };
        Some(resize.unwrap_or(match (self.width, self.height) {
            (Some(_), Some(_)) => Resize::Fit,
            (Some(_), None) => Resize::AutoHeight,
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
}

/// A colored span of text. `start` and `end` count Unicode characters
/// (code points), end exclusive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    /// First character, counted in Unicode code points.
    pub start: usize,
    /// One past the last character.
    pub end: usize,
    /// Color of the span.
    pub color: Color,
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
            mode(json!({"type": "text", "text": "a", "width": 9, "height": 9})),
            Some(Resize::Fit)
        );
        assert_eq!(
            mode(json!({"type": "text", "text": "a", "width": 9, "height": 9, "resize": "fixed"})),
            Some(Resize::Fixed)
        );
        assert_eq!(mode(json!({"type": "rect"})), None);
    }
}
