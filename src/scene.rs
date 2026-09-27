//! MVP scene document: the JSON the agent edits and the renderer draws.
//!
//! Every field has a default and defaults are omitted when serialized, so the
//! stored JSON stays as small as what the agent wrote.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Version of the scene format; bumped on breaking changes.
pub const SCHEMA_VERSION: u32 = 0;

/// A design: a master layout, its assets and layers, and the sizes to render.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scene {
    /// Format version the scene was written with.
    #[serde(default)]
    pub schema_version: u32,
    /// Master width, px.
    pub width: f32,
    /// Master height, px.
    pub height: f32,
    /// Canvas color behind every layer; white when omitted.
    #[serde(default = "white")]
    pub background: Color,
    /// Target sizes to render; at least one.
    pub sizes: Vec<Size>,
    /// Images the layers use, by asset id.
    #[serde(default)]
    pub assets: BTreeMap<String, Asset>,
    /// Named text styles: text fields (`fontFamily`, `fontSize`, `weight`,
    /// `color`, …) that a text layer's `style` pulls in under its own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    /// Top-level layers, drawn bottom to top.
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Bumped on every mutation; doubles as the concurrency token.
    #[serde(default)]
    pub version: u64,
}

/// One output size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Size {
    /// Name used in replies, file names and `render`'s size list.
    pub id: String,
    /// Output width, px.
    pub width: f32,
    /// Output height, px.
    pub height: f32,
    /// Scale-tool factor applied to the master before constraints (default 1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f32,
}

/// An image in the asset store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// Hex SHA-256 of the bytes; the file name in the asset store.
    pub sha256: String,
    /// Intrinsic width, px (an SVG's viewBox width).
    pub width: f32,
    /// Intrinsic height, px.
    pub height: f32,
    /// Whether the asset is an SVG, rasterized at draw size.
    #[serde(default, skip_serializing_if = "is_false")]
    pub svg: bool,
}

/// A layer: the fields every type shares, plus its type's own in `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    /// Unique id; generated (`text1`, `rect2`, …) when omitted.
    #[serde(default)]
    pub id: String,
    /// Semantic name; edits can target every layer with a role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Left edge, px, relative to the parent frame.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub x: f32,
    /// Top edge, px, relative to the parent frame.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub y: f32,
    /// Width, px; images and text size themselves when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    /// Height, px; images and text size themselves when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    /// How the layer follows its parent when the parent resizes.
    #[serde(default, skip_serializing_if = "Constraints::is_default")]
    pub constraints: Constraints,
    /// 0–1; the layer and its children as one.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// Degrees, clockwise, about the box's center. Children rotate with it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f32,
    /// How the layer composites onto what's below.
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
    /// Fades the layer: the gradient's alpha at each point is the layer's
    /// opacity there (stop colors are ignored), e.g. an image fading out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Gradient>,
    /// Per-size changes, by size id: fields merged over this layer's own
    /// for that size only, e.g. `{"sky": {"fontSize": 20}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub at: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    /// The type-specific part, tagged by `type`.
    #[serde(flatten)]
    pub kind: Kind,
}

