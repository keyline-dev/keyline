//! Layer types: what a layer is, with its type's own fields.

use serde::{Deserialize, Serialize};

use super::defaults::{
    black, center, half, inter, is_black, is_center, is_default, is_half, is_inter, is_one,
    is_sixteen, is_true, is_w400, is_zero, one, sixteen, w400, yes,
};
use super::{
    Adjust, Align, Color, Crop, Fit, Gradient, ImageFill, Layer, Outline, Range, Resize, Shadow,
    Stack, Stroke, TextCase, TextMore,
};

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
        /// Color adjustments (brightness, contrast, saturate, grayscale,
        /// sepia, hue, duotone, tint).
        #[serde(default, skip_serializing_if = "Adjust::is_none")]
        adjust: Adjust,
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
        /// Italic, decoration, wrapping, vertical alignment, trim,
        /// highlight, direction, features, padding, curve, leader, knockout.
        #[serde(flatten)]
        more: TextMore,
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
        /// Part of the ellipse: a pie, a ring or a ring segment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arc: Option<Arc>,
    },
    /// A regular polygon inscribed in the box, or a star with `innerRadius`;
    /// round its points with `radius` (a scalloped seal).
    #[serde(rename_all = "camelCase")]
    Polygon {
        /// Solid fill; none when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        /// Gradient fill; wins over `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        /// Outline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
        /// Number of corners (or star points), 3 or more (3).
        #[serde(default = "three", skip_serializing_if = "is_three")]
        sides: u32,
        /// Makes a star: every other corner moves in to this share of the
        /// outer radius (0.38 a classic star, 0.8 a starburst).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inner_radius: Option<f32>,
    },
    /// An SVG path: your own `d`, or a built-in `shape` by name, fitted to the box.
    #[serde(rename_all = "camelCase")]
    Path {
        /// Solid fill; none when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        /// Gradient fill; wins over `color`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        /// Outline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
        /// SVG path data.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        d: Option<String>,
        /// A built-in shape: ribbon, bubble, arrow, blob-1, …
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shape: Option<String>,
        /// Which regions count as inside (`nonzero`).
        #[serde(default, skip_serializing_if = "is_default")]
        fill_rule: FillRule,
        /// Keep the path's aspect ratio (`contain`) or stretch it to the box.
        #[serde(default, skip_serializing_if = "is_default")]
        fit_path: FitPath,
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
    /// Flexible empty space in a stack: it takes the free space, like
    /// SwiftUI's `Spacer`. Outside a stack it's an empty box.
    #[serde(rename_all = "camelCase")]
    Spacer {
        /// Smallest length along the stack, px (0).
        #[serde(default, skip_serializing_if = "is_zero")]
        min_length: f32,
    },
    /// Draws the first child that fits its box, like SwiftUI's
    /// `ViewThatFits`; e.g. a long headline, then a short one.
    #[serde(rename_all = "camelCase")]
    FirstFit {
        /// The alternatives, in order of preference.
        #[serde(default)]
        children: Vec<Layer>,
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

impl Kind {
    /// Child layers, for types that hold them (frames and `firstFit`).
    pub fn children(&self) -> Option<&Vec<Layer>> {
        match self {
            Kind::Frame { children, .. } | Kind::FirstFit { children } => Some(children),
            _ => None,
        }
    }

    /// Child layers, mutably.
    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match self {
            Kind::Frame { children, .. } | Kind::FirstFit { children } => Some(children),
            _ => None,
        }
    }

    /// The type's name, as in `type`.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Image { .. } => "image",
            Kind::Text { .. } => "text",
            Kind::Rect { .. } => "rect",
            Kind::Ellipse { .. } => "ellipse",
            Kind::Polygon { .. } => "polygon",
            Kind::Path { .. } => "path",
            Kind::Line { .. } => "line",
            Kind::Icon { .. } => "icon",
            Kind::Frame { .. } => "frame",
            Kind::Spacer { .. } => "spacer",
            Kind::FirstFit { .. } => "firstFit",
        }
    }
}

/// Part of an ellipse: angles in degrees clockwise from the top.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Arc {
    /// Start angle (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub start: f32,
    /// End angle (360).
    #[serde(default = "full_turn", skip_serializing_if = "is_full_turn")]
    pub end: f32,
    /// Hole in the middle as a share of the radius, 0–1: a ring (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub inner: f32,
}

/// Which regions of a path count as inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillRule {
    /// Non-zero winding.
    #[default]
    Nonzero,
    /// Even-odd: overlaps make holes.
    Evenodd,
}

/// How a path is fitted to its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitPath {
    /// Scaled evenly to fit, centered.
    #[default]
    Contain,
    /// Stretched to the box on each axis.
    Stretch,
}

fn three() -> u32 {
    3
}
fn is_three(v: &u32) -> bool {
    *v == 3
}
fn full_turn() -> f32 {
    360.0
}
fn is_full_turn(v: &f32) -> bool {
    *v == 360.0
}
