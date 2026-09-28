//! Paints: what fills a shape or the letters of a text. A color string, or
//! an object keyed by its kind: `{color}`, `{gradient}`, `{image}`,
//! `{pattern}` or `{noise}`, each with `opacity` and `blendMode`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::defaults::{center, is_center, is_default, is_one, is_zero, one};
use super::{BlendMode, Color, Crop, Fit, Gradient};

/// One paint.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// A solid color.
    Solid(Solid),
    /// A linear, radial or conic gradient.
    Gradient(GradientFill),
    /// An image, placed like an image layer.
    Image(ImagePaint),
    /// A repeating built-in pattern.
    Pattern(PatternPaint),
    /// Film grain.
    Noise(NoisePaint),
}

/// Opacity and blend mode, shared by every paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Common {
    /// 0–1.
    pub opacity: f32,
    /// How the paint composites onto the paints below it.
    pub blend_mode: BlendMode,
}

/// `{color}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solid {
    /// The color.
    pub color: Color,
    /// 0–1 (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// How the paint composites onto the paints below it (normal).
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
}

/// `{gradient}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradientFill {
    /// The gradient.
    pub gradient: Gradient,
    /// 0–1 (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// How the paint composites onto the paints below it (normal).
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
}

/// `{image}`: an asset placed in the shape's box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImagePaint {
    /// Asset id.
    pub image: String,
    /// Cover, contain or tile the box (`fill`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub fit: Fit,
    /// The point, 0–1 per axis, kept in view ([0.5, 0.5]).
    #[serde(default = "center", skip_serializing_if = "is_center")]
    pub focus: [f32; 2],
    /// Show only this part of the image, 0–1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    /// With `fit: tile`, each tile's size relative to the image's own (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub tile_scale: f32,
    /// Color adjustments.
    #[serde(default, skip_serializing_if = "Adjust::is_none")]
    pub adjust: Adjust,
    /// 0–1 (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// How the paint composites onto the paints below it (normal).
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
}

/// `{pattern}`: a repeating pattern in one color over transparent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatternPaint {
    /// Which pattern.
    pub pattern: PatternKind,
    /// Its color (`#00000033`).
    #[serde(default = "faint", skip_serializing_if = "is_faint")]
    pub color: Color,
    /// Repeat length, px (12).
    #[serde(default = "twelve", skip_serializing_if = "is_twelve")]
    pub size: f32,
    /// Rotation in degrees (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f32,
    /// 0–1 (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// How the paint composites onto the paints below it (normal).
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
}

/// Built-in patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternKind {
    /// Dots on a square grid.
    Dots,
    /// Parallel stripes, half the size wide.
    Stripes,
    /// Grid lines.
    Grid,
    /// A checkerboard.
    Checker,
    /// A zigzag line.
    Zigzag,
    /// Rays from the box center (sunburst); `size` is the number of rays.
    Rays,
}

/// `{noise}`: grain, drawn over the paints below it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoisePaint {
    /// Strength, 0–1.
    pub noise: f32,
    /// Same seed, same grain (0).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub seed: u32,
    /// Grain size, px (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub size: f32,
    /// 0–1 (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// How the paint composites onto the paints below it (normal).
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
}

/// Image color adjustments, CSS filter names; 0 leaves the image unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adjust {
    /// −1…1: darker or lighter.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub brightness: f32,
    /// −1…1: flatter or punchier.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub contrast: f32,
    /// −1…1: grayer or more vivid.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub saturate: f32,
    /// 0…1: toward black and white.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub grayscale: f32,
    /// 0…1: toward sepia.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sepia: f32,
    /// Hue rotation, degrees.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub hue: f32,
    /// Maps shadows to the first color and highlights to the second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duotone: Option<[Color; 2]>,
    /// Recolors every visible pixel, keeping its alpha (a logo in white).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<Color>,
    /// Halftone dot spacing, px: the image redrawn as black dots, bigger
    /// where it's darker, on transparent (0: off). `tint` recolors the dots.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub halftone: f32,
}

impl Adjust {
    /// True when nothing is adjusted.
    pub fn is_none(&self) -> bool {
        *self == Adjust::default()
    }
}

