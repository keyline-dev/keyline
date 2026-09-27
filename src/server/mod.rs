//! The MCP surface: six batched, verb-shaped tools with terse descriptions
//! and plain-text results, to keep the agent's token spend low.

mod args;
mod handlers;
mod schema;

use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::describe::{describe, facts, warnings};
use crate::gpu::Backend;
use crate::ops;
use crate::scene::Scene;
use crate::store::Store;

pub use args::{
    AssetAddArgs, LayerAddArgs, LayerUpdateArgs, RenderArgs, SceneCreateArgs, SceneDescribeArgs,
};

const INSTRUCTIONS: &str = "Compose images from a JSON scene and export several sizes. \
Author once at the master size; constraints and per-size scale adapt it to every size. \
Add all layers in one layer_add; give repeated text looks a style. Every edit replies ok or with problems per size: !defects (overflow with the \
size that fits, clipping, hidden or overlapping text) to fix with layer_update, and warn advisories (contrast) \
for you to judge. It also states each size's smallest text and any upscaled photo; decide whether that suits \
where the design will be used, and adapt a size with at. \
The server's checks are the verification: render once at the end, and its reply shows how wrapped, shrunk or \
cut text came out. Ask for a preview only when you must judge the look. \
Omit fields that match defaults. When done, reply in one short line.";

/// Height of each size in the preview contact sheet.
const PREVIEW_HEIGHT: f32 = 384.0;

