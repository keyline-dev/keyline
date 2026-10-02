//! Paints: what fills a shape or the letters of a text. A color string, or
//! an object keyed by its kind: `{color}`, `{gradient}`, `{image}`,
//! `{pattern}` or `{noise}`, each with `opacity` and `blendMode`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    #[serde(rename = "filter", default, skip_serializing_if = "Adjust::is_none")]
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

/// Image color adjustments, CSS `filter` functions on their CSS scales.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adjust {
    /// × each channel: 0 is black, 1 unchanged, 1.5 lighter (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub brightness: f32,
    /// 0 is flat gray, 1 unchanged, 2 punchier (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub contrast: f32,
    /// 0 is gray, 1 unchanged, 2 more vivid (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub saturate: f32,
    /// 0…1: toward black and white.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub grayscale: f32,
    /// 0…1: toward sepia.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sepia: f32,
    /// Hue rotation, degrees (CSS `hue-rotate`).
    #[serde(rename = "hueRotate", default, skip_serializing_if = "is_zero")]
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

impl Default for Adjust {
    fn default() -> Self {
        Adjust {
            brightness: 1.0,
            contrast: 1.0,
            saturate: 1.0,
            grayscale: 0.0,
            sepia: 0.0,
            hue: 0.0,
            duotone: None,
            tint: None,
            halftone: 0.0,
        }
    }
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
            Value::String(s) => match super::gradient::from_css(s) {
                Some(g) => serde_json::from_value(serde_json::json!({ "gradient": g }))
                    .map(Paint::Gradient)
                    .map_err(|e| D::Error::custom(format!("{s}: {e}"))),
                None => Color::parse(s)
                    .map(Paint::color)
                    .ok_or_else(|| D::Error::custom(format!("bad color {s}"))),
            },
            // A gradient written flat, `{type, angle, stops}`, as CSS and
            // Figma put it: every run of the benchmark guessed this first.
            Value::Object(o) if o.contains_key("stops") && !o.contains_key("gradient") => {
                let (common, gradient): (Map<String, Value>, Map<String, Value>) = o
                    .clone()
                    .into_iter()
                    .partition(|(k, _)| k == "opacity" || k == "blendMode");
                let mut wrapped = common;
                wrapped.insert("gradient".into(), Value::Object(gradient));
                serde_json::from_value(Value::Object(wrapped))
                    .map(Paint::Gradient)
                    .map_err(|e| D::Error::custom(format!("gradient fill: {e}")))
            }
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
    fn css_gradient_strings_are_gradients() {
        // What every recreate run wrote for a fade.
        let css = |s: &str| serde_json::from_value::<Paint>(json!(s)).unwrap();
        let obj = |v| serde_json::from_value::<Paint>(v).unwrap();
        assert_eq!(
            css("linear-gradient(180deg, #fff 0%, rgba(255, 255, 255, 0) 100%)"),
            obj(json!({"type": "linear", "angle": 180,
                "stops": [{"color": "#fff", "offset": 0}, {"color": "#FFFFFF00", "offset": 1}]}))
        );
        assert_eq!(
            css("linear-gradient(to right, red, blue)"),
            obj(json!({"type": "linear", "angle": 90, "stops": ["red", "blue"]}))
        );
        assert_eq!(
            css("linear-gradient(red, blue)"),
            obj(json!({"type": "linear", "stops": ["red", "blue"]}))
        );
        assert_eq!(
            css("radial-gradient(circle at center, #000 40%, transparent)"),
            obj(json!({"type": "radial", "radius": [0.70710677, 0.70710677],
                    "stops": [{"color": "#000", "offset": 0.4}, "transparent"]}))
        );
        let Paint::Gradient(g) = css("linear-gradient(to bottom right, #000, #fff)") else {
            panic!("not a gradient")
        };
        assert_eq!(g.gradient.angle, Some(135.0));
        let e = serde_json::from_value::<Paint>(json!("linear-gradient(90deg, nope, #fff)"))
            .unwrap_err()
            .to_string();
        assert!(e.contains("linear-gradient(90deg, nope, #fff): "), "{e}");
    }

    #[test]
    fn flat_gradients_as_agents_write_them_are_gradients() {
        // The fill every benchmark run guessed first: the gradient written flat.
        let p: Paint =
            serde_json::from_value(json!({"type": "linear", "angle": 180, "opacity": 0.5,
            "stops": [{"color": "#F4F5F4", "offset": 0}, {"color": "#F4F5F400", "offset": 1}]}))
            .unwrap();
        let Paint::Gradient(g) = p else {
            panic!("not a gradient")
        };
        assert_eq!(g.opacity, 0.5);
        assert_eq!(g.gradient.stops.len(), 2);
        let e = serde_json::from_value::<Paint>(json!({"type": "linear", "stops": [], "bogus": 1}))
            .unwrap_err();
        assert!(e.to_string().starts_with("gradient fill:"), "{e}");
    }

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
