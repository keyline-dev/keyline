//! keyline-mcp server: speaks MCP over stdio.

use keyline_mcp::{fonts, gpu::Backend, local::AllowedDirs, server::Server, store::Store, text};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Local paths are read only inside folders named with --allow-read.
    let reads = AllowedDirs::from_args(std::env::args().skip(1))?;
    let store = Store::open_default()?;
    // Web fonts cached in the data dir, plus fonts dropped there or in a dir
    // named by $KEYLINE_MCP_FONTS, join the bundled Inter.
    let extra = std::env::var_os("KEYLINE_MCP_FONTS").map(std::path::PathBuf::from);
    let fonts_dir = store.root().join("fonts");
    let dirs: Vec<&std::path::Path> = std::iter::once(fonts_dir.as_path())
        .chain(extra.as_deref())
        .collect();
    fonts::load_cache(&fonts_dir)?;
    text::load_fonts(&dirs)?;
    // GPU by default; KEYLINE_MCP_RENDERER=cpu forces the CPU.
    Server::new(store, Backend::from_env()?, reads)
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
