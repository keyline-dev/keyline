//! Local files an agent may name by path, so their bytes never pass through
//! the model. Off unless the server is started with `--allow-read <folder>`;
//! a path is resolved through every symlink first, and must then lie inside
//! one of those folders.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

/// The folders local paths may be read from, resolved at startup.
#[derive(Debug, Default)]
pub struct AllowedDirs(Vec<PathBuf>);

impl AllowedDirs {
    /// Reads `--allow-read <folder>` (repeatable, or `--allow-read=<folder>`)
    /// from the command line, without the program name.
    ///
    /// # Errors
    /// An unknown argument, a flag without a folder, or a folder that
    /// doesn't exist.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut dirs = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let dir = match arg.strip_prefix("--allow-read") {
                Some("") => args.next().context("--allow-read needs a folder")?,
                Some(rest) if rest.starts_with('=') => rest[1..].to_owned(),
                _ => bail!("unknown argument {arg}; see keyline-mcp --help"),
            };
            dirs.push(
                std::fs::canonicalize(&dir)
                    .with_context(|| format!("--allow-read {dir}: no such folder"))?,
            );
        }
        Ok(AllowedDirs(dirs))
    }

    /// The bytes of the regular file at `path`, at most `max` of them.
    ///
    /// # Errors
    /// Paths are off, the file is missing, outside every allowed folder
    /// (after following symlinks), not a regular file, or too large.
    pub fn read(&self, path: &str, max: usize) -> Result<Vec<u8>, String> {
        if self.0.is_empty() {
            return Err("local paths are off; start the server with --allow-read <folder>".into());
        }
        // The real location, every symlink and `..` followed.
        let real = std::fs::canonicalize(path).map_err(|_| format!("no such file {path}"))?;
        if !self.0.iter().any(|d| real.starts_with(d)) {
            return Err(format!(
                "{path} is outside the folders the server may read (--allow-read)"
            ));
        }
        // ponytail: a folder swapped for a symlink between the check and the
        // open could still redirect it; O_NOFOLLOW per component if that matters.
        // Checked before opening: Windows refuses to open a folder at all,
        // which would hide this clearer answer.
        if !std::fs::metadata(&real).is_ok_and(|m| m.is_file()) {
            return Err(format!("{path} isn't a file"));
        }
        let file = std::fs::File::open(&real).map_err(|e| format!("{path}: {e}"))?;
        let meta = file.metadata().map_err(|e| format!("{path}: {e}"))?;
        let max64 = u64::try_from(max).unwrap_or(u64::MAX);
        if meta.len() > max64 {
            return Err(format!("{path} is larger than {} MB", max >> 20));
        }
        let mut bytes = Vec::new();
        file.take(max64.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{path}: {e}"))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::AllowedDirs;

    /// A fresh, empty folder.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("keyline-local-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn paths_are_off_unless_a_folder_is_allowed() {
        let none = AllowedDirs::from_args(Vec::new()).unwrap();
        assert!(
            none.read("/etc/hosts", 1 << 20)
                .unwrap_err()
                .contains("--allow-read")
        );
        assert!(AllowedDirs::from_args(["--bogus".to_owned()]).is_err());
        assert!(AllowedDirs::from_args(["--allow-read".to_owned()]).is_err());
        assert!(AllowedDirs::from_args(["--allow-read=/no/such/folder".to_owned()]).is_err());
    }

    #[test]
    fn only_regular_files_inside_the_folder_are_read() {
        let base = scratch("read");
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        std::fs::create_dir_all(allowed.join("sub")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(allowed.join("sub/logo.svg"), b"<svg/>").unwrap();
        std::fs::write(outside.join("secret.txt"), b"secret").unwrap();
        let dirs = AllowedDirs::from_args([format!("--allow-read={}", allowed.display())]).unwrap();
        let path = |p: &std::path::Path| p.display().to_string();

        assert_eq!(
            dirs.read(&path(&allowed.join("sub/logo.svg")), 1024)
                .unwrap(),
            b"<svg/>"
        );
        // `..` out of the folder, and a missing file.
        let up = allowed.join("../outside/secret.txt");
        assert!(
            dirs.read(&path(&up), 1024)
                .unwrap_err()
                .contains("outside the folders")
        );
        assert!(
            dirs.read(&path(&allowed.join("nope.png")), 1024)
                .unwrap_err()
                .contains("no such file")
        );
        // A folder, and a file over the limit.
        assert!(
            dirs.read(&path(&allowed.join("sub")), 1024)
                .unwrap_err()
                .contains("isn't a file")
        );
        assert!(
            dirs.read(&path(&allowed.join("sub/logo.svg")), 3)
                .unwrap_err()
                .contains("larger than")
        );

        #[cfg(unix)]
        {
            // Symlinks that lead outside, to a file or through a folder.
            std::os::unix::fs::symlink(outside.join("secret.txt"), allowed.join("link.txt"))
                .unwrap();
            std::os::unix::fs::symlink(&outside, allowed.join("door")).unwrap();
            for p in [allowed.join("link.txt"), allowed.join("door/secret.txt")] {
                assert!(
                    dirs.read(&path(&p), 1024)
                        .unwrap_err()
                        .contains("outside the folders"),
                    "{p:?}"
                );
            }
            // A symlink that stays inside is fine.
            std::os::unix::fs::symlink(allowed.join("sub/logo.svg"), allowed.join("alias.svg"))
                .unwrap();
            assert_eq!(
                dirs.read(&path(&allowed.join("alias.svg")), 1024).unwrap(),
                b"<svg/>"
            );
        }
    }
}
