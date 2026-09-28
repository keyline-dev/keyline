//! How things are painted: colors, gradients, strokes, shadows, image fills
//! and blend modes.

use serde::{Deserialize, Serialize};

use super::defaults::{is_default, is_one, one};

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
    /// Cover the box, cropping the overflow (centered); also `cover`.
    #[default]
    #[serde(alias = "cover")]
    Fill,
    /// Contain within the box, letterboxed; also `contain`.
    #[serde(alias = "contain")]
    Fit,
    /// Repeat the image at its own size × `tileScale`, from the top-left.
    Tile,
}

/// `#RRGGBB`, `#RRGGBBAA`, `#RGB`, `#RGBA` or a common CSS color name,
/// stored as ARGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    /// Parses `#RRGGBB`, `#RRGGBBAA`, `#RGB`, `#RGBA` or a CSS color name.
    pub fn parse(s: &str) -> Option<Self> {
        let Some(hex) = s.strip_prefix('#') else {
            return named(s);
        };
        // #RGB and #RGBA double each digit, as in CSS.
        let long: String = match hex.len() {
            3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
            _ => hex.to_owned(),
        };
        let v = u32::from_str_radix(&long, 16).ok()?;
        match long.len() {
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

/// Common CSS color names, for agents that write `white` instead of hex.
fn named(s: &str) -> Option<Color> {
    let rgb = match s.to_ascii_lowercase().as_str() {
        "transparent" => return Some(Color(0)),
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "red" => 0xFF0000,
        "green" => 0x008000,
        "blue" => 0x0000FF,
        "yellow" => 0xFFFF00,
        "orange" => 0xFFA500,
        "purple" => 0x800080,
        "pink" => 0xFFC0CB,
        "gray" | "grey" => 0x808080,
        "navy" => 0x000080,
        "teal" => 0x008080,
        "gold" => 0xFFD700,
        "silver" => 0xC0C0C0,
        "maroon" => 0x800000,
        _ => return None,
    };
    Some(Color(0xFF00_0000 | rgb))
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
        // Short hex and CSS names, as agents often write them.
        assert_eq!(Color::parse("#FFF").unwrap().0, 0xFFFF_FFFF);
        assert_eq!(Color::parse("#0008").unwrap().0, 0x8800_0000);
        assert_eq!(Color::parse("White").unwrap().0, 0xFFFF_FFFF);
        assert_eq!(Color::parse("transparent").unwrap().0, 0);
        assert!(Color::parse("#12345").is_none());
        assert!(Color::parse("reddish").is_none());
    }
}
