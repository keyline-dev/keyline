//! The work behind the tools that need more than a line: creating scenes,
//! adding assets, rendering, fetching fonts and locked edits.

use std::sync::Arc;

use base64::Engine;
use rmcp::model::ContentBlock;
use serde_json::Value;

use super::{AssetAddArgs, PREVIEW_HEIGHT, RenderArgs, SceneCreateArgs, Server, err};
use crate::describe::text_report;
use crate::fetch::{MAX_ASSET_BYTES, fetch};
use crate::fonts::{Outcome, ensure as ensure_font};
use crate::render::{contact_sheet, raster_size, render_png_on, svg_size};
use crate::scene::{Asset, Color, SCHEMA_VERSION, Scene, Size};

impl Server {
    pub(super) async fn scene_create_impl(&self, a: SceneCreateArgs) -> Result<String, String> {
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

    pub(super) async fn asset_add_impl(&self, a: AssetAddArgs) -> Result<String, String> {
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

    pub(super) async fn render_impl(&self, a: RenderArgs) -> Result<Vec<ContentBlock>, String> {
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
    pub(super) async fn fetch_fonts(&self, values: &[Value]) -> Result<String, String> {
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
    pub(super) async fn edit<T>(
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
