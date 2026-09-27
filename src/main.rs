//! keyline-mcp server: speaks MCP over stdio.

use keyline_mcp::{fonts, gpu::Backend, server::Server, store::Store, text};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    Server::new(store, Backend::from_env()?)
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
