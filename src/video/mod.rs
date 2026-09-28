//! Video through ffmpeg, run as a separate program so its license never
//! touches keyline's: found at runtime (no hard dependency), fed raw
//! frames through a pipe, and encoding on the GPU when the machine has a
//! hardware encoder that works.

pub mod audio;
pub mod decode;
pub mod encode;
pub mod frame;
pub mod probe;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// What to say when a call needs ffmpeg and there is none.
pub const MISSING: &str = "video needs ffmpeg: install it (brew install ffmpeg, apt install ffmpeg) \\
or set KEYLINE_MCP_FFMPEG; stills, apng and gif work without it";

/// The ffmpeg program: `$KEYLINE_MCP_FFMPEG`, else `ffmpeg` on the PATH.
/// Looked up on every call, so installing it needs no restart.
///
/// # Errors
/// [`MISSING`] when there is none.
pub fn ffmpeg() -> Result<PathBuf, String> {
    tool("KEYLINE_MCP_FFMPEG", "ffmpeg")
}

/// The ffprobe program, next to ffmpeg or on the PATH.
///
/// # Errors
/// [`MISSING`] when there is none.
pub fn ffprobe() -> Result<PathBuf, String> {
    if let Ok(f) = ffmpeg()
        && let Some(dir) = f.parent()
    {
        let probe = dir.join(format!("ffprobe{}", std::env::consts::EXE_SUFFIX));
        if probe.is_file() {
            return Ok(probe);
        }
    }
    tool("KEYLINE_MCP_FFPROBE", "ffprobe")
}

fn tool(var: &str, name: &str) -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os(var).map(PathBuf::from) {
        return if p.is_file() {
            Ok(p)
        } else {
            Err(MISSING.into())
        };
    }
    let exe = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|d| d.join(&exe))
        .find(|p| p.is_file())
        .ok_or_else(|| MISSING.into())
}

/// Hardware H.264 encoders worth trying on this platform, best first.
fn hardware_h264() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["h264_videotoolbox"]
    } else if cfg!(windows) {
        &["h264_nvenc", "h264_qsv", "h264_amf", "h264_mf"]
    } else {
        &["h264_nvenc", "h264_qsv"]
    }
}

/// The H.264 encoder to use: `$KEYLINE_MCP_ENCODER` (`software`, or an
/// encoder name), else the first hardware encoder that encodes a test frame
/// on this machine, else software (`libx264`). Probed once per run:
/// `ffmpeg -encoders` lists what ffmpeg was built with, not what the
/// machine can run.
pub fn h264_encoder(ffmpeg: &std::path::Path) -> String {
    static CHOSEN: OnceLock<String> = OnceLock::new();
    match std::env::var("KEYLINE_MCP_ENCODER").as_deref() {
        Ok("software") => return "libx264".into(),
        Ok(name) if !name.is_empty() && name != "auto" => return name.into(),
        _ => {}
    }
    CHOSEN
        .get_or_init(|| {
            hardware_h264()
                .iter()
                .find(|enc| works(ffmpeg, enc))
                .map_or_else(|| "libx264".into(), |e| (*e).to_owned())
        })
        .clone()
}

/// Whether `encoder` encodes one small frame here.
fn works(ffmpeg: &std::path::Path, encoder: &str) -> bool {
    Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=128x128:d=0.1")
        .args(["-frames:v", "1", "-c:v", encoder, "-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_missing_ffmpeg_says_how_to_get_it() {
        // An explicit path that doesn't exist is reported, not searched past.
        let e = super::tool("KEYLINE_MCP_TEST_NO_SUCH_TOOL", "keyline-no-such-tool").unwrap_err();
        assert!(
            e.contains("brew install ffmpeg") && e.contains("apng and gif work"),
            "{e}"
        );
    }
}