fn faint() -> Color {
    Color(0x3300_0000)
}
fn is_faint(c: &Color) -> bool {
    *c == faint()
}
fn twelve() -> f32 {
    12.0
}
fn is_twelve(v: &f32) -> bool {
    *v == 12.0
}
fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl Paint {
    /// Opacity and blend mode.
    pub fn common(&self) -> Common {
        let (opacity, blend_mode) = match self {
            Paint::Solid(p) => (p.opacity, p.blend_mode),
            Paint::Gradient(p) => (p.opacity, p.blend_mode),
            Paint::Image(p) => (p.opacity, p.blend_mode),
            Paint::Pattern(p) => (p.opacity, p.blend_mode),
            Paint::Noise(p) => (p.opacity, p.blend_mode),
        };
        Common {
            opacity,
            blend_mode,
        }
    }

    /// A plain color at full opacity.
    pub fn color(color: Color) -> Paint {
        Paint::Solid(Solid {
            color,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        })
    }
}

impl Serialize for Paint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Paint::Solid(p) if p.opacity == 1.0 && p.blend_mode == BlendMode::Normal => {
                p.color.serialize(s)
            }
            Paint::Solid(p) => p.serialize(s),
            Paint::Gradient(p) => p.serialize(s),
            Paint::Image(p) => p.serialize(s),
            Paint::Pattern(p) => p.serialize(s),
            Paint::Noise(p) => p.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Paint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = Value::deserialize(d)?;
        let parse = |what: &str| -> Result<Paint, D::Error> {
            let r = match what {
                "color" => serde_json::from_value(v.clone()).map(Paint::Solid),
                "gradient" => serde_json::from_value(v.clone()).map(Paint::Gradient),
                "image" => serde_json::from_value(v.clone()).map(Paint::Image),
                "pattern" => serde_json::from_value(v.clone()).map(Paint::Pattern),
                _ => serde_json::from_value(v.clone()).map(Paint::Noise),
            };
            r.map_err(|e| D::Error::custom(format!("{what} fill: {e}")))
        };
        match &v {
            Value::String(s) => Color::parse(s)
                .map(Paint::color)
                .ok_or_else(|| D::Error::custom(format!("bad color {s}"))),
            // `color` last: patterns carry a color too.
            Value::Object(o) => match ["gradient", "image", "pattern", "noise", "color"]
                .into_iter()
                .find(|k| o.contains_key(*k))
            {
                Some(k) => parse(k),
                None => Err(D::Error::custom(
                    "a fill is a color, or an object with color, gradient, image, pattern or noise",
                )),
            },
            _ => Err(D::Error::custom("a fill is a color string or an object")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Paint, PatternKind};
    use serde_json::json;

    #[test]
    fn paints_are_known_by_their_key() {
        let p = |v| serde_json::from_value::<Paint>(v).unwrap();
        assert!(matches!(p(json!("#FF0000")), Paint::Solid(_)));
        assert!(
            matches!(p(json!({"color": "#FF0000", "opacity": 0.5})), Paint::Solid(s) if s.opacity == 0.5)
        );
        assert!(matches!(
            p(json!({"gradient": {"stops": ["#000", "#FFF"]}})),
            Paint::Gradient(_)
        ));
        assert!(matches!(
            p(json!({"image": "photo", "fit": "cover"})),
            Paint::Image(_)
        ));
        assert!(
            matches!(p(json!({"pattern": "dots"})), Paint::Pattern(x) if x.pattern == PatternKind::Dots)
        );
        assert!(
            matches!(
                p(json!({"pattern": "stripes", "color": "#000000"})),
                Paint::Pattern(_)
            ),
            "a pattern's color doesn't make it a color fill"
        );
        assert!(matches!(p(json!({"noise": 0.1})), Paint::Noise(_)));
    }

    #[test]
    fn solid_paints_write_back_as_a_color_string() {
        let v = json!("#FF0000");
        assert_eq!(
            serde_json::to_value(serde_json::from_value::<Paint>(v.clone()).unwrap()).unwrap(),
            v
        );
    }

    #[test]
    fn bad_paints_say_what_went_wrong() {
        let e = |v| serde_json::from_value::<Paint>(v).unwrap_err().to_string();
        assert!(e(json!({"colour": "#000"})).contains("a fill is a color, or an object with"));
        assert!(
            e(json!({"image": "a", "fitt": "fill"}))
                .starts_with("image fill: unknown field `fitt`")
        );
        assert!(e(json!("red-ish")).contains("bad color red-ish"));
    }
}
