//! Layers: the fields every layer shares, and each type's own in [`Kind`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::defaults::{
    black, center, half, inter, is_black, is_center, is_default, is_half, is_inter, is_one,
    is_sixteen, is_true, is_w400, is_zero, one, sixteen, w400, yes,
};
use super::{
    Align, BlendMode, Color, Constraints, Crop, Fit, Gradient, ImageFill, Outline, Range, Resize,
    Shadow, Stack, Stroke, TextCase,
};

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
}

#[cfg(test)]
mod tests {
    use crate::scene::Layer;
    use serde_json::json;

    #[test]
    fn defaults_are_omitted_on_serialize() {
        let l: Layer =
            serde_json::from_value(json!({"id": "t", "type": "text", "text": "hi"})).unwrap();
        assert_eq!(
            serde_json::to_value(&l).unwrap(),
            json!({"id": "t", "type": "text", "text": "hi"})
        );
    }
}
