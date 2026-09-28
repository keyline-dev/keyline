//! The scene document: the JSON the agent edits and the renderer draws.
//!
//! Every field has a default and defaults are omitted when serialized, so the
//! stored JSON stays as small as what the agent wrote.

mod check;
mod constraints;
mod de;
mod defaults;
mod fill;
mod gradient;
mod grid;
mod keys;
mod kind;
mod layer;
mod length;
mod look;
mod mask;
mod paint;
mod presets;
mod resolve;
mod stack;
mod stroke;
mod text;
mod text_more;
mod validate;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use constraints::{Constraints, HConstraint, Pin, VConstraint};
pub use fill::{
    Adjust, Common, GradientFill, ImagePaint, NoisePaint, Paint, PatternKind, PatternPaint, Solid,
};
pub use gradient::{Gradient, GradientKind, Stop};
pub use grid::{Area, Grid, MAX_TRACKS, Track, Tracks};
pub use keys::check_keys;
pub use kind::{Arc, FillRule, FitPath, IconSet, Kind};
pub use layer::{Layer, StyleRef};
pub use length::{Inset, Length, Place, Position, Spot};
pub use look::{EdgeStyle, Edges, Look, OneOrMany, Radius, Side};
pub use mask::{Mask, MaskMode, MaskSource};
pub use paint::{BlendMode, Color, Crop, Fit, ImageFill, Outline};
pub use presets::{PRESETS, SizeSpec};
pub use resolve::{ASPECT_CLASSES, aspect_classes};
pub use stack::{Dir, Dirs, Gap, Justify, Padding, Stack, StackAlign};
pub use stroke::{Cap, Join, Marker, Shadow, Stroke, StrokeAlign, StrokeWidth};
pub use text::{Align, Range, Resize, Shift, TextCase};
pub use text_more::{
    Decoration, Direction, Highlight, HighlightStyle, TextMore, TextWrap, Trim, VAlign,
};

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
    /// Named styles: layer fields (`fontSize`, `color`, `fills`, `radius`,
    /// …) that a layer's `style` pulls in under its own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    /// Named values any layer field can use as `"$name"`: colors, sizes, …
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tokens: BTreeMap<String, serde_json::Value>,
    /// Named layer trees placed by `use` layers; `{prop}` in their strings
    /// is filled from each instance's props.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, serde_json::Value>,
    /// Length, seconds: its presence makes the scene animated (none: a
    /// still).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f32>,
    /// Frames per second of animated output (30).
    #[serde(default = "thirty", skip_serializing_if = "is_thirty")]
    pub fps: f32,
    /// The animation is meant to loop: an animated PNG repeats forever
    /// (false: it plays once).
    #[serde(rename = "loop", default, skip_serializing_if = "is_false")]
    pub looping: bool,
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
    /// Insets the platform covers, px `[top, right, bottom, left]` (a
    /// story's UI bars); `scene_describe` flags text inside them (0).
    #[serde(default, skip_serializing_if = "is_zero4")]
    pub safe: [f32; 4],
}

fn is_zero4(v: &[f32; 4]) -> bool {
    *v == [0.0; 4]
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
    /// For a video clip: its length and frame rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<Clip>,
}

/// A video clip's timing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    /// Length, seconds.
    pub duration: f32,
    /// Frames per second.
    pub fps: f32,
    /// Whether it has sound.
    #[serde(default, skip_serializing_if = "is_false")]
    pub audio: bool,
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

fn thirty() -> f32 {
    30.0
}

fn is_thirty(v: &f32) -> bool {
    *v == 30.0
}
