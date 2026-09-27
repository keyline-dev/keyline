//! Layers: the fields every layer shares, and each type's own in [`Kind`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::defaults::{is_default, is_false, is_one, is_zero, one};
use super::{BlendMode, Constraints, Inset, Kind, Length, Look, Mask, Place, Position, StackAlign};

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
    /// Left edge relative to the parent frame: px, or `"25%"` of the
    /// parent's width at every size.
    #[serde(default, skip_serializing_if = "Length::is_zero")]
    pub x: Length,
    /// Top edge relative to the parent frame: px, or a share of its height.
    #[serde(default, skip_serializing_if = "Length::is_zero")]
    pub y: Length,
    /// Width: px, `"hug"` (fit the content), `"fill"` (the free space in a
    /// stack; the rest of the parent from `x` in free layout) or `"40%"` of
    /// the parent. Images and text size themselves when omitted; other
    /// layers are 100 wide, and stacked frames hug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Length>,
    /// Height, as `width`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Length>,
    /// Smallest width, px, after every other sizing rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f32>,
    /// Largest width, px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    /// Smallest height, px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    /// Largest height, px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f32>,
    /// Width ÷ height kept when only one side is set; the other follows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f32>,
    /// Pins the layer to one of nine spots of its parent, e.g.
    /// `"bottom-right"`, at `inset` from the edges; wins over `x`, `y` and
    /// `constraints`, and holds at every size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
    /// Distance from the parent's edges for `place`, px or `[x, y]` (0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inset: Option<Inset>,
    /// Not drawn and takes no space (default false); handy per size via `at`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// In a stack: this child's cross-axis placement, over the stack's `align`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_self: Option<StackAlign>,
    /// In a stack: this child's share of the free space when it `fill`s (1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub grow: f32,
    /// In a stack that's too small: lower priorities give way first, like
    /// SwiftUI's `layoutPriority` (0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub priority: f32,
    /// In a stack: `absolute` takes the child out of the flow (default `auto`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub position: Position,
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
    /// What part of the layer shows: a gradient (its alpha fades the layer,
    /// e.g. a photo fading out), a shape name, `{path}`, `{layer}` or
    /// `{image}`, with `mode` alpha|luminance and `invert`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    /// Fills, strokes, shadows, blur, radius and post-layout transforms.
    #[serde(flatten)]
    pub look: Look,
    /// A named style, or several applied in order (a later one wins); the
    /// layer's own fields win over all of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleRef>,
    /// Which fields came from which token, by JSON pointer (`"/color": "brand"`),
    /// so changing the token updates them. Kept by the server.
    #[serde(
        rename = "$tokens",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub token_refs: BTreeMap<String, String>,
    /// Per-size changes, by size id: fields merged over this layer's own
    /// for that size only, e.g. `{"sky": {"fontSize": 20}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub at: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    /// The type-specific part, tagged by `type`.
    #[serde(flatten)]
    pub kind: Kind,
}

/// One style name, or several applied in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StyleRef {
    /// One style.
    One(String),
    /// Several, a later one winning where they overlap.
    Many(Vec<String>),
}

impl StyleRef {
    /// The style names, in order.
    pub fn names(&self) -> &[String] {
        match self {
            StyleRef::One(n) => std::slice::from_ref(n),
            StyleRef::Many(v) => v,
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
