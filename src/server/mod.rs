//! The MCP surface: six batched, verb-shaped tools with terse descriptions
//! and plain-text results, to keep the agent's token spend low.

mod args;
mod cli;
mod handlers;
mod schema;
mod template;

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
pub use cli::{RenderFile, Report};

const INSTRUCTIONS: &str = "Compose images from a JSON scene and export several sizes. \
Author once at the master size; constraints and per-size scale adapt it to every size. \
Add all layers in one layer_add; give repeated text looks a style. Every edit replies ok or with problems per size: !defects (cut text with the \
size that fits, clipping, hidden or overlapping text) to fix with layer_update, and warn advisories (contrast) \
for you to judge. It also states each size's smallest text and any upscaled photo; decide whether that suits \
where the design will be used, and adapt a size with media. A stack's !overflow means its children don't fit: \
make one shorter at that size (media) or let it grow less. A top-level frame with width and height \"fill\" \
already covers the canvas and clips at every size. \
The server's checks are the verification: render once at the end, and its reply shows how wrapped, shrunk or \
cut text came out. To judge the look, render with preview (one small image), not by opening the files. \
An animation's reply states its length, frames and looping; opened, it shows only its first frame. \
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
    /// Folders `asset_add` may read local paths from.
    reads: Arc<crate::local::AllowedDirs>,
    /// Motion and video are in the tools (not `--no-motion`).
    motion: bool,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Server {
    /// A server over `store`, rendering final PNGs on `backend`, reading
    /// local asset paths only inside `reads`; without `motion`, time is
    /// left out of the tools.
    pub fn new(
        store: Store,
        backend: Backend,
        reads: crate::local::AllowedDirs,
        motion: bool,
    ) -> Self {
        let mut tool_router = Self::tool_router();
        schema::compact_all(&mut tool_router, motion, reads.dirs());
        Server {
            store: Arc::new(store),
            lock: Arc::new(Mutex::new(())),
            backend,
            reads: Arc::new(reads),
            motion,
            tool_router,
        }
    }

    #[tool(
        description = "Create a scene from sizes, or from a template by url or path with its tokens set. Returns its id."
    )]
    async fn scene_create(&self, Parameters(a): Parameters<SceneCreateArgs>) -> CallToolResult {
        reply(self.scene_create_impl(a).await)
    }

    #[tool(
        description = "Add an image (PNG, JPEG, SVG) to a scene's assets from url, path or base64. Returns id and size."
    )]
    async fn asset_add(&self, Parameters(a): Parameters<AssetAddArgs>) -> CallToolResult {
        reply(self.asset_add_impl(a).await)
    }

    #[tool(
        description = "Add layers, drawn bottom to top. CSS names and values. Any layer: {type, id?, role?, parent? \
(frame id), x, y (px or \"25%\"), width, height (px, \"hug\", \"fill\", \"40%\"), min/maxWidth, min/maxHeight, aspectRatio, \
place top-left…bottom-right + margin, constraints {horizontal: left|right|center|stretch|scale, vertical: \
top|bottom|center|stretch|scale}, hidden, opacity, rotate (deg), blendMode, style (name or list), media {size id or \
landscape|square|portrait|wide|tall: {fields}}, fill, stroke, shadow (each one or a list), blur, backdropBlur, \
borderRadius (px, [tl,tr,br,bl], \"full\"), mask, scale, translate [x,y], skew, flipX, flipY, edges {sides, depth, seed}}. \
frame: children, clipsContent (true). Stack: flexDirection row|column|row-reverse|column-reverse (a list is \
tried in order), gap, padding, alignItems stretch (default)|flex-start|center|flex-end|baseline, justifyContent \
flex-start|center|flex-end|space-between|space-around|space-evenly, flexWrap; children: alignSelf, flexGrow (alone, it fills), \
layoutPriority (number; lower gives way first), position absolute. Grid: gridTemplateColumns/Rows (CSS tracks, \
repeat(auto-fill, minmax(160px, 1fr))), gridTemplateAreas [\"hero side\", …], gap, padding; children: gridArea, or \
gridRow/gridColumn (\"1 / span 2\"). spacer: free space in a \
stack. firstFit: children; draws the first that fits. text: text (markup <b> <i> <u> <s> <sup> <sub> <br>, \
<span style=\"color:…;font-weight:…\">, style names as tags; \"$token\" alone takes its value), fontSize (the max when \
fitting), fontWeight, color, fontFamily (Inter or any Google Font), textAlign, textAlignVertical, lineHeight (× \
fontSize), letterSpacing (px), fill (paints the letters, wins over color), textTransform, fontStyle, textDecoration, maxLines, textWrap balance|pretty, trim \
\"cap\", highlight, padding, curve, leader (\".\" fills a tab gap), knockout. Text box: width+height → shrinks to fit \
(minimumScaleFactor 0.5) then ellipsis; width → wraps down; neither → one line. \
image: asset, fit cover|contain|fill|tile, focus [x,y], crop {x,y,width,height} 0–1, filter {brightness, contrast, \
saturate (1 = unchanged), grayscale, sepia, hueRotate, duotone [dark, light], tint, halftone px}. icon: name, set \
lucide|solid|regular|brands, color (24 px). rect, ellipse (arc {start, end, inner}), polygon (sides, innerRadius → \
star), path (d, or shape: ribbon, bubble, arrow, chevron, tag, arch, shield, heart, cloud, wave, burst, blob-1…6, \
brush-stroke). line: from x,y by width,height; drawn by stroke. Fill: a CSS color or linear-gradient(), or \
{color|gradient|image|pattern|noise, opacity, blendMode}; a list stacks, [] none. gradient {type linear|radial|conic, \
angle (default 180: top to bottom) or from/to, stops [colors] or [{offset, color}]}. Stroke: \"#000\" or {width (1), \
color, align inside|center|outside, dash, cap, join, markerStart|markerEnd arrow|triangle|circle|diamond, \
roughness}. Shadow: {x, y, blur, spread, color, inset}. Mask: a gradient, shape name, \
{path}, {layer: id} or {image}; mode luminance, invert. Shared: styles {name: {fields}}; tokens \
{name: value} used as \"$name\"; components {card: {…, text: \"{{name}}\"}} placed by {type: use, component: \"card\", each: [{name: \
\"Dana\"}, …]}."
    )]
    async fn layer_add(&self, Parameters(a): Parameters<LayerAddArgs>) -> CallToolResult {
        let styles = Value::Object(a.styles.clone());
        let components = Value::Object(a.components.clone());
        let fetched = match self
            .fetch_fonts(&[&a.layers[..], &[styles, components]].concat())
            .await
        {
            Ok(f) => f,
            Err(e) => return reply(Err(e)),
        };
        reply(
            self.edit(&a.scene_id, |s| {
                ops::add_layers(
                    s,
                    ops::Shared {
                        styles: a.styles,
                        tokens: a.tokens,
                        components: a.components,
                    },
                    a.layers,
                )
            })
            .await
            .map(|(ids, s)| fetched + &edited("added", &ids, &s, &self.store.assets_dir())),
        )
    }

    #[tool(
        description = "Change or delete layers atomically. Each op: {target: {id}|{role}|{style}|{component, role?}, \
set: {fields}} or {target, delete: true} or {target: {id}, detach: true} (a use becomes plain layers). A role targets \
every layer with it; a style or component target changes it everywhere it's used. tokens {name: value} changes \
tokens. null resets a field. {target: {scene: true}, set: {background, sizes, duration, fps, loop, audio {asset, volume, trimStart, fadeIn, \
fadeOut}}} changes the scene."
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
        // Changed tokens are named too, so a tokens-only edit says what it did.
        let token_names: Vec<String> = a.tokens.keys().map(|k| format!("${k}")).collect();
        reply(
            self.edit(&a.scene_id, |s| {
                ops::update_layers(
                    s,
                    ops::Shared {
                        styles: Default::default(),
                        tokens: a.tokens,
                        components: Default::default(),
                    },
                    &a.ops,
                )
            })
            .await
            .map(|(mut ids, s)| {
                ids.extend(token_names);
                fetched + &edited("changed", &ids, &s, &self.store.assets_dir())
            }),
        )
    }

    #[tool(
        description = "Problems per size, or ok. Defects: !truncated|!overflow (needs W×H) !clipped !hidden !overlaps !unsafe (under \
safeArea). Advisory: warn contrast. full: one line per layer per size: id type x,y w×h, font px, lines, image crop, upscale."
    )]
    async fn scene_describe(&self, Parameters(a): Parameters<SceneDescribeArgs>) -> CallToolResult {
        let scene = self.store.load(&a.scene_id).map_err(err);
        // Text is measured in its real font, even one another process fetched.
        let fonts = match &scene {
            Ok(s) => self.scene_fonts(s).await,
            Err(_) => Ok(String::new()),
        };
        reply(fonts.and(scene).and_then(|s| {
            describe(
                &s.resolved(),
                a.size.as_deref(),
                a.full,
                Some(&self.store.assets_dir()),
            )
        }))
    }

    #[tool(
        description = "Render each size (PNG, or format jpeg|webp|pdf). Returns each size's path and how wrapped, \
shrunk or cut text was drawn; maxKB lowers quality to fit and says so; rows renders a variant per row of tokens. preview \
adds one small image of all sizes."
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
