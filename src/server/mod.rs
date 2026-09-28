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
    /// Folders `asset_add` may read local paths from.
    reads: Arc<crate::local::AllowedDirs>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Server {
    /// A server over `store`, rendering final PNGs on `backend`, reading
    /// local asset paths only inside `reads`.
    pub fn new(store: Store, backend: Backend, reads: crate::local::AllowedDirs) -> Self {
        let mut tool_router = Self::tool_router();
        schema::compact_all(&mut tool_router);
        Server {
            store: Arc::new(store),
            lock: Arc::new(Mutex::new(())),
            backend,
            reads: Arc::new(reads),
            tool_router,
        }
    }

    #[tool(description = "Create a scene. Returns its id.")]
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
        description = "Add layers, drawn bottom to top. Any layer: {type, id?, role?, parent? (frame id), x, y (px or \
\"25%\"), width, height (px, \"hug\", \"fill\", \"40%\"), min/maxWidth, min/maxHeight, aspectRatio, place \
top-left…bottom-right + inset, constraints {h: left|right|center|stretch|scale, v: top|bottom|center|stretch|scale}, \
hidden, opacity, rotation, blendMode, style (name or list), at {sizeId or landscape|square|portrait|wide|tall: \
{fields}}, fills, strokes, shadows, blur, backdropBlur, radius (px, [tl,tr,br,bl], \"full\"), mask, scale, offset \
[x,y], skew, flipX, flipY, edges {sides, depth, seed} (torn)}. frame: children, clip (true), stack {dir row|column|row-reverse|column-reverse or a list \
tried in order, gap, padding (1, 2 or 4 values), align start|center|end|stretch|baseline, justify \
start|center|end|between|around|evenly, wrap} or grid {columns \"1fr 200px auto\" | 3 | {min}, rows, gap, \
padding, areas [\"hero hero side\", …]} (children: area, or cell [r,c] + span [rows,cols], from 1); stack children flow in order and may set alignSelf, grow, priority (low \
gives way first), position absolute; unsized, it hugs them. spacer: free space in a stack. firstFit: children, draws \
the first that fits. text: text (markup <b> <i> <u> <s> <sup> <br>, <span color=… weight=…>, style names as tags), \
fontSize (the max when fitting), weight, color, fontFamily (Inter or any Google Font), align left|center|right|justify, \
lineHeight (× size), letterSpacing, textCase upper|lower|capitalize, italic, decoration underline|strike, maxLines, \
textWrap balance|pretty, verticalAlign, trim \"cap\", highlight (color or {color, padding, radius, style brush}), \
padding, curve (radius), leader (\".\" fills a tab gap, right part flush right), knockout, ranges [{start, end, \
…fields}]. Box: width+height → shrinks to fit (minFontScale 0.5) then ellipsis; width → wraps down; neither → one line; resize \
fixed|truncate keeps the size. image: asset, fit fill|fit|tile, focus [x,y], crop {x,y,width,height} 0–1, adjust \
{brightness, contrast, saturate, grayscale, sepia, hue, duotone [dark, light], tint, halftone (dot px)}. icon: name, set \
lucide|solid|regular|brands, color (24px tall unless sized). rect, ellipse (arc {start, end, inner}), polygon \
(sides, innerRadius → star), path (d, or shape: ribbon, bubble, arrow, chevron, tag, arch, shield, heart, cloud, wave, \
burst, blob-1…6, brush-stroke): color, gradient, stroke. line: from x,y by width,height; color, strokeWidth. Fill: a \
color or {color|gradient|image|pattern|noise, opacity, blendMode}; a list stacks, [] none. gradient {type \
linear|radial|conic, angle or from/to [x,y], stops [colors] or [{at, color}]}. pattern \
dots|stripes|grid|checker|zigzag|rays, color, size. Stroke: \"#000\" or {width or [t,r,b,l], color|gradient, align \
inside|center|outside, dash [on, off], cap, start|end arrow|triangle|circle|diamond, rough px + seed (hand-drawn)}. Shadow {x, y, blur, spread, \
color, inset} (follows text's or a cutout's alpha). Mask: a gradient, a shape name, {path}, {layer: id} or {image}; \
mode luminance, invert. Shared: styles {name: {fields}} (a later style wins; own fields win); tokens {name: value} \
used as \"$name\"; components {name: layer tree with {prop}} placed by {type: use, component, props, each: [props…]} \
(ids use.n.role). Colors #RGB[A], #RRGGBB[AA] or CSS names."
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
tokens. null resets a field."
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
            .map(|(ids, s)| fetched + &edited("changed", &ids, &s, &self.store.assets_dir())),
        )
    }

    #[tool(
        description = "Problems per size, or ok. Defects: !overflow !truncated !clipped !hidden !overlaps !unsafe (under a size's safe insets). \
Advisory: warn contrast. full: one line per layer per size: id type x,y w×h, font px, lines, image crop, upscale."
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
shrunk or cut text was drawn; maxKB lowers quality to fit and says so. preview adds one small image of all sizes."
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
