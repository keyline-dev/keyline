//! The work behind the tools that need more than a line: creating scenes,
//! adding assets, rendering, fetching fonts and locked edits.

use std::fmt::Write as _;
use std::sync::Arc;

use base64::Engine;
use rmcp::model::ContentBlock;
use serde_json::Value;

use super::{AssetAddArgs, PREVIEW_HEIGHT, RenderArgs, SceneCreateArgs, Server, err};
use crate::describe::text_report;
use crate::fetch::{MAX_ASSET_BYTES, fetch};

/// The largest local video file `asset_add` takes.
const MAX_VIDEO_BYTES: usize = 500 * 1024 * 1024;
use crate::fonts::{Outcome, ensure as ensure_font};
use crate::render::{
    Format, contact_sheet, encode, raster_size, render_apng, render_gif, render_image, render_pdf,
    render_png_on, svg_size,
};
use crate::scene::{Asset, Color, SCHEMA_VERSION, Scene, Size, SizeSpec};

impl Server {
    pub(super) async fn scene_create_impl(&self, a: SceneCreateArgs) -> Result<String, String> {
        let background = match a.background {
            Some(b) => Color::parse(&b).ok_or_else(|| format!("bad color {b}"))?,
            None => Color(0xFFFF_FFFF),
        };
        let sizes = a
            .sizes
            .into_iter()
            .map(SizeSpec::resolve)
            .collect::<Result<Vec<_>, _>>()?;
        let first = sizes.first().map_or((0.0, 0.0), |s| (s.width, s.height));
        let scene = Scene {
            schema_version: SCHEMA_VERSION,
            width: a.width.unwrap_or(first.0),
            height: a.height.unwrap_or(first.1),
            background,
            sizes,
            assets: Default::default(),
            styles: Default::default(),
            tokens: Default::default(),
            components: Default::default(),
            duration: a.duration,
            fps: a.fps.unwrap_or(30.0),
            looping: a.looping,
            layers: Vec::new(),
            version: 0,
        };
        scene.validate()?;
        let id = self.store.new_scene_id();
        self.store.save(&id, &scene).map_err(err)?;
        Ok(format!("{id} v0"))
    }

