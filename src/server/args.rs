//! Arguments of the MCP tools, with the field docs the agent sees.

use serde::Deserialize;
use serde_json::Value;

use crate::ops::Op;
use crate::scene::SizeSpec;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `scene_create`.
pub struct SceneCreateArgs {
    /// Master size, px; the first size's when omitted.
    pub(super) width: Option<f32>,
    pub(super) height: Option<f32>,
    /// Target sizes: {id, width, height, scale, safe}, a preset (instagram-portrait|-square|-story,
    /// facebook-feed, linkedin-post, x-post, youtube-thumbnail, iab-medium-rectangle|-leaderboard|-skyscraper|-half-page,
    /// a4-portrait) or "WxH". `scale` (default 1) shrinks everything, fonts included, before constraints apply.
    pub(super) sizes: Vec<SizeSpec>,
    /// Hex color, default #FFFFFF.
    pub(super) background: Option<String>,
    /// Seconds: makes it move
    pub(super) duration: Option<f32>,
    /// (30)
    pub(super) fps: Option<f32>,
    #[serde(default, rename = "loop")]
    pub(super) looping: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `asset_add`.
pub struct AssetAddArgs {
    pub(super) scene_id: String,
    /// Public http(s) URL of a PNG, JPEG or SVG.
    pub(super) url: Option<String>,
    /// Or a local file, in a folder the server allows (--allow-read).
    pub(super) path: Option<String>,
    /// Or the file bytes, base64 (small files only: they pass through the model).
    pub(super) base64: Option<String>,
    /// Asset id to use in layers; generated if omitted.
    pub(super) id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `layer_add`.
pub struct LayerAddArgs {
    pub(super) scene_id: String,
    /// Named styles to add or replace: layer fields a layer's `style` pulls in.
    #[serde(default)]
    pub(super) styles: serde_json::Map<String, Value>,
    /// Named values to add or replace, used as "$name" in any field.
    #[serde(default)]
    pub(super) tokens: serde_json::Map<String, Value>,
    /// Named layer trees to add or replace, placed by `use` layers.
    #[serde(default)]
    pub(super) components: serde_json::Map<String, Value>,
    pub(super) layers: Vec<Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `layer_update`.
pub struct LayerUpdateArgs {
    pub(super) scene_id: String,
    /// Tokens to change; every field using one follows.
    #[serde(default)]
    pub(super) tokens: serde_json::Map<String, Value>,
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
    pub(super) time: Option<f32>,
    /// png|jpeg|webp|pdf|apng|gif
    #[serde(default)]
    pub(super) format: crate::render::Format,
    /// 0–100 (90)
    pub(super) quality: Option<u32>,
    #[serde(rename = "maxKB")]
    pub(super) max_kb: Option<u32>,
}