// ponytail: serde can't combine `deny_unknown_fields` with `flatten`, so
// unknown keys are caught by `check_keys` instead.
/// What a layer is, with its type's fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Kind {
    /// A raster or SVG image.
    #[serde(rename_all = "camelCase")]
    Image {
        /// Asset id from `Scene::assets`.
        asset: String,
        /// Cover, contain or tile the box; ignored with a `crop`.
        #[serde(default, skip_serializing_if = "is_default")]
        fit: Fit,
        /// Show only this part of the image, stretched to the box.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<Crop>,
        /// With `fit: tile`, each tile's size relative to the image's own.
        #[serde(default = "one", skip_serializing_if = "is_one")]
        tile_scale: f32,
        /// The point, 0–1 per axis, kept in view when `fill` crops or `fit`
        /// letterboxes, like CSS `object-position` (default center).
        #[serde(default = "center", skip_serializing_if = "is_center")]
        focus: [f32; 2],
    },
    /// Text, sized by its box like iOS `UILabel`.
    #[serde(rename_all = "camelCase")]
    Text {
        /// The text; `\n` breaks lines.
        text: String,
        /// Colored character spans.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        ranges: Vec<Range>,
        /// Inferred from the box when omitted; see [`Layer::text_resize`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resize: Option<Resize>,
        /// Line cap for fit and truncate; unlimited when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_lines: Option<usize>,
        /// `fit` shrinks the font down to `fontSize × minFontScale`.
        #[serde(default = "half", skip_serializing_if = "is_half")]
        min_font_scale: f32,
        /// End cut-off text with "…" (true) or just cut it (false).
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        ellipsis: bool,
        /// Font size, px; the maximum when fitting.
        #[serde(default = "sixteen", skip_serializing_if = "is_sixteen")]
        font_size: f32,
        /// Font weight, 100–900.
        #[serde(default = "w400", skip_serializing_if = "is_w400")]
        weight: u16,
        /// Horizontal alignment.
        #[serde(default, skip_serializing_if = "is_default")]
        align: Align,
        /// Base text color; ranges override it.
        #[serde(default = "black", skip_serializing_if = "is_black")]
        color: Color,
        /// A bundled or locally installed family; see `text::families`.
        #[serde(default = "inter", skip_serializing_if = "is_inter")]
        font_family: String,
        /// Extra space between characters, px.
        #[serde(default, skip_serializing_if = "is_zero")]
        letter_spacing: f32,
        /// Line height as a multiple of the font size; the font's own when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line_height: Option<f32>,
        /// Case transform applied when drawing.
        #[serde(default, skip_serializing_if = "is_default")]
        text_case: TextCase,
        /// Drop shadow behind the glyphs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<Shadow>,
        /// Fill the letters with an image instead of `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<ImageFill>,
        /// Fill the letters with a gradient instead of `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        /// An outline around the letters, drawn over the fill.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outline: Option<Outline>,
        /// A named style from `Scene::styles`, under this layer's own fields.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// A filled, optionally rounded rectangle.
    #[serde(rename_all = "camelCase")]
    Rect {
        /// Solid fill; none when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        /// Gradient fill; wins over `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        /// Outline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
        /// Corner radius, px.
        #[serde(default, skip_serializing_if = "is_zero")]
        corner_radius: f32,
    },
    /// A filled ellipse inscribed in the box.
    #[serde(rename_all = "camelCase")]
    Ellipse {
        /// Solid fill; none when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        /// Gradient fill; wins over `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        /// Outline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
    },
    /// A straight line from the box's top-left corner to its bottom-right:
    /// `height` 0 is horizontal, `width` 0 vertical.
    #[serde(rename_all = "camelCase")]
    Line {
        /// Line color (default black).
        #[serde(default = "black", skip_serializing_if = "is_black")]
        color: Color,
        /// Thickness, px (default 1).
        #[serde(default = "one", skip_serializing_if = "is_one")]
        stroke_width: f32,
    },
    /// A built-in icon by name, drawn in one color and contained in the box.
    #[serde(rename_all = "camelCase")]
    Icon {
        /// Icon name in its set, e.g. `mail` (Lucide) or `envelope` (Font Awesome).
        name: String,
        /// Icon set (default lucide).
        #[serde(default, skip_serializing_if = "is_default")]
        set: IconSet,
        /// Icon color (default black).
        #[serde(default = "black", skip_serializing_if = "is_black")]
        color: Color,
        /// Line width in the icon's 24-unit grid (Lucide only; default 2).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_width: Option<f32>,
    },
    /// A container: children are placed and constrained relative to it.
    #[serde(rename_all = "camelCase")]
    Frame {
        /// Child layers, drawn bottom to top.
        #[serde(default)]
        children: Vec<Layer>,
        /// Clip children to the frame's box (default true).
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        clip: bool,
        /// Background fill; none when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        /// Background gradient; wins over `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        /// Outline, drawn above the children.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
        /// Corner radius, px; also rounds the clip.
        #[serde(default, skip_serializing_if = "is_zero")]
        corner_radius: f32,
        /// Lay children out in a row or column instead of by their x and y.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stack: Option<Stack>,
    },
}

