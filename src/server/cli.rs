//! `keyline-mcp render scene.json`: the same load and render an agent's
//! calls make, from the command line, for scripts and CI.

use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::Server;

/// What `render` should draw, from its flags.
#[derive(Debug, Default)]
pub struct RenderFile {
    /// The scene file, loaded like a template.
    pub scene: PathBuf,
    /// Where the files go (default: the current folder).
    pub out: Option<PathBuf>,
    /// Size ids; all when empty.
    pub sizes: Vec<String>,
    /// A JSON file of token rows: one render per row.
    pub rows: Option<PathBuf>,
    /// The file format (default `png`).
    pub format: Option<String>,
    /// A still at this moment, seconds.
    pub time: Option<f64>,
    /// Lossy quality, 1–100.
    pub quality: Option<u32>,
    /// A file size cap, KB.
    pub max_kb: Option<u32>,
    /// Also write the preview sheet, `<scene>-preview.png`.
    pub preview: bool,
    /// Print the checks only: lay out, draw and encode nothing.
    pub check: bool,
}

/// The report `render` prints, and whether the design has a `!` defect.
pub struct Report {
    /// The load's problems and the render's lines, as the tools reply.
    pub text: String,
    /// Any `!` defect: the command then exits non-zero.
    pub defects: bool,
}

impl Server {
    /// Loads `r.scene` and renders it into `r.out`.
    ///
    /// # Errors
    /// The scene, the rows or a flag doesn't parse, or a file can't be
    /// written: the one-line error the tool would give.
    pub async fn render_file(&self, r: &RenderFile) -> Result<Report, String> {
        let path = r.scene.display().to_string();
        let (id, created) = self
            .scene_create_impl(from(json!({ "path": path }))?)
            .await?;
        let report = self.render_created(r, &id, &created).await;
        // The scene and its files were only the way there: `out` has the copies.
        self.store.discard(&id);
        report
    }

    /// Checks and renders scene `id`, just made from `r.scene`.
    async fn render_created(
        &self,
        r: &RenderFile,
        id: &str,
        created: &str,
    ) -> Result<Report, String> {
        let rows: Value = match &r.rows {
            Some(p) => serde_json::from_slice(
                &std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?,
            )
            .map_err(|e| format!("{}: {e}", p.display()))?,
            None => json!([]),
        };
        let mut args = json!({ "sceneId": id, "rows": rows });
        if !r.sizes.is_empty() {
            args["sizes"] = json!(r.sizes);
        }
        if let Some(f) = &r.format {
            args["format"] = json!(f);
        }
        for (key, v) in [
            ("time", json!(r.time)),
            ("quality", json!(r.quality)),
            ("maxKB", json!(r.max_kb)),
        ] {
            if !v.is_null() {
                args[key] = v;
            }
        }
        if r.preview {
            args["preview"] = json!(true);
        }
        let scene = self.store.load(id).map_err(|e| e.to_string())?;
        let rows =
            serde_json::from_value::<Vec<_>>(rows_of(&args)).map_err(|e| format!("rows: {e}"))?;
        let mut text = self.checks(&scene, id, created, &r.sizes, &rows)?;
        if r.check {
            return Ok(Report {
                defects: text.split_whitespace().any(|w| w.starts_with('!')),
                text,
            });
        }
        let (blocks, files) = self.render_impl(from(args.clone())?).await?;
        let out = r.out.clone().unwrap_or_else(|| PathBuf::from("."));
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        // Each file copied into `out`, and the reply naming the copy.
        let mut reply = blocks
            .iter()
            .filter_map(|b| b.as_text())
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for src in &files {
            let dst = out.join(src.file_name().ok_or("a render has no file name")?);
            std::fs::copy(src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
            reply = reply.replace(&src.display().to_string(), &dst.display().to_string());
        }
        for line in reply.lines() {
            let _ = writeln!(text, "{line}");
        }
        if let Some(image) = blocks.iter().find_map(|b| b.as_image()) {
            use base64::Engine as _;
            let png = base64::engine::general_purpose::STANDARD
                .decode(&image.data)
                .map_err(|e| e.to_string())?;
            let stem = r
                .scene
                .file_stem()
                .map_or_else(|| "scene".into(), |s| s.to_string_lossy());
            let path = out.join(format!("{stem}-preview.png"));
            std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
            let _ = writeln!(text, "preview {}", path.display());
        }
        Ok(Report {
            defects: text.split_whitespace().any(|w| w.starts_with('!')),
            text,
        })
    }
}

impl Server {
    /// What a render of `sizes` (all when empty) would be checked for:
    /// the load's notes (fetched fonts, hints), then per row of tokens (or
    /// the scene alone) its problem lines and its facts, for those sizes
    /// only, a row's lines tagged `r<n>`.
    fn checks(
        &self,
        scene: &crate::scene::Scene,
        id: &str,
        created: &str,
        sizes: &[String],
        rows: &[serde_json::Map<String, Value>],
    ) -> Result<String, String> {
        for want in sizes {
            if !scene.sizes.iter().any(|s| s.id == *want) {
                return Err(format!("no size {want}"));
            }
        }
        let drawn: Vec<&crate::scene::Size> = scene
            .sizes
            .iter()
            .filter(|s| sizes.is_empty() || sizes.contains(&s.id))
            .collect();
        let mut text = String::new();
        // The load's own lines that aren't checks: fonts fetched, hints.
        for line in created.lines().filter(|l| {
            !l.starts_with(id)
                && (l.starts_with("hint: ")
                    || l.starts_with("fetched ")
                    || l.starts_with("video off"))
        }) {
            let _ = writeln!(text, "{line}");
        }
        let assets = self.store.assets_dir();
        for (label, variant) in super::handlers::variants(scene, rows)? {
            let tag = label.trim_end_matches('.');
            let tag = if tag.is_empty() {
                String::new()
            } else {
                format!("{tag} ")
            };
            let resolved = variant.resolved();
            if let Some(w) = crate::describe::warnings_for(&resolved, &drawn, Some(&assets)) {
                for line in w.lines() {
                    let _ = writeln!(text, "{tag}{line}");
                }
            }
            let facts = crate::describe::facts_for(&resolved, &drawn);
            if !facts.is_empty() {
                let _ = writeln!(text, "{tag}{facts}");
            }
        }
        Ok(text)
    }
}

/// The `rows` of a render call's args.
fn rows_of(args: &Value) -> Value {
    args.get("rows").cloned().unwrap_or_else(|| json!([]))
}

/// The tool's args from JSON, as an agent's call would give them.
fn from<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| e.to_string())
}
