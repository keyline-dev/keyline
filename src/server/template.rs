//! Templates: a scene file, loaded by url or path the way an asset is. Its
//! images are named by url, or by a path relative to the file, and are added
//! as assets; `tokens` given with it fill its variables. Nothing is kept
//! but the new scene: the file stays wherever its owner keeps it.

use std::path::PathBuf;

use reqwest::Url;
use serde_json::{Map, Value};

use super::handlers::MAX_VIDEO_BYTES;
use super::{SceneCreateArgs, Server, err};
use crate::fetch::{MAX_ASSET_BYTES, fetch};
use crate::scene::{Asset, Color, SCHEMA_VERSION, Scene, SizeSpec};

/// What a template's relative names resolve against.
enum Base {
    /// A template from a URL: its images are URLs too, never local files.
    Url(Url),
    /// A template from a local file: its folder.
    Dir(PathBuf),
}

/// The fields a template file may have: a scene's.
const FIELDS: &[&str] = &[
    "schemaVersion",
    "version",
    "width",
    "height",
    "background",
    "sizes",
    "assets",
    "styles",
    "tokens",
    "components",
    "duration",
    "fps",
    "loop",
    "layers",
];

impl Server {
    /// The scene the template at `a.url` or `a.path` describes, with `a`'s
    /// own fields (sizes, background, tokens …) over the template's.
    pub(super) async fn template(&self, a: SceneCreateArgs) -> Result<Scene, String> {
        let (bytes, base) = match (&a.url, &a.path) {
            (Some(url), None) => {
                let base = Url::parse(url).map_err(|e| format!("bad url {url}: {e}"))?;
                (fetch(url).await.map_err(err)?, Base::Url(base))
            }
            (None, Some(path)) => {
                let bytes = self.reads.read(path, MAX_ASSET_BYTES)?;
                let dir = std::fs::canonicalize(path)
                    .ok()
                    .and_then(|f| f.parent().map(PathBuf::from))
                    .unwrap_or_default();
                (bytes, Base::Dir(dir))
            }
            _ => return Err("give a template by url or path, not both".into()),
        };
        let Ok(Value::Object(mut t)) = serde_json::from_slice(&bytes) else {
            return Err("the template isn't a scene (a JSON object)".into());
        };
        if let Some(k) = t.keys().find(|k| !FIELDS.contains(&k.as_str())) {
            return Err(format!("template: unknown field {k}"));
        }
        let mut take = |k: &str| t.remove(k).unwrap_or(Value::Null);
        let map = |v: Value, what: &str| match v {
            Value::Null => Ok(Map::new()),
            Value::Object(o) => Ok(o),
            _ => Err(format!("template: {what} must be an object")),
        };

        let sizes = if a.sizes.is_empty() {
            serde_json::from_value::<Option<Vec<SizeSpec>>>(take("sizes"))
                .map_err(|e| format!("template sizes: {e}"))?
                .unwrap_or_default()
        } else {
            a.sizes
        }
        .into_iter()
        .map(SizeSpec::resolve)
        .collect::<Result<Vec<_>, _>>()?;
        let first = sizes.first().map_or((0.0, 0.0), |s| (s.width, s.height));
        let number = |v: Value| v.as_f64().map(|n| n as f32);
        let background = match a
            .background
            .or_else(|| take("background").as_str().map(String::from))
        {
            Some(b) => Color::parse(&b).ok_or_else(|| format!("bad color {b}"))?,
            None => Color(0xFFFF_FFFF),
        };

        let mut assets = std::collections::BTreeMap::new();
        for (id, v) in map(take("assets"), "assets")? {
            let asset = match v {
                Value::String(name) => {
                    let bytes = self
                        .asset_bytes(&name, &base)
                        .await
                        .map_err(|e| format!("template asset {id}: {e}"))?;
                    self.ingest(&bytes, false)
                        .map_err(|e| format!("template asset {id}: {e}"))?
                }
                // A scene saved by keyline names its assets by content hash.
                v => serde_json::from_value::<Asset>(v)
                    .map_err(|e| format!("template asset {id}: {e}"))?,
            };
            assets.insert(id, asset);
        }

        let mut tokens = map(take("tokens"), "tokens")?;
        for (k, v) in a.tokens {
            if !tokens.contains_key(&k) {
                let names: Vec<_> = tokens.keys().map(String::as_str).collect();
                return Err(format!(
                    "no token {k}; the template has: {}",
                    names.join(", ")
                ));
            }
            tokens.insert(k, v);
        }
        let shared = crate::ops::Shared {
            styles: map(take("styles"), "styles")?,
            tokens,
            components: map(take("components"), "components")?,
        };
        let layers = match take("layers") {
            Value::Null => Vec::new(),
            Value::Array(l) => l,
            _ => return Err("template: layers must be a list".into()),
        };
        let mut scene = Scene {
            schema_version: SCHEMA_VERSION,
            width: a.width.or(number(take("width"))).unwrap_or(first.0),
            height: a.height.or(number(take("height"))).unwrap_or(first.1),
            background,
            sizes,
            assets,
            styles: Default::default(),
            tokens: Default::default(),
            components: Default::default(),
            duration: a.duration.or(number(take("duration"))),
            fps: a.fps.or(number(take("fps"))).unwrap_or(30.0),
            looping: a.looping || take("loop").as_bool().unwrap_or(false),
            layers: Vec::new(),
            version: 0,
        };
        // The layers go in as layer_add adds them: normalized, ids given,
        // tokens bound, and the whole scene validated.
        crate::ops::add_layers(&mut scene, shared, layers)?;
        scene.version = 0;
        Ok(scene)
    }

    /// The bytes of a template's image `name`: a URL, or a path relative to
    /// the template. A template from the web never reads local files.
    async fn asset_bytes(&self, name: &str, base: &Base) -> Result<Vec<u8>, String> {
        let web = name.starts_with("http://") || name.starts_with("https://");
        match base {
            Base::Url(url) => {
                let url = url.join(name).map_err(|e| format!("bad url {name}: {e}"))?;
                fetch(url.as_str()).await.map_err(err)
            }
            Base::Dir(_) if web => fetch(name).await.map_err(err),
            Base::Dir(dir) => self
                .reads
                .read(&dir.join(name).to_string_lossy(), MAX_VIDEO_BYTES),
        }
    }
}
