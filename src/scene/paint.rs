//! How things are painted: colors, gradients, strokes, shadows, image fills
//! and blend modes.

use serde::{Deserialize, Serialize};

use super::defaults::{is_default, is_one, left_mid, one, right_mid};

/// An image painted inside text: the letters show the image laid out over
/// the text's box (like CSS `background-clip: text`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageFill {
    /// Asset id from `Scene::assets`.
    pub asset: String,
    /// Cover, contain or tile the text's box.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fit: Fit,
    /// With `fit: tile`, each tile's size relative to the image's own.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub tile_scale: f32,
}

/// A stroke around each glyph, centered on its edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outline {
    /// Thickness, px.
    pub width: f32,
    /// Outline color.
    pub color: Color,
}

/// A region of an image, 0–1 of its width and height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    /// Left edge of the region.
    pub x: f32,
    /// Top edge of the region.
    pub y: f32,
    /// Width of the region.
    pub width: f32,
    /// Height of the region.
    pub height: f32,
}

/// A linear gradient. `from` and `to` are points in the box, 0–1 on each
/// axis (like a design tool's gradient handles): `[0.5, 0]` → `[0.5, 1]` runs top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gradient {
    /// Start point in the box, 0–1 per axis (default left middle).
    #[serde(default = "left_mid")]
    pub from: [f32; 2],
    /// End point in the box, 0–1 per axis (default right middle).
    #[serde(default = "right_mid")]
    pub to: [f32; 2],
    /// Color stops, at least two.
    pub stops: Vec<Stop>,
}

/// One color stop of a gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stop {
    /// Position along the gradient, 0–1.
    pub at: f32,
    /// Color at that position.
    pub color: Color,
}

/// An outline; `color` or `gradient` paints it (black when neither).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    /// Width, px.
    pub width: f32,
    /// Solid color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Gradient; wins over `color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// Inside, centered on, or outside the edge.
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: StrokeAlign,
}

/// Where a stroke sits relative to the box edge; inside by default.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrokeAlign {
    /// Wholly inside the box.
    #[default]
    Inside,
    /// Centered on the edge.
    Center,
    /// Wholly outside the box.
    Outside,
}

/// A drop shadow behind text: offset `x`, `y` and `blur` radius in px.
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
    /// Shadow color, usually translucent.
    pub color: Color,
}

/// How a layer composites onto what's below it (the usual design-tool set).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    /// Draw over (source-over).
    #[default]
    Normal,
    /// Darken by multiplying colors.
    Multiply,
    /// Lighten by inverse multiplying.
    Screen,
    /// Multiply or screen, by the base color.
    Overlay,
    /// Keep the darker color.
    Darken,
    /// Keep the lighter color.
    Lighten,
    /// Brighten the base by the layer.
    ColorDodge,
    /// Darken the base by the layer.
    ColorBurn,
    /// Multiply or screen, by the layer color.
    HardLight,
    /// A gentler hard light.
    SoftLight,
    /// Absolute difference.
    Difference,
    /// A lower-contrast difference.
    Exclusion,
    /// Hue of the layer, saturation and luminosity of the base.
    Hue,
    /// Saturation of the layer.
    Saturation,
    /// Hue and saturation of the layer.
    Color,
    /// Luminosity of the layer.
    Luminosity,
}

/// How an image fills its box.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Cover the box, cropping the overflow (centered).
    #[default]
    Fill,
    /// Contain within the box, letterboxed.
    Fit,
    /// Repeat the image at its own size × `tileScale`, from the top-left.
    Tile,
}

/// `#RRGGBB` or `#RRGGBBAA`, stored as ARGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    /// Parses `#RRGGBB` or `#RRGGBBAA`.
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        let v = u32::from_str_radix(hex, 16).ok()?;
        match hex.len() {
            6 => Some(Color(0xFF00_0000 | v)),
            8 => Some(Color(v.rotate_right(8))),
            _ => None,
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = self.0 >> 24;
        let rgb = self.0 & 0x00FF_FFFF;
        if a == 0xFF {
            write!(f, "#{rgb:06X}")
        } else {
            write!(f, "#{rgb:06X}{a:02X}")
        }
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("bad color {s:?}, want #RRGGBB")))
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::Color;

    #[test]
    fn color_round_trips() {
        for s in ["#1B2A5C", "#00000040"] {
            assert_eq!(Color::parse(s).unwrap().to_string(), s);
        }
        assert_eq!(Color::parse("#FFFFFF80").unwrap().0, 0x80FF_FFFF);
        assert!(Color::parse("red").is_none());
        assert!(Color::parse("#FFF").is_none());
    }
}