/// A built-in icon set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconSet {
    /// Lucide: outline icons (ISC).
    #[default]
    Lucide,
    /// Font Awesome Free solid icons (CC BY 4.0).
    Solid,
    /// Font Awesome Free regular (outline) icons.
    Regular,
    /// Font Awesome Free brand logos.
    Brands,
}

impl std::fmt::Display for IconSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            IconSet::Lucide => "lucide",
            IconSet::Solid => "solid",
            IconSet::Regular => "regular",
            IconSet::Brands => "brands",
        })
    }
}

/// A frame that places its children one after another, like CSS flexbox or
/// a design tool's Auto Layout. Children's `x`, `y` and `constraints` are
/// ignored. Without a `width` or `height`, the frame hugs its children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stack {
    /// Main axis.
    pub dir: Dir,
    /// Space between children, px (default 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gap: f32,
    /// Space inside the frame's edges, px (default 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub padding: f32,
    /// Where children sit across the main axis (default start).
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: StackAlign,
    /// How children share leftover space along the main axis (default start).
    #[serde(default, skip_serializing_if = "is_default")]
    pub justify: Justify,
}

/// A stack's main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dir {
    /// Left to right.
    Row,
    /// Top to bottom.
    Column,
}

/// Cross-axis placement of a stack's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StackAlign {
    /// Top of a row, left of a column.
    #[default]
    Start,
    /// Centered.
    Center,
    /// Bottom of a row, right of a column.
    End,
}

/// Main-axis distribution of a stack's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Justify {
    /// Packed at the start.
    #[default]
    Start,
    /// Packed in the middle.
    Center,
    /// Packed at the end.
    End,
    /// First and last at the edges, the rest spread evenly between.
    Between,
    /// Equal space around every child, edges included.
    Evenly,
}

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

impl Kind {
    /// The type's name, as in `type`.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Image { .. } => "image",
            Kind::Text { .. } => "text",
            Kind::Rect { .. } => "rect",
            Kind::Ellipse { .. } => "ellipse",
            Kind::Line { .. } => "line",
            Kind::Icon { .. } => "icon",
            Kind::Frame { .. } => "frame",
        }
    }

    /// Keys a layer of this kind may carry, besides the common ones.
    fn keys(&self) -> &'static [&'static str] {
        match self {
            Kind::Image { .. } => &["asset", "fit", "crop", "tileScale", "focus"],
            Kind::Text { .. } => &[
                "text",
                "ranges",
                "resize",
                "maxLines",
                "minFontScale",
                "ellipsis",
                "fontSize",
                "weight",
                "align",
                "color",
                "fontFamily",
                "letterSpacing",
                "lineHeight",
                "textCase",
                "shadow",
                "fill",
                "gradient",
                "outline",
                "style",
            ],
            Kind::Rect { .. } => &["color", "gradient", "stroke", "cornerRadius"],
            Kind::Ellipse { .. } => &["color", "gradient", "stroke"],
            Kind::Line { .. } => &["color", "strokeWidth"],
            Kind::Icon { .. } => &["name", "set", "color", "strokeWidth"],
            Kind::Frame { .. } => &[
                "children",
                "clip",
                "color",
                "gradient",
                "stroke",
                "cornerRadius",
                "stack",
            ],
        }
    }
}

const COMMON_KEYS: &[&str] = &[
    "type",
    "id",
    "role",
    "x",
    "y",
    "width",
    "height",
    "constraints",
    "opacity",
    "rotation",
    "blendMode",
    "mask",
    "at",
];

