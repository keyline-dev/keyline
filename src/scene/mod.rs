//! MVP scene document: the JSON the agent edits and the renderer draws.
//!
//! Every field has a default and defaults are omitted when serialized, so the
//! stored JSON stays as small as what the agent wrote.

mod constraints;
mod defaults;
mod keys;
mod layer;
mod length;
mod paint;
mod resolve;
mod stack;
mod text;
mod validate;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use constraints::{Constraints, HConstraint, Pin, VConstraint};
pub use keys::check_keys;
pub use layer::{IconSet, Kind, Layer};
pub use length::{Inset, Length, Place, Position, Spot};
pub use paint::{
    BlendMode, Color, Crop, Fit, Gradient, ImageFill, Outline, Shadow, Stop, Stroke, StrokeAlign,
};
pub use resolve::{ASPECT_CLASSES, aspect_classes};
pub use stack::{Dir, Dirs, Gap, Justify, Padding, Stack, StackAlign};
pub use text::{Align, Range, Resize, TextCase};

use defaults::{is_false, is_one, one, white};

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

impl Scene {
    /// Visits every layer depth-first, parents before children.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Layer)) {
        fn go<'a>(layers: &'a [Layer], f: &mut impl FnMut(&'a Layer)) {
            for l in layers {
                f(l);
                if let Some(children) = l.kind.children() {
                    go(children, f);
                }
            }
        }
        go(&self.layers, f);
    }
}
