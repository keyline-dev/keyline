//! keyline-mcp server: speaks MCP over stdio.

use keyline_mcp::{fonts, gpu::Backend, local::AllowedDirs, server::Server, store::Store, text};
use rmcp::ServiceExt;

/// What `--help` prints: every option and environment variable.
const HELP: &str = "keyline-mcp: an AI-native design engine, an MCP server over stdio.

Usage: keyline-mcp [--allow-read <folder>]... [--no-motion]

Options:
  --allow-read <folder>   Let asset_add read local files by path inside this folder
                          (repeatable). Without it, paths are refused. Paths are
                          resolved through symlinks before the check.
  --no-motion             Leave animation and video out of the tools: stills
                          only, and fewer tokens of tool definitions per turn
  -h, --help              Print this help

Environment:
  KEYLINE_MCP_DATA        Scenes, assets, renders and the web-font cache (~/.keyline-mcp)
  KEYLINE_MCP_FONTS       Extra folder of .ttf and .otf fonts
  KEYLINE_MCP_RENDERER    gpu (default: GPU with CPU fallback) or cpu
  KEYLINE_MCP_FFMPEG      The ffmpeg program for video (default: ffmpeg on the PATH)
  KEYLINE_MCP_ENCODER     H.264 encoder: auto (default: a working GPU encoder, else
                          libx264), software, or an ffmpeg encoder name

Docs: https://github.com/yuvalt/keyline";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return Ok(());
    }
    let before = args.len();
    args.retain(|a| a != "--no-motion");
    let motion = args.len() == before;
    // Local paths are read only inside folders named with --allow-read.
    let reads = AllowedDirs::from_args(args)?;
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
    Server::new(store, Backend::from_env()?, reads, motion)
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
