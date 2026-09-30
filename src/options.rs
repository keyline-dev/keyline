//! The server's command line. Every setting is a flag, so an MCP client's
//! config shows all of it in `args`.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::gpu::Backend;

/// What `--help` prints: every flag.
pub const HELP: &str = "keyline-mcp: a design engine for AI agents: images and video at every size,
no Chrome needed. An MCP server over stdio.

Usage: keyline-mcp [options]                  the MCP server, over stdio
       keyline-mcp render <scene.json> [options]
                                               render a scene file without an agent

Options:
  --allow-read <folder>...
                          Let asset_add read local files by path inside these
                          folders (every folder up to the next flag; repeatable).
                          Without it, paths are refused. Paths are resolved
                          through symlinks before the check.
  --no-motion[=true|false]
                          Leave animation and video out of the tools: stills
                          only, and fewer tokens of tool definitions per turn
  --data <folder>         Scenes, assets, renders and the web-font cache
                          (default: ~/.keyline-mcp)
  --fonts <folder>        An extra folder of .ttf and .otf fonts (repeatable)
  --renderer gpu|cpu      gpu (default: the GPU, falling back to the CPU) or cpu
  --ffmpeg <path>         The ffmpeg program for video (default: ffmpeg on the PATH)
  --encoder <name>        H.264 encoder: auto (default: a working GPU encoder, else
                          libx264), software, or an ffmpeg encoder name
  -h, --help              Print this help

render:
  --out <folder>          Where the files go (default: the current folder)
  --size <id>             A size to draw (repeatable; default: all)
  --rows <rows.json>      A JSON list of token values: one render per row
  --format <format>       png (default), jpeg, webp, pdf, apng, gif, mp4 or webm
  --time <s>              A still of that moment of an animated scene
  --quality <1-100>       JPEG, WebP and video quality
  --max-kb <n>            Lower a lossy file's quality until it fits
  render reads images beside the scene file, prints what the render tool
  replies, and exits 1 if the design has a ! defect.

Each flag also takes its value as --flag=value.

Docs: https://github.com/keyline-dev/keyline";

/// The parsed command line.
#[derive(Debug, Default, PartialEq)]
pub struct Options {
    /// `--allow-read`: folders local files may be read from, as given.
    pub allow_read: Vec<PathBuf>,
    /// `--no-motion` (or `--no-motion=true`): leave animation and video
    /// out of the tools.
    pub no_motion: bool,
    /// `--data`: the data directory (default `~/.keyline-mcp`).
    pub data: Option<PathBuf>,
    /// `--fonts`: extra font folders.
    pub fonts: Vec<PathBuf>,
    /// `--renderer`: GPU (default) or CPU.
    pub renderer: Backend,
    /// `--ffmpeg`: the ffmpeg program (default: `ffmpeg` on the PATH).
    pub ffmpeg: Option<PathBuf>,
    /// `--encoder`: the H.264 encoder (default `auto`).
    pub encoder: Option<String>,
    /// `-h`/`--help`: print [`HELP`] and exit.
    pub help: bool,
    /// Arguments that aren't flags: a subcommand and its operands.
    pub operands: Vec<String>,
    /// `--out`: where `render` writes its files.
    pub out: Option<PathBuf>,
    /// `--size`: the sizes `render` draws (default: all).
    pub sizes: Vec<String>,
    /// `--rows`: a JSON file of token rows, one render per row.
    pub rows: Option<PathBuf>,
    /// `--format`: `render`'s file format (default `png`).
    pub format: Option<String>,
    /// `--time`: `render` a still at this moment, seconds.
    pub time: Option<f64>,
    /// `--quality`: `render`'s lossy quality, 1–100.
    pub quality: Option<u32>,
    /// `--max-kb` (or `--maxKB`): `render`'s file size cap, KB.
    pub max_kb: Option<u32>,
}