/// The MCP server: the six tools over a scene store.
#[derive(Clone)]
pub struct Server {
    store: Arc<Store>,
    // ponytail: one global lock for load-modify-save; per-scene locks if
    // several agents ever share a server.
    lock: Arc<Mutex<()>>,
    /// Final renders: GPU (default) or CPU.
    backend: Backend,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Server {
    /// A server over `store`, rendering final PNGs on `backend`.
    pub fn new(store: Store, backend: Backend) -> Self {
        let mut tool_router = Self::tool_router();
        schema::compact_all(&mut tool_router);
        Server {
            store: Arc::new(store),
            lock: Arc::new(Mutex::new(())),
            backend,
            tool_router,
        }
    }

    #[tool(description = "Create a scene. Returns its id.")]
    async fn scene_create(&self, Parameters(a): Parameters<SceneCreateArgs>) -> CallToolResult {
        reply(self.scene_create_impl(a).await)
    }

    #[tool(description = "Add an image to a scene's assets. Returns id and size.")]
    async fn asset_add(&self, Parameters(a): Parameters<AssetAddArgs>) -> CallToolResult {
        reply(self.asset_add_impl(a).await)
    }

    #[tool(
        description = "Add layers, drawn bottom to top. Layer: {type, id?, role?, parent? (frame id), x, y (px or \"25%\" \
of the parent), width, height (px, \"hug\" content, \"fill\" = rest of the parent or a stack's free space, \"40%\"), \
minWidth…maxHeight, aspectRatio, place top-left|top|…|center|…|bottom-right + inset (pinned at every size), \
constraints {h: left|right|center|stretch|scale, v: top|bottom|center|stretch|scale} (default left, top), hidden, \
opacity, rotation (degrees), blendMode (multiply, screen, …), mask (gradient; its alpha fades the layer), \
at {sizeId or landscape|square|portrait|wide|tall: {fields}}: changes for those sizes only}. \
image: asset, fit fill|fit|tile (fill = cover; tileScale sizes tiles), focus [x,y] 0–1 kept in view (center), \
crop {x, y, width, height} 0–1. \
text: text, style (name from styles), fontSize (16; the maximum when fitting), weight 100-900, color, align left|center|right, \
maxLines, ranges [{start, end, color}] in characters. Sizing follows the box: width+height → font shrinks to fit \
(down to minFontScale 0.5), then ellipsis; width only → wraps, grows down; neither → one line. resize fixed|truncate \
keeps the font size; ellipsis false cuts without …. Also fontFamily (Inter or any Google Fonts family), \
letterSpacing px, lineHeight (× fontSize), textCase upper|lower, shadow {x, y, blur, color}, outline {width, color}, \
fill {asset, fit, tileScale} or gradient paints the letters. \
styles {name: {text fields}}: shared text fields; a layer's own fields win. \
icon: name, set lucide (outline; mail, map-pin, landmark…)|solid|regular|brands (Font Awesome), color, strokeWidth; \
24px tall unless sized. rect: color, cornerRadius. ellipse: color. line: from x,y by width,height; color, strokeWidth (1). \
frame: children[], clip (true), color, cornerRadius, stack {dir row|column|row-reverse|column-reverse or a list \
tried in order (first that fits: [\"row\", \"column\"]), gap (or [row, column]), padding (or [v, h] or [t, r, b, l]), \
align start|center|end|stretch|baseline, justify start|center|end|between|around|evenly, wrap} places children in \
order ignoring their x, y; unsized, it hugs them. Stack children: alignSelf, grow (fill share), priority (lower \
gives way first when tight), position absolute (out of the flow). spacer: free space in a stack. firstFit: \
children[], draws only the first that fits its box (e.g. a long, then a short headline). \
rect, ellipse, frame: gradient {from: [x,y], to: [x,y] 0–1 of the box, stops: [{at, color}]}, \
stroke {width, color or gradient, align inside|center|outside}. Colors #RRGGBB[AA]."
    )]
    async fn layer_add(&self, Parameters(a): Parameters<LayerAddArgs>) -> CallToolResult {
        let styles = Value::Object(a.styles.clone());
        let fetched = match self.fetch_fonts(&[&a.layers[..], &[styles]].concat()).await {
            Ok(f) => f,
            Err(e) => return reply(Err(e)),
        };
        reply(
            self.edit(&a.scene_id, |s| ops::add_layers(s, a.styles, a.layers))
                .await
                .map(|(ids, s)| fetched + &edited("added", &ids, &s, &self.store.assets_dir())),
        )
    }

    #[tool(
        description = "Change or delete layers atomically. Each op: {target: {id}|{role}|{style}, set: {fields}} \
or {target, delete: true}. A role targets every layer with it; a style target creates or changes that text style. \
null resets a field."
    )]
    async fn layer_update(&self, Parameters(a): Parameters<LayerUpdateArgs>) -> CallToolResult {
        let sets: Vec<Value> = a
            .ops
            .iter()
            .filter_map(|op| op.set.clone().map(Value::Object))
            .collect();
        let fetched = match self.fetch_fonts(&sets).await {
            Ok(f) => f,
            Err(e) => return reply(Err(e)),
        };
        reply(
            self.edit(&a.scene_id, |s| ops::update_layers(s, &a.ops))
                .await
                .map(|(ids, s)| fetched + &edited("changed", &ids, &s, &self.store.assets_dir())),
        )
    }

    #[tool(
        description = "Problems per size, or ok. Defects: !overflow !truncated !clipped !hidden !overlaps !unsafe (under a size's safe insets). \
Advisory: warn contrast. full: one line per layer per size: id type x,y w×h, font px, lines, image crop, upscale."
    )]
    async fn scene_describe(&self, Parameters(a): Parameters<SceneDescribeArgs>) -> CallToolResult {
        reply(self.store.load(&a.scene_id).map_err(err).and_then(|s| {
            describe(
                &s.resolved(),
                a.size.as_deref(),
                a.full,
                Some(&self.store.assets_dir()),
            )
        }))
    }

    #[tool(
        description = "Render PNGs. Returns each size's path and how wrapped, shrunk or cut text was drawn. \
preview adds one small image of all sizes."
    )]
    async fn render(&self, Parameters(a): Parameters<RenderArgs>) -> CallToolResult {
        match self.render_impl(a).await {
            Ok(content) => CallToolResult::success(content),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            // Not `Implementation::from_build_env`, which names rmcp itself.
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(format!(
                "{INSTRUCTIONS} Fonts: any Google Fonts family (downloaded once, then cached); installed now: {}.",
                crate::text::families().join(", ")
            ))
    }
}

/// `added a,b v3 ok`, or the problems instead of `ok`, then one line of facts
/// (smallest text, upscaled photos), so the agent never needs a
/// `scene_describe` round trip to check an edit.
// ponytail: checks (including a text-free backdrop render per size) run on
// the async thread; move to spawn_blocking if edits on big photos lag.
fn edited(verb: &str, ids: &[String], scene: &Scene, assets: &std::path::Path) -> String {
    let head = format!("{verb} {} v{}", ids.join(","), scene.version);
    let scene = &*scene.resolved();
    let mut out = match warnings(scene, Some(assets)) {
        Some(w) => format!("{head}\n{}", w.trim_end()),
        None => format!("{head} ok"),
    };
    let facts = facts(scene);
    if !facts.is_empty() {
        out.push('\n');
        out.push_str(&facts);
    }
    out
}

fn reply(r: Result<String, String>) -> CallToolResult {
    match r {
        Ok(s) => CallToolResult::success(vec![ContentBlock::text(s)]),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
    }
}

#[expect(clippy::needless_pass_by_value, reason = "shaped for `.map_err(err)`")]
fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}
