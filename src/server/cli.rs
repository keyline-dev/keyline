//! `keyline-mcp render scene.json`: the same load and render an agent's
//! calls make, from the command line, for scripts and CI.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

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
        let created = self
            .scene_create_impl(from(json!({ "path": path }))?)
            .await?;
        // The reply names the new scene just before its version, `v0`.
        let words: Vec<&str> = created.split_whitespace().collect();
        let id = words
            .windows(2)
            .find(|w| w[1] == "v0")
            .map(|w| w[0].to_owned())
            .ok_or("the scene didn't load")?;
        let rows: Value = match &r.rows {
            Some(p) => serde_json::from_slice(
                &std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?,
            )
            .map_err(|e| format!("{}: {e}", p.display()))?,
            None => json!([]),
        };
        let mut args = json!({ "sceneId": &id, "rows": rows });
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
        let blocks = self.render_impl(from(args.clone())?).await?;
        let out = r.out.clone().unwrap_or_else(|| PathBuf::from("."));
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        let renders = self.store.root().join("renders");
        let mut text = String::new();
        // Problem lines start with their size: only the sizes drawn count.
        let drawn = |line: &str| {
            r.sizes.is_empty()
                || line
                    .split_whitespace()
                    .next()
                    .is_some_and(|s| r.sizes.iter().any(|w| w == s))
        };
        if r.rows.is_none() {
            for line in created.lines().skip(1).filter(|l| drawn(l)) {
                let _ = writeln!(text, "{line}");
            }
        } else {
            // Each row is its own design: checked as it will be drawn.
            let scene = self.store.load(&id).map_err(|e| e.to_string())?;
            let rows = serde_json::from_value::<Vec<_>>(rows_of(&args))
                .map_err(|e| format!("rows: {e}"))?;
            for (label, variant) in super::handlers::variants(&scene, &rows)? {
                let assets = self.store.assets_dir();
                let Some(w) = crate::describe::warnings(&variant, Some(&assets)) else {
                    continue;
                };
                for line in w.lines().filter(|l| drawn(l)) {
                    let _ = writeln!(text, "{} {line}", label.trim_end_matches('.'));
                }
            }
        }
        for t in blocks.iter().filter_map(|b| b.as_text()) {
            for line in t.text.lines() {
                let _ = writeln!(text, "{}", copy_out(line, &renders, &out)?);
            }
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

/// The `rows` of a render call's args.
fn rows_of(args: &Value) -> Value {
    args.get("rows").cloned().unwrap_or_else(|| json!([]))
}

/// The tool's args from JSON, as an agent's call would give them.
fn from<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| e.to_string())
}

/// A reply line naming a rendered file (`<size> <path> (<facts>)`, the
/// path under `renders`): the file copied into `out`, and the line with
/// its new path. Other lines as they are.
fn copy_out(line: &str, renders: &Path, out: &Path) -> Result<String, String> {
    let root = renders.display().to_string();
    let Some(at) = line.find(&root) else {
        return Ok(line.to_owned());
    };
    let end = line.rfind(" (").filter(|&e| e > at).unwrap_or(line.len());
    let src = Path::new(&line[at..end]);
    let (Some(name), true) = (src.file_name(), src.is_file()) else {
        return Ok(line.to_owned());
    };
    let dst = out.join(name);
    std::fs::copy(src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    Ok(format!("{}{}{}", &line[..at], dst.display(), &line[end..]))
}
