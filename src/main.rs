//! keyline-mcp: speaks MCP over stdio, or renders a scene file
//! (`keyline-mcp render scene.json`).

use anyhow::Context as _;
use keyline_mcp::options::{HELP, Options};
use keyline_mcp::server::{RenderFile, Server};
use keyline_mcp::{fonts, local::AllowedDirs, store::Store, text, video};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut o = Options::parse(std::env::args().skip(1))?;
    if o.help {
        println!("{HELP}");
        return Ok(());
    }
    let render = match o.operands.as_slice() {
        [] => None,
        [cmd, scene] if cmd == "render" => Some(scene.clone()),
        _ => anyhow::bail!(
            "unexpected {}; see keyline-mcp --help",
            o.operands.join(" ")
        ),
    };
    let render_only = o.out.is_some()
        || !o.sizes.is_empty()
        || o.rows.is_some()
        || o.format.is_some()
        || o.time.is_some()
        || o.quality.is_some()
        || o.max_kb.is_some()
        || o.preview
        || o.check;
    if render.is_none() && render_only {
        anyhow::bail!(
            "--out, --size, --rows, --format, --time, --quality, --max-kb, --preview and --check go with render; see keyline-mcp --help"
        );
    }
    // A scene's images are beside it: rendering one may read its folder.
    // The server's designs go in a workspace: --folder, else ~/keyline.
    let workspace = if let Some(scene) = &render {
        let dir = std::path::Path::new(scene)
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        o.folders.push(dir.to_path_buf());
        false
    } else {
        if o.folders.is_empty() {
            let home = std::env::home_dir()
                .context("no home folder; pass --folder <folder>")?
                .join("keyline");
            std::fs::create_dir_all(&home)
                .with_context(|| format!("creating {}", home.display()))?;
            o.folders.push(home);
        }
        true
    };
    // Local paths are read only inside the workspace folders.
    let reads = AllowedDirs::new(&o.folders)?;
    let mut store = Store::open_default(o.data)?;
    if workspace {
        store = store.with_workspace(reads.dirs().to_vec());
    }
    // Web fonts cached in the data dir, plus fonts dropped there or in the
    // --fonts folders, join the bundled Inter.
    let fonts_dir = store.root().join("fonts");
    let dirs: Vec<&std::path::Path> = std::iter::once(fonts_dir.as_path())
        .chain(o.fonts.iter().map(std::path::PathBuf::as_path))
        .collect();
    fonts::load_cache(&fonts_dir)?;
    text::load_fonts(&dirs)?;
    video::configure(video::Settings {
        ffmpeg: o.ffmpeg,
        encoder: o.encoder,
    });
    let server = Server::new(store, o.renderer, reads, !o.no_motion);
    if let Some(scene) = render {
        let report = server
            .render_file(&RenderFile {
                scene: scene.into(),
                out: o.out,
                sizes: o.sizes,
                rows: o.rows,
                format: o.format,
                time: o.time,
                quality: o.quality,
                max_kb: o.max_kb,
                preview: o.preview,
                check: o.check,
            })
            .await
            .map_err(anyhow::Error::msg)?;
        print!("{}", report.text);
        // A script or CI job fails on a broken design.
        if report.defects {
            std::process::exit(1);
        }
        return Ok(());
    }
    server
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