impl Options {
    /// Reads the command line, without the program name.
    ///
    /// # Errors
    /// An unknown flag, a flag without its value, or a bad `--renderer`
    /// or `--no-motion` value.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut o = Options::default();
        let mut args = args.into_iter().peekable();
        while let Some(arg) = args.next() {
            // A client's setting left empty can arrive as "", or as its
            // placeholder unfilled (Claude Desktop: `${user_config.folders}`).
            if unset(&arg) {
                continue;
            }
            if !arg.starts_with('-') {
                o.operands.push(arg);
                continue;
            }
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
                _ => (arg.as_str(), None),
            };
            if flag == "--allow-read" {
                // Every folder up to the next flag, so a client can pass a
                // list (Claude Desktop's folder picker); none is fine.
                o.allow_read
                    .extend(inline.filter(|d| !unset(d)).map(PathBuf::from));
                while let Some(dir) = args.next_if(|a| !a.starts_with('-')) {
                    if !unset(&dir) {
                        o.allow_read.push(dir.into());
                    }
                }
                continue;
            }
            let mut value = || {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .with_context(|| format!("{flag} needs a value; see keyline-mcp --help"))
            };
            match flag {
                "-h" | "--help" => o.help = true,
                // A value too, for clients that can only fill one in.
                "--no-motion" => {
                    o.no_motion = match inline.as_deref() {
                        None | Some("true") => true,
                        Some(v) if v == "false" || unset(v) => false,
                        Some(v) => bail!("--no-motion takes true or false, not {v}"),
                    }
                }
                "--data" => o.data = Some(value()?.into()),
                "--fonts" => o.fonts.push(value()?.into()),
                "--renderer" => o.renderer = value()?.parse()?,
                "--ffmpeg" => o.ffmpeg = Some(value()?.into()),
                "--encoder" => o.encoder = Some(value()?),
                "--out" => o.out = Some(value()?.into()),
                "--size" => o.sizes.push(value()?),
                "--rows" => o.rows = Some(value()?.into()),
                "--format" => o.format = Some(value()?),
                "--time" => o.time = Some(number(flag, &value()?)?),
                "--quality" => o.quality = Some(number(flag, &value()?)?),
                "--max-kb" | "--maxKB" => o.max_kb = Some(number(flag, &value()?)?),
                _ => bail!("unknown argument {arg}; see keyline-mcp --help"),
            }
        }
        Ok(o)
    }
}

/// `v` as the number `flag` takes.
fn number<T: std::str::FromStr>(flag: &str, v: &str) -> Result<T> {
    v.parse()
        .map_err(|_| anyhow::anyhow!("{flag} takes a number, not {v}"))
}

/// A value a client left unset: empty, or its `${…}` placeholder unfilled.
fn unset(v: &str) -> bool {
    v.is_empty() || (v.starts_with("${") && v.ends_with('}'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options> {
        Options::parse(args.iter().map(|a| (*a).to_owned()))
    }

    #[test]
    fn no_flags_means_every_default() {
        assert_eq!(parse(&[]).unwrap(), Options::default());
        assert_eq!(Options::default().renderer, Backend::Gpu);
    }

    #[test]
    fn flags_take_their_value_after_a_space_or_an_equals_sign() {
        let o = parse(&[
            "--data",
            "/d",
            "--fonts=/f1",
            "--fonts",
            "/f2",
            "--renderer=cpu",
            "--ffmpeg",
            "/bin/ffmpeg",
            "--encoder",
            "software",
            "--allow-read",
            "/a",
            "--no-motion",
        ])
        .unwrap();
        assert_eq!(o.data, Some("/d".into()));
        assert_eq!(o.fonts, [PathBuf::from("/f1"), PathBuf::from("/f2")]);
        assert_eq!(o.renderer, Backend::Cpu);
        assert_eq!(o.ffmpeg, Some("/bin/ffmpeg".into()));
        assert_eq!(o.encoder.as_deref(), Some("software"));
        assert_eq!(o.allow_read, [PathBuf::from("/a")]);
        assert!(o.no_motion && !o.help);
    }

    #[test]
    fn allow_read_takes_every_folder_up_to_the_next_flag() {
        let o = parse(&[
            "--allow-read",
            "/a",
            "",
            "/b",
            "--no-motion=false",
            "--allow-read",
            "",
        ])
        .unwrap();
        assert_eq!(o.allow_read, [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert!(!o.no_motion);
        assert!(parse(&["--no-motion=true"]).unwrap().no_motion);
        let o = parse(&["render", "ad.json", "--size", "wide", "--out=dist"]).unwrap();
        assert_eq!(o.operands, ["render", "ad.json"]);
        assert_eq!(o.sizes, ["wide"]);
        assert_eq!(o.out, Some("dist".into()));
        let o = parse(&[
            "render",
            "ad.json",
            "--time=2.5",
            "--quality",
            "80",
            "--maxKB",
            "300",
        ])
        .unwrap();
        assert_eq!(
            (o.time, o.quality, o.max_kb),
            (Some(2.5), Some(80), Some(300))
        );
        assert_eq!(parse(&["--max-kb=9"]).unwrap().max_kb, Some(9));
    }

    #[test]
    fn placeholders_a_client_left_unfilled_mean_unset() {
        // What Claude Desktop passed with no folder picked.
        let o = parse(&[
            "--no-motion=${user_config.stills_only}",
            "--allow-read",
            "${user_config.folders}",
        ])
        .unwrap();
        assert!(o.allow_read.is_empty() && !o.no_motion && o.operands.is_empty());
    }

    #[test]
    fn bad_command_lines_say_what_to_fix() {
        let e = |args: &[&str]| parse(args).unwrap_err().to_string();
        assert!(e(&["--bogus"]).contains("unknown argument --bogus"));
        assert!(e(&["--data"]).contains("--data needs a value"));
        assert!(e(&["--renderer", "metal"]).contains("gpu or cpu"));
        assert!(e(&["--no-motion=maybe"]).contains("true or false"));
        assert!(e(&["--time", "soon"]).contains("--time takes a number, not soon"));
    }
}