    pub(super) async fn asset_add_impl(&self, a: AssetAddArgs) -> Result<String, String> {
        let bytes = match (&a.url, &a.path, &a.base64) {
            (Some(url), None, None) => fetch(url).await.map_err(err)?,
            // Local files may be video, so their limit is video's.
            (None, Some(path), None) => self.reads.read(path, MAX_VIDEO_BYTES)?,
            (None, None, Some(b64)) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64.trim())
                    .map_err(|e| format!("bad base64: {e}"))?;
                if bytes.len() > MAX_ASSET_BYTES {
                    return Err(format!("asset larger than {} MB", MAX_ASSET_BYTES >> 20));
                }
                bytes
            }
            _ => return Err("give exactly one of url, path or base64".into()),
        };
        let still = raster_size(&bytes)
            .map(|(w, h)| (w, h, false))
            .or_else(|| svg_size(&bytes).ok().map(|(w, h)| (w, h, true)));
        if still.is_some() && bytes.len() > MAX_ASSET_BYTES {
            return Err(format!("image larger than {} MB", MAX_ASSET_BYTES >> 20));
        }
        if still.is_none() && a.base64.is_some() {
            return Err("not a PNG, JPEG or SVG; a video comes by path or url, not base64".into());
        }
        let sha256 = self.store.put_asset(&bytes).map_err(err)?;
        let (width, height, svg, clip) = match still {
            Some((w, h, svg)) => (w, h, svg, None),
            None => {
                let file = self.store.assets_dir().join(&sha256);
                let (w, h, clip) =
                    crate::video::probe::probe(&file)?.ok_or("not a PNG, JPEG, SVG or video")?;
                (w, h, false, Some(clip))
            }
        };
        let id = a.id.unwrap_or_else(|| format!("a{}", &sha256[..6]));
        let asset = Asset {
            sha256,
            width,
            height,
            svg,
            clip,
        };
        let (_, scene) = self
            .edit(&a.scene_id, |s| {
                s.assets.insert(id.clone(), asset);
                s.version += 1;
                Ok(())
            })
            .await?;
        let length = clip.map_or_else(String::new, |c| {
            format!(
                " {}s {}fps",
                (c.duration * 10.0).round() / 10.0,
                c.fps.round()
            )
        });
        Ok(format!("{id} {width}×{height}{length} v{}", scene.version))
    }

    pub(super) async fn render_impl(&self, a: RenderArgs) -> Result<Vec<ContentBlock>, String> {
        let scene = self.store.load(&a.scene_id).map_err(err)?;
        let fetched = self.scene_fonts(&scene).await?;
        let scene = scene.resolved().into_owned();
        if a.time.is_some()
            && matches!(
                a.format,
                Format::Apng | Format::Gif | Format::Mp4 | Format::Webm
            )
        {
            return Err("time renders a still; leave it out for moving formats".into());
        }
        let scene = Arc::new(scene);
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
                let (format, quality, max_kb) = (a.format, a.quality.unwrap_or(90), a.max_kb);
                let time = a.time;
                let sound = a.audio.unwrap_or(true);
                tokio::task::spawn_blocking(move || {
                    // A still of a moment: the scene as it is then, at this
                    // size (slides travel this size's width).
                    let scene = match time {
                        Some(t) => Arc::new(crate::anim::at_time(&scene, t, &size)),
                        None => scene,
                    };
                    let mut note = String::new();
                    let still = matches!(format, Format::Png | Format::Jpeg | Format::Webp);
                    let video = crate::video::frame::has_video(&scene);
                    if video && format == Format::Pdf {
                        return Err(anyhow::anyhow!(
                            "pdf can't hold video; render png, jpeg or webp"
                        ));
                    }
                    let bytes = match format {
                        // A still of a scene with video shows the clips' frames at that moment.
                        _ if still && video => {
                            let (at, frames) = crate::video::frame::still(
                                &scene,
                                &size,
                                time.unwrap_or(0.0),
                                &assets,
                            )?;
                            let image = crate::render::render_image_with(
                                &at, &size, 1.0, &assets, false, frames,
                            )?;
                            encode(&image, format, quality, max_kb)?.bytes
                        }
                        Format::Png if max_kb.is_none() => {
                            render_png_on(&scene, &size, &assets, backend)?
                        }
                        Format::Pdf => render_pdf(&scene, &size, &assets)?,
                        Format::Mp4 | Format::Webm => {
                            let container = if format == Format::Mp4 {
                                crate::video::encode::Container::Mp4
                            } else {
                                crate::video::encode::Container::Webm
                            };
                            crate::video::encode::render_video(
                                &scene, &size, scene.fps, &assets, container, sound,
                            )?
                        }
                        Format::Apng | Format::Gif => {
                            // maxKB: halve the frame rate until it fits.
                            let mut fps = scene.fps;
                            loop {
                                let bytes = if format == Format::Gif {
                                    render_gif(&scene, &size, fps, &assets)?
                                } else {
                                    render_apng(&scene, &size, fps, &assets)?
                                };
                                let over =
                                    max_kb.is_some_and(|kb| bytes.len() > kb as usize * 1024);
                                if !over {
                                    if fps < scene.fps {
                                        note = format!(" fps {fps}");
                                    }
                                    break bytes;
                                }
                                if fps / 2.0 < 5.0 {
                                    let kb = bytes.len().div_ceil(1024);
                                    note = format!(" !too-big {kb} KB at {fps} fps");
                                    break bytes;
                                }
                                fps /= 2.0;
                            }
                        }
                        // ponytail: lossy formats render on the CPU; add a GPU readback if they get hot.
                        _ => {
                            let image = render_image(&scene, &size, 1.0, &assets, false)?;
                            let e = encode(&image, format, quality, max_kb)?;
                            if let Some(q) = e.lowered {
                                note = format!(" quality {q}");
                            }
                            if e.too_big {
                                let kb = e.bytes.len().div_ceil(1024);
                                let _ = write!(note, " !too-big {kb} KB");
                            }
                            e.bytes
                        }
                    };
                    anyhow::Ok((text_report(&scene, &size), size.id, bytes, note))
                })
            })
            .collect();

        let mut text = String::new();
        for job in jobs {
            let (report, size_id, bytes, note) =
                job.await.map_err(|e| e.to_string())?.map_err(err)?;
            let path = self
                .store
                .render_path(
                    &a.scene_id,
                    scene.version,
                    &size_id,
                    // A still at a moment gets its own file: `main-v3.at2.5s.png`.
                    &a.time.map_or_else(
                        || a.format.ext().to_owned(),
                        |t| format!("at{t}s.{}", a.format.ext()),
                    ),
                )
                .map_err(err)?;
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            text.push_str(&format!("{size_id} {}{note}\n{report}", path.display()));
        }
        let mut content = vec![ContentBlock::text(format!("{fetched}{}", text.trim_end()))];
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

    /// Makes every family `scene` uses available before it's measured or
    /// drawn: a render must never quietly fall back to Inter.
    pub(super) async fn scene_fonts(&self, scene: &Scene) -> Result<String, String> {
        let v = serde_json::to_value(scene).map_err(|e| e.to_string())?;
        self.fetch_fonts(&[v]).await
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
