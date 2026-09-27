//! The MCP surface: six batched, verb-shaped tools with terse descriptions
//! and plain-text results, to keep the agent's token spend low.

use std::sync::Arc;

use base64::Engine;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::describe::{describe, facts, text_report, warnings};
use crate::fetch::{MAX_ASSET_BYTES, fetch};
use crate::fonts::{Outcome, ensure as ensure_font};
use crate::gpu::Backend;
use crate::ops::{self, Op};
use crate::render::{contact_sheet, raster_size, render_png_on, svg_size};
use crate::scene::{Asset, Color, SCHEMA_VERSION, Scene, Size};
use crate::store::Store;

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

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `scene_create`.
pub struct SceneCreateArgs {
    /// Master size, px.
    width: f32,
    height: f32,
    /// Target sizes. `scale` (default 1) shrinks everything, fonts included, before constraints apply.
    sizes: Vec<Size>,
    /// Hex color, default #FFFFFF.
    background: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `asset_add`.
pub struct AssetAddArgs {
    scene_id: String,
    /// Public http(s) URL of a PNG, JPEG or SVG.
    url: Option<String>,
    /// Or the file bytes, base64.
    base64: Option<String>,
    /// Asset id to use in layers; generated if omitted.
    id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `layer_add`.
pub struct LayerAddArgs {
    scene_id: String,
    /// Named text styles to add or replace, used by text layers' `style`.
    #[serde(default)]
    styles: serde_json::Map<String, Value>,
    layers: Vec<Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `layer_update`.
pub struct LayerUpdateArgs {
    scene_id: String,
    ops: Vec<Op>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `scene_describe`.
pub struct SceneDescribeArgs {
    scene_id: String,
    /// One size id; all sizes if omitted.
    size: Option<String>,
    /// Every layer's box, not just warnings.
    #[serde(default)]
    full: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Arguments of `render`.
pub struct RenderArgs {
    scene_id: String,
    /// Size ids; all if omitted.
    sizes: Option<Vec<String>>,
    /// Also return one small image of all sizes side by side.
    #[serde(default)]
    preview: bool,
}

#[tool_router]
impl Server {
    /// A server over `store`, rendering final PNGs on `backend`.
    pub fn new(store: Store, backend: Backend) -> Self {
        Server {
            store: Arc::new(store),
            lock: Arc::new(Mutex::new(())),
            backend,
            tool_router: Self::tool_router(),
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
        description = "Add layers, drawn bottom to top. Layer: {type, id?, role?, parent? (frame id), x, y, width, \
height (px, relative to parent), constraints {h: left|right|center|stretch|scale, v: top|bottom|center|stretch|scale} \
(default left, top), opacity, rotation (degrees), blendMode (multiply, screen, …), mask (gradient; its alpha fades the layer), \
at {sizeId: {fields}}: changes for one size only (bigger text, a column stack, a shorter photo)}. \
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
frame: children[], clip (true), color, cornerRadius, stack {dir row|column, gap, padding, align start|center|end, \
justify start|center|end|between|evenly} places children in order ignoring their x, y; unsized, it hugs them. \
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
        description = "Problems per size, or ok. Defects: !overflow !truncated !clipped !hidden !overlaps. \
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

impl Server {
    async fn scene_create_impl(&self, a: SceneCreateArgs) -> Result<String, String> {
        let background = match a.background {
            Some(b) => Color::parse(&b).ok_or_else(|| format!("bad color {b}"))?,
            None => Color(0xFFFF_FFFF),
        };
        let scene = Scene {
            schema_version: SCHEMA_VERSION,
            width: a.width,
            height: a.height,
            background,
            sizes: a.sizes,
            assets: Default::default(),
            styles: Default::default(),
            layers: Vec::new(),
            version: 0,
        };
        scene.validate()?;
        let id = self.store.new_scene_id();
        self.store.save(&id, &scene).map_err(err)?;
        Ok(format!("{id} v0"))
    }

    async fn asset_add_impl(&self, a: AssetAddArgs) -> Result<String, String> {
        let bytes = match (&a.url, &a.base64) {
            (Some(url), None) => fetch(url).await.map_err(err)?,
            (None, Some(b64)) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64.trim())
                    .map_err(|e| format!("bad base64: {e}"))?;
                if bytes.len() > MAX_ASSET_BYTES {
                    return Err(format!("asset larger than {} MB", MAX_ASSET_BYTES >> 20));
                }
                bytes
            }
            _ => return Err("give exactly one of url or base64".into()),
        };
        let (width, height, svg) = match raster_size(&bytes) {
            Some((w, h)) => (w, h, false),
            None => {
                let (w, h) = svg_size(&bytes).map_err(|_| "not a PNG, JPEG or SVG".to_string())?;
                (w, h, true)
            }
        };
        let sha256 = self.store.put_asset(&bytes).map_err(err)?;
        let id = a.id.unwrap_or_else(|| format!("a{}", &sha256[..6]));
        let asset = Asset {
            sha256,
            width,
            height,
            svg,
        };
        let (_, scene) = self
            .edit(&a.scene_id, |s| {
                s.assets.insert(id.clone(), asset);
                s.version += 1;
                Ok(())
            })
            .await?;
        Ok(format!("{id} {width}×{height} v{}", scene.version))
    }

    async fn render_impl(&self, a: RenderArgs) -> Result<Vec<ContentBlock>, String> {
        let scene = self.store.load(&a.scene_id).map_err(err)?;
        let scene = Arc::new(scene.resolved().into_owned());
        let sizes: Vec<Size> = match &a.sizes {
            None => scene.sizes.clone(),
            Some(ids) => ids
                .iter()
                .map(|id| {
                    scene
                        .sizes
                        .iter()
                        .find(|s| s.id == *id)
                        .cloned()
                        .ok_or_else(|| format!("no size {id}"))
                })
                .collect::<Result<_, _>>()?,
        };

        // Each size renders on the blocking pool, concurrently.
        let jobs: Vec<_> = sizes
            .iter()
            .cloned()
            .map(|size| {
                let scene = Arc::clone(&scene);
                let assets = self.store.assets_dir();
                let backend = self.backend;
                tokio::task::spawn_blocking(move || {
                    let png = render_png_on(&scene, &size, &assets, backend)?;
                    anyhow::Ok((text_report(&scene, &size), size.id, png))
                })
            })
            .collect();

        let mut text = String::new();
        for job in jobs {
            let (report, size_id, png) = job.await.map_err(|e| e.to_string())?.map_err(err)?;
            let path = self
                .store
                .render_path(&a.scene_id, scene.version, &size_id)
                .map_err(err)?;
            std::fs::write(&path, png).map_err(|e| e.to_string())?;
            text.push_str(&format!("{size_id} {}\n{report}", path.display()));
        }
        let mut content = vec![ContentBlock::text(text.trim_end())];
        if a.preview {
            let assets = self.store.assets_dir();
            let sheet = tokio::task::spawn_blocking(move || {
                contact_sheet(&scene, &sizes, PREVIEW_HEIGHT, &assets)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(err)?;
            content.push(ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(sheet),
                "image/png",
            ));
        }
        Ok(content)
    }

    /// Makes every `fontFamily` in `values` available, downloading missing
    /// ones (CSS-style web fonts). Returns a note per download, for the reply.
    async fn fetch_fonts(&self, values: &[Value]) -> Result<String, String> {
        fn collect<'v>(v: &'v Value, out: &mut Vec<&'v str>) {
            match v {
                Value::Object(o) => {
                    if let Some(Value::String(f)) = o.get("fontFamily") {
                        out.push(f);
                    }
                    o.values().for_each(|c| collect(c, out));
                }
                Value::Array(a) => a.iter().for_each(|c| collect(c, out)),
                _ => {}
            }
        }
        let mut wanted = Vec::new();
        values.iter().for_each(|v| collect(v, &mut wanted));
        wanted.sort_unstable();
        wanted.dedup();
        let dir = self.store.root().join("fonts");
        let mut notes = String::new();
        for family in wanted {
            if let Outcome::Fetched(n) = ensure_font(family, &dir).await.map_err(err)? {
                notes.push_str(&format!("fetched font {family} ({n} files)\n"));
            }
        }
        Ok(notes)
    }

    /// Load, mutate and save a scene under the lock. Returns the saved scene.
    async fn edit<T>(
        &self,
        scene_id: &str,
        f: impl FnOnce(&mut Scene) -> Result<T, String>,
    ) -> Result<(T, Scene), String> {
        let _guard = self.lock.lock().await;
        let mut scene = self.store.load(scene_id).map_err(err)?;
        let out = f(&mut scene)?;
        self.store.save(scene_id, &scene).map_err(err)?;
        Ok((out, scene))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            format!(
                "{INSTRUCTIONS} Fonts: any Google Fonts family (downloaded once, then cached); installed now: {}.",
                crate::text::families().join(", ")
            ),
        )
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