/// Rejects keys that don't belong to the layer's type, so a typo like
/// `fontsize` fails loudly instead of silently falling back to a default.
pub fn check_keys(json: &serde_json::Value, layer: &Layer) -> Result<(), String> {
    let Some(obj) = json.as_object() else {
        return Err("layer must be an object".into());
    };
    let allowed = layer.kind.keys();
    let unknown: Vec<&str> = obj
        .keys()
        .map(String::as_str)
        .filter(|k| !COMMON_KEYS.contains(k) && !allowed.contains(k))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    // A wrong-case key (`fontsize`) gets a suggestion; anything else, the list.
    let known = || allowed.iter().chain(COMMON_KEYS);
    let hints: Vec<String> = unknown
        .iter()
        .filter_map(|u| {
            known()
                .find(|k| k.eq_ignore_ascii_case(u))
                .map(|k| format!("{u} → {k}"))
        })
        .collect();
    let help = if hints.len() == unknown.len() {
        format!("did you mean {}", hints.join(", "))
    } else {
        format!("allowed: {}", allowed.join(", "))
    };
    Err(format!(
        "unknown field(s) {} for {} layer; {help}",
        unknown.join(", "),
        layer.kind.name()
    ))
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

/// A layer's constraints on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    /// Horizontal constraint.
    #[serde(default)]
    pub h: HConstraint,
    /// Vertical constraint.
    #[serde(default)]
    pub v: VConstraint,
}

impl Constraints {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Horizontal constraint, as in design tools.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HConstraint {
    /// Keep the distance to the parent's left edge.
    #[default]
    Left,
    /// Keep the distance to the parent's right edge.
    Right,
    /// Keep the offset from the parent's center.
    Center,
    /// Keep both margins; the width changes.
    Stretch,
    /// Position and width scale with the parent.
    Scale,
}

/// Vertical constraint, as in design tools.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VConstraint {
    /// Keep the distance to the parent's top edge.
    #[default]
    Top,
    /// Keep the distance to the parent's bottom edge.
    Bottom,
    /// Keep the offset from the parent's center.
    Center,
    /// Keep both margins; the height changes.
    Stretch,
    /// Position and height scale with the parent.
    Scale,
}

/// A constraint on one axis, with `left`/`top` as `Start`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pin {
    /// Pinned to the start edge (left or top).
    Start,
    /// Pinned to the end edge (right or bottom).
    End,
    /// Pinned to the center.
    Center,
    /// Pinned to both edges.
    Stretch,
    /// Proportional to the parent.
    Scale,
}

impl From<HConstraint> for Pin {
    fn from(c: HConstraint) -> Self {
        match c {
            HConstraint::Left => Pin::Start,
            HConstraint::Right => Pin::End,
            HConstraint::Center => Pin::Center,
            HConstraint::Stretch => Pin::Stretch,
            HConstraint::Scale => Pin::Scale,
        }
    }
}

