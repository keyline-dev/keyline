//! keyline-mcp server: speaks MCP over stdio.

use keyline_mcp::options::{HELP, Options};
use keyline_mcp::{fonts, local::AllowedDirs, server::Server, store::Store, text, video};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let o = Options::parse(std::env::args().skip(1))?;
    if o.help {
        println!("{HELP}");
        return Ok(());
    }
    // Local paths are read only inside folders named with --allow-read.
    let reads = AllowedDirs::new(&o.allow_read)?;
    let store = Store::open_default(o.data)?;
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
    Server::new(store, o.renderer, reads, !o.no_motion)
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
