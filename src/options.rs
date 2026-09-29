//! The server's command line. Every setting is a flag, so an MCP client's
//! config shows all of it in `args`.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::gpu::Backend;

/// What `--help` prints: every flag.
pub const HELP: &str = "keyline-mcp: an AI-native design engine, an MCP server over stdio.

Usage: keyline-mcp [options]

Options:
  --allow-read <folder>   Let asset_add read local files by path inside this folder
                          (repeatable). Without it, paths are refused. Paths are
                          resolved through symlinks before the check.
  --no-motion             Leave animation and video out of the tools: stills
                          only, and fewer tokens of tool definitions per turn
  --data <folder>         Scenes, assets, renders and the web-font cache
                          (default: ~/.keyline-mcp)
  --fonts <folder>        An extra folder of .ttf and .otf fonts (repeatable)
  --renderer gpu|cpu      gpu (default: the GPU, falling back to the CPU) or cpu
  --ffmpeg <path>         The ffmpeg program for video (default: ffmpeg on the PATH)
  --encoder <name>        H.264 encoder: auto (default: a working GPU encoder, else
                          libx264), software, or an ffmpeg encoder name
  -h, --help              Print this help

Each flag also takes its value as --flag=value.

Docs: https://github.com/keyline-dev/keyline";

/// The parsed command line.
#[derive(Debug, Default, PartialEq)]
pub struct Options {
    /// `--allow-read`: folders local files may be read from, as given.
    pub allow_read: Vec<PathBuf>,
    /// `--no-motion`: leave animation and video out of the tools.
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
}

impl Options {
    /// Reads the command line, without the program name.
    ///
    /// # Errors
    /// An unknown flag, a flag without its value, or a bad `--renderer`.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut o = Options::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
                _ => (arg.as_str(), None),
            };
            let mut value = || {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .with_context(|| format!("{flag} needs a value; see keyline-mcp --help"))
            };
            match flag {
                "-h" | "--help" => o.help = true,
                "--no-motion" => o.no_motion = true,
                "--allow-read" => o.allow_read.push(value()?.into()),
                "--data" => o.data = Some(value()?.into()),
                "--fonts" => o.fonts.push(value()?.into()),
                "--renderer" => o.renderer = value()?.parse()?,
                "--ffmpeg" => o.ffmpeg = Some(value()?.into()),
                "--encoder" => o.encoder = Some(value()?),
                _ => bail!("unknown argument {arg}; see keyline-mcp --help"),
            }
        }
        Ok(o)
    }
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
    fn bad_command_lines_say_what_to_fix() {
        let e = |args: &[&str]| parse(args).unwrap_err().to_string();
        assert!(e(&["--bogus"]).contains("unknown argument --bogus"));
        assert!(e(&["--data"]).contains("--data needs a value"));
        assert!(e(&["--renderer", "metal"]).contains("gpu or cpu"));
    }
}