impl From<VConstraint> for Pin {
    fn from(c: VConstraint) -> Self {
        match c {
            VConstraint::Top => Pin::Start,
            VConstraint::Bottom => Pin::End,
            VConstraint::Center => Pin::Center,
            VConstraint::Stretch => Pin::Stretch,
            VConstraint::Scale => Pin::Scale,
        }
    }
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

/// Text keys a style may set: everything but the content itself.
const STYLE_KEYS: &[&str] = &[
    "resize",
    "maxLines",
    "minFontScale",
    "ellipsis",
    "fontSize",
    "weight",
    "align",
    "color",
    "fontFamily",
    "letterSpacing",
    "lineHeight",
    "textCase",
    "shadow",
    "fill",
    "gradient",
    "outline",
];

/// `l` with its text style applied under its own fields, or `None` when it
/// has no style. A field the layer leaves at its default takes the style's.
fn styled(
    l: &Layer,
    styles: &BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
) -> Result<Option<Layer>, String> {
    let Kind::Text {
        style: Some(name), ..
    } = &l.kind
    else {
        return Ok(None);
    };
    let style = styles
        .get(name)
        .ok_or_else(|| format!("unknown style {name}"))?;
    let mut v = serde_json::to_value(l).map_err(|e| e.to_string())?;
    let obj = v.as_object_mut().ok_or("layer isn't an object")?;
    for (k, val) in style {
        obj.entry(k.clone()).or_insert_with(|| val.clone());
    }
    obj.remove("style");
    serde_json::from_value(v)
        .map(Some)
        .map_err(|e| format!("style {name}: {e}"))
}

/// `l` with its changes for `size` merged over its own fields, or `None`
/// when it has none.
fn sized(l: &Layer, size: &str) -> Result<Option<Layer>, String> {
    let Some(patch) = l.at.get(size) else {
        return Ok(None);
    };
    if let Some(k) = ["id", "type", "children", "at"]
        .iter()
        .find(|k| patch.contains_key(**k))
    {
        return Err(format!("at.{size} can't change {k}"));
    }
    let mut v = serde_json::to_value(l).map_err(|e| e.to_string())?;
    crate::ops::merge_patch(&mut v, &serde_json::Value::Object(patch.clone()));
    if let Some(o) = v.as_object_mut() {
        o.remove("at");
    }
    let layer: Layer = serde_json::from_value(v.clone()).map_err(|e| format!("at.{size}: {e}"))?;
    check_keys(&v, &layer).map_err(|e| format!("at.{size}: {e}"))?;
    Ok(Some(layer))
}

impl Scene {
    /// The scene as drawn at one size: every layer's `at` changes for it
    /// applied. Borrowed when no layer has any. Call on a validated scene.
    pub fn for_size(&self, size: &str) -> std::borrow::Cow<'_, Scene> {
        fn go(layers: &mut [Layer], size: &str) {
            for l in layers {
                if let Ok(Some(s)) = sized(l, size) {
                    *l = s;
                }
                if let Kind::Frame { children, .. } = &mut l.kind {
                    go(children, size);
                }
            }
        }
        let mut any = false;
        self.walk(&mut |l| any |= !l.at.is_empty());
        if !any {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut layers = self.layers.clone();
        go(&mut layers, size);
        std::borrow::Cow::Owned(Scene {
            layers,
            ..self.clone()
        })
    }

    /// The scene as drawn: every text layer's style applied. Borrowed when
    /// there are no styles. Call on a validated scene.
    pub fn resolved(&self) -> std::borrow::Cow<'_, Scene> {
        fn go(layers: &mut [Layer], scene: &Scene) {
            for l in layers {
                if let Kind::Frame { children, .. } = &mut l.kind {
                    go(children, scene);
                }
                if let Ok(Some(s)) = styled(l, &scene.styles) {
                    *l = s;
                }
            }
        }
        if self.styles.is_empty() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut layers = self.layers.clone();
        go(&mut layers, self);
        std::borrow::Cow::Owned(Scene {
            layers,
            ..self.clone()
        })
    }

    /// Checks invariants serde can't express. Called after every mutation.
    pub fn validate(&self) -> Result<(), String> {
        for (name, style) in &self.styles {
            if let Some(k) = style.keys().find(|k| !STYLE_KEYS.contains(&k.as_str())) {
                return Err(format!(
                    "style {name}: {k} can't be styled; allowed: {}",
                    STYLE_KEYS.join(", ")
                ));
            }
        }
        let mut result = Ok(());
        self.walk(&mut |l| {
            if result.is_ok()
                && let Err(e) = styled(l, &self.styles)
            {
                result = Err(format!("{}: {e}", l.id));
            }
        });
        result?;
        let mut result = Ok(());
        self.walk(&mut |l| {
            for size in l.at.keys() {
                if result.is_err() {
                    return;
                }
                result = if self.sizes.iter().any(|s| s.id == *size) {
                    sized(l, size).map(drop)
                } else {
                    Err(format!("no size {size}"))
                }
                .map_err(|e| format!("{}: {e}", l.id));
            }
        });
        result?;
        let resolved = self.resolved();
        resolved.validate_resolved()?;
        for size in &self.sizes {
            resolved.for_size(&size.id).validate_resolved()?;
        }
        Ok(())
    }

