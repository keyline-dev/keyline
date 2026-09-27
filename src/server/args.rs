//! Arguments of the MCP tools, with the field docs the agent sees.

use serde::Deserialize;
use serde_json::Value;

use crate::ops::Op;
use crate::scene::Size;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `scene_create`.
pub struct SceneCreateArgs {
    /// Master size, px.
    pub(super) width: f32,
    pub(super) height: f32,
    /// Target sizes. `scale` (default 1) shrinks everything, fonts included, before constraints apply.
    pub(super) sizes: Vec<Size>,
    /// Hex color, default #FFFFFF.
    pub(super) background: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `asset_add`.
pub struct AssetAddArgs {
    pub(super) scene_id: String,
    /// Public http(s) URL of a PNG, JPEG or SVG.
    pub(super) url: Option<String>,
    /// Or the file bytes, base64.
    pub(super) base64: Option<String>,
    /// Asset id to use in layers; generated if omitted.
    pub(super) id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `layer_add`.
pub struct LayerAddArgs {
    pub(super) scene_id: String,
    /// Named text styles to add or replace, used by text layers' `style`.
    #[serde(default)]
    pub(super) styles: serde_json::Map<String, Value>,
    pub(super) layers: Vec<Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `layer_update`.
pub struct LayerUpdateArgs {
    pub(super) scene_id: String,
    pub(super) ops: Vec<Op>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `scene_describe`.
pub struct SceneDescribeArgs {
    pub(super) scene_id: String,
    /// One size id; all sizes if omitted.
    pub(super) size: Option<String>,
    /// Every layer's box, not just warnings.
    #[serde(default)]
    pub(super) full: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `render`.
pub struct RenderArgs {
    pub(super) scene_id: String,
    /// Size ids; all if omitted.
    pub(super) sizes: Option<Vec<String>>,
    /// Also return one small image of all sizes side by side.
    #[serde(default)]
    pub(super) preview: bool,
}