    fn validate_resolved(&self) -> Result<(), String> {
        if self.width <= 0.0 || self.height <= 0.0 {
            return Err("scene width and height must be > 0".into());
        }
        if self.sizes.is_empty() {
            return Err("scene needs at least one size".into());
        }
        let mut size_ids = std::collections::HashSet::new();
        for s in &self.sizes {
            // Size ids name the rendered files.
            crate::store::check_id(&s.id).map_err(|e| format!("size {e}"))?;
            if s.width < 1.0 || s.height < 1.0 || s.scale <= 0.0 {
                return Err(format!("size {}: width, height >= 1 and scale > 0", s.id));
            }
            if !size_ids.insert(&s.id) {
                return Err(format!("duplicate size id {}", s.id));
            }
        }
        let mut ids = std::collections::HashSet::new();
        let mut result = Ok(());
        self.walk(&mut |l| {
            if result.is_err() {
                return;
            }
            if !ids.insert(l.id.clone()) {
                result = Err(format!("duplicate layer id {}", l.id));
            } else if let Err(e) = self.validate_layer(l) {
                result = Err(format!("{}: {e}", l.id));
            }
        });
        result
    }

    fn validate_layer(&self, l: &Layer) -> Result<(), String> {
        if !(0.0..=1.0).contains(&l.opacity) {
            return Err("opacity must be 0–1".into());
        }
        if let Kind::Rect {
            gradient, stroke, ..
        }
        | Kind::Ellipse {
            gradient, stroke, ..
        }
        | Kind::Frame {
            gradient, stroke, ..
        } = &l.kind
        {
            let strokes = stroke.iter().filter_map(|s| s.gradient.as_ref());
            for g in gradient.iter().chain(strokes) {
                if g.stops.len() < 2 || g.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)) {
                    return Err("gradient needs at least 2 stops, each at 0–1".into());
                }
            }
            if stroke.as_ref().is_some_and(|s| s.width <= 0.0) {
                return Err("stroke width must be > 0".into());
            }
        }
        if let Kind::Frame { stack: Some(s), .. } = &l.kind
            && (s.gap < 0.0 || s.padding < 0.0)
        {
            return Err("stack gap and padding must be >= 0".into());
        }
        if let Some(m) = &l.mask
            && (m.stops.len() < 2 || m.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)))
        {
            return Err("mask needs at least 2 stops, each at 0–1".into());
        }
        if l.width.is_some_and(|w| w < 0.0) || l.height.is_some_and(|h| h < 0.0) {
            return Err("width and height must be >= 0".into());
        }
        match &l.kind {
            Kind::Image { asset, .. } if !self.assets.contains_key(asset) => {
                Err(format!("unknown asset {asset}"))
            }
            Kind::Image { tile_scale, .. } if *tile_scale <= 0.0 => {
                Err("tileScale must be > 0".into())
            }
            Kind::Image { focus, .. } if focus.iter().any(|f| !(0.0..=1.0).contains(f)) => {
                Err("focus must be [x, y], each 0–1".into())
            }
            Kind::Line { stroke_width, .. } if *stroke_width <= 0.0 => {
                Err("strokeWidth must be > 0".into())
            }
            Kind::Icon { stroke_width: Some(w), .. } if *w <= 0.0 => {
                Err("strokeWidth must be > 0".into())
            }
            Kind::Icon { name, set, .. } => crate::icons::check(*set, name),
            Kind::Image {
                crop: Some(c), ..
            } if c.width <= 0.0 || c.height <= 0.0 || c.x < 0.0 || c.y < 0.0 || c.x + c.width > 1.001 || c.y + c.height > 1.001 => {
                Err("crop must lie within the image: x, y ≥ 0, width, height > 0, x+width and y+height ≤ 1".into())
            }
            Kind::Text {
                text,
                ranges,
                weight,
                font_size,
                min_font_scale,
                font_family,
                fill,
                gradient,
                outline,
                ..
            } => {
                if let Some(f) = fill {
                    if !self.assets.contains_key(&f.asset) {
                        return Err(format!("unknown asset {} in fill", f.asset));
                    }
                    if f.tile_scale <= 0.0 {
                        return Err("fill tileScale must be > 0".into());
                    }
                }
                if gradient
                    .as_ref()
                    .is_some_and(|g| g.stops.len() < 2 || g.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)))
                {
                    return Err("gradient needs at least 2 stops, each at 0–1".into());
                }
                if outline.as_ref().is_some_and(|o| o.width <= 0.0) {
                    return Err("outline width must be > 0".into());
                }
                if !(100..=900).contains(weight) || weight % 100 != 0 {
                    return Err("weight must be 100, 200, … 900".into());
                }
                if !crate::text::families().contains(font_family) {
                    return Err(format!(
                        "unknown fontFamily {font_family}; available: {}",
                        crate::text::families().join(", ")
                    ));
                }
                if *font_size <= 0.0 {
                    return Err("fontSize must be > 0".into());
                }
                if !(*min_font_scale > 0.0 && *min_font_scale <= 1.0) {
                    return Err("minFontScale must be > 0 and <= 1".into());
                }
                let resize = l.text_resize();
                if resize == Some(Resize::AutoHeight) && l.width.is_none() {
                    return Err("auto-height text needs a width".into());
                }
                if matches!(resize, Some(Resize::Fit | Resize::Fixed | Resize::Truncate))
                    && (l.width.is_none() || l.height.is_none())
                {
                    return Err("fit, fixed and truncate text need width and height".into());
                }
                let n = text.chars().count();
                match ranges.iter().find(|r| r.start >= r.end || r.end > n) {
                    Some(r) => Err(format!(
                        "range {}..{} out of bounds for {n} characters",
                        r.start, r.end
                    )),
                    None => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }

    /// Visits every layer depth-first, parents before children.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Layer)) {
        fn go<'a>(layers: &'a [Layer], f: &mut impl FnMut(&'a Layer)) {
            for l in layers {
                f(l);
                if let Kind::Frame { children, .. } = &l.kind {
                    go(children, f);
                }
            }
        }
        go(&self.layers, f);
    }
}

fn white() -> Color {
    Color(0xFFFF_FFFF)
}
fn black() -> Color {
    Color(0xFF00_0000)
}
fn is_black(c: &Color) -> bool {
    *c == black()
}
fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn sixteen() -> f32 {
    16.0
}
fn is_sixteen(v: &f32) -> bool {
    *v == 16.0
}
fn w400() -> u16 {
    400
}
fn is_w400(v: &u16) -> bool {
    *v == 400
}
fn inter() -> String {
    "Inter".into()
}
fn is_inter(v: &String) -> bool {
    v == "Inter"
}
fn left_mid() -> [f32; 2] {
    [0.0, 0.5]
}
fn right_mid() -> [f32; 2] {
    [1.0, 0.5]
}
fn center() -> [f32; 2] {
    [0.5, 0.5]
}
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_center(v: &[f32; 2]) -> bool {
    *v == center()
}
fn half() -> f32 {
    0.5
}
fn is_half(v: &f32) -> bool {
    *v == 0.5
}
fn yes() -> bool {
    true
}
fn is_true(v: &bool) -> bool {
    *v
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn at_changes_a_layer_for_one_size_only() {
        let mut s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100,
            "sizes": [{"id": "a", "width": 100, "height": 100}, {"id": "b", "width": 50, "height": 100}],
            "layers": [{"id": "t", "type": "text", "text": "Hi", "fontSize": 10,
                        "at": {"b": {"fontSize": 20, "color": "#FF0000"}}}]
        }))
        .unwrap();
        s.validate().unwrap();
        let size = |id: &str| match &s.for_size(id).layers[0].kind {
            Kind::Text {
                font_size, color, ..
            } => (*font_size, color.to_string()),
            _ => unreachable!(),
        };
        assert_eq!(size("a"), (10.0, "#000000".into()));
        assert_eq!(size("b"), (20.0, "#FF0000".into()));

        s.layers[0].at.insert("c".into(), serde_json::Map::new());
        assert!(s.validate().unwrap_err().contains("no size c"));
        s.layers[0].at.remove("c");
        let bad = serde_json::json!({"type": "rect"});
        s.layers[0]
            .at
            .insert("b".into(), bad.as_object().unwrap().clone());
        assert!(s.validate().unwrap_err().contains("at.b can't change type"));
        let typo = serde_json::json!({"fontsize": 20});
        s.layers[0]
            .at
            .insert("b".into(), typo.as_object().unwrap().clone());
        assert!(s.validate().unwrap_err().contains("fontsize → fontSize"));
    }

    #[test]
    fn styles_fill_in_what_a_text_layer_leaves_out() {
        let mut s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "styles": {"label": {"fontSize": 30, "weight": 700, "color": "#FF0000"}},
            "layers": [
                {"id": "a", "type": "text", "text": "A", "style": "label"},
                {"id": "b", "type": "text", "text": "B", "style": "label", "color": "#0000FF"}
            ]
        }))
        .unwrap();
        s.validate().unwrap();
        let r = s.resolved();
        let look = |i: usize| match &r.layers[i].kind {
            Kind::Text {
                font_size,
                weight,
                color,
                ..
            } => (*font_size, *weight, color.to_string()),
            _ => unreachable!(),
        };
        assert_eq!(look(0), (30.0, 700, "#FF0000".into()));
        // The layer's own color wins over the style's.
        assert_eq!(look(1), (30.0, 700, "#0000FF".into()));

        s.styles
            .get_mut("label")
            .unwrap()
            .insert("text".into(), "x".into());
        assert!(s.validate().unwrap_err().contains("text can't be styled"));
        s.styles.clear();
        assert!(s.validate().unwrap_err().contains("unknown style label"));
    }

    #[test]
    fn icons_must_exist_in_their_set() {
        let s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [{"type": "icon", "name": "envelope"}]
        }))
        .unwrap();
        // Font Awesome has envelope; Lucide, the default set, calls it mail.
        assert!(
            s.validate()
                .unwrap_err()
                .contains("no lucide icon envelope")
        );
    }

    #[test]
    fn color_round_trips() {
        for s in ["#1B2A5C", "#00000040"] {
            assert_eq!(Color::parse(s).unwrap().to_string(), s);
        }
        assert_eq!(Color::parse("#FFFFFF80").unwrap().0, 0x80FF_FFFF);
        assert!(Color::parse("red").is_none());
        assert!(Color::parse("#FFF").is_none());
    }

    #[test]
    fn defaults_are_omitted_on_serialize() {
        let l: Layer =
            serde_json::from_value(json!({"id": "t", "type": "text", "text": "hi"})).unwrap();
        assert_eq!(
            serde_json::to_value(&l).unwrap(),
            json!({"id": "t", "type": "text", "text": "hi"})
        );
    }

    #[test]
    fn constraint_names_parse() {
        let c: Constraints = serde_json::from_value(json!({"h": "right", "v": "bottom"})).unwrap();
        assert_eq!(Pin::from(c.h), Pin::End);
        assert_eq!(Pin::from(c.v), Pin::End);
        assert!(serde_json::from_value::<Constraints>(json!({"h": "top"})).is_err());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let v = json!({"type": "text", "text": "hi", "fontsize": 12});
        let l: Layer = serde_json::from_value(v.clone()).unwrap();
        let err = check_keys(&v, &l).unwrap_err();
        assert!(err.ends_with("did you mean fontsize → fontSize"), "{err}");
        let v = json!({"type": "text", "text": "hi", "bogus": 1});
        assert!(
            check_keys(&v, &l)
                .unwrap_err()
                .contains("allowed: text, ranges")
        );
    }

    #[test]
    fn validate_catches_bad_ranges_and_duplicates() {
        let mut s: Scene = serde_json::from_value(json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [{"id": "t", "type": "text", "text": "abc", "ranges": [{"start": 1, "end": 5, "color": "#FF0000"}]}]
        }))
        .unwrap();
        assert!(s.validate().unwrap_err().contains("out of bounds"));
        s.layers[0] = serde_json::from_value(json!({"id": "t", "type": "rect"})).unwrap();
        s.layers.push(s.layers[0].clone());
        assert!(s.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn size_ids_must_work_as_file_names() {
        let s: Scene = serde_json::from_value(json!({
            "width": 10, "height": 10, "sizes": [{"id": "1080x1350 portrait", "width": 10, "height": 10}]
        }))
        .unwrap();
        assert!(s.validate().unwrap_err().contains("bad id"));
    }

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
