//! Local files an agent may name by path, so their bytes never pass through
//! the model. Off unless the server is started with `--allow-read <folder>`;
//! a path is resolved through every symlink first, and must then lie inside
//! one of those folders. A leading `~` is the user's home folder, as in a
//! shell, since agents copy paths the way people write them.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// `path` with a leading `~` component replaced by the home folder; any
/// other path, or `~` with no home folder known, as it is.
pub fn expand_home(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), std::env::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

/// The folders local paths may be read from, resolved at startup.
#[derive(Debug, Default)]
pub struct AllowedDirs(Vec<PathBuf>);

impl AllowedDirs {
    /// The folders named with `--allow-read`, resolved through symlinks.
    ///
    /// # Errors
    /// A folder that doesn't exist.
    pub fn new(dirs: &[PathBuf]) -> Result<Self> {
        dirs.iter()
            .map(|d| {
                std::fs::canonicalize(d)
                    .with_context(|| format!("--allow-read {}: no such folder", d.display()))
            })
            .collect::<Result<_>>()
            .map(AllowedDirs)
    }

    /// The allowed folders, resolved.
    pub fn dirs(&self) -> &[PathBuf] {
        &self.0
    }

    /// The bytes of the regular file at `path`, at most `max` of them.
    ///
    /// # Errors
    /// Paths are off, the file is missing, outside every allowed folder
    /// (after following symlinks), not a regular file, or too large.
    pub fn read(&self, path: impl AsRef<Path>, max: usize) -> Result<Vec<u8>, String> {
        let path = path.as_ref();
        if self.0.is_empty() {
            return Err("local paths are off; start the server with --allow-read <folder>".into());
        }
        // The real location, `~` expanded and every symlink and `..` followed.
        let real = std::fs::canonicalize(expand_home(path))
            .map_err(|_| format!("no such file {}", path.display()))?;
        if !self.0.iter().any(|d| real.starts_with(d)) {
            return Err(format!(
                "{} is outside the folders the server may read (--allow-read)",
                path.display()
            ));
        }
        // ponytail: a folder swapped for a symlink between the check and the
        // open could still redirect it; O_NOFOLLOW per component if that matters.
        // Checked before opening: Windows refuses to open a folder at all,
        // which would hide this clearer answer.
        if !std::fs::metadata(&real).is_ok_and(|m| m.is_file()) {
            return Err(format!("{} isn't a file", path.display()));
        }
        let file = std::fs::File::open(&real).map_err(|e| format!("{}: {e}", path.display()))?;
        let meta = file
            .metadata()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let max64 = u64::try_from(max).unwrap_or(u64::MAX);
        if meta.len() > max64 {
            return Err(format!(
                "{} is larger than {} MB",
                path.display(),
                max >> 20
            ));
        }
        let mut bytes = Vec::new();
        file.take(max64.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{AllowedDirs, expand_home};
    use std::path::Path;

    #[test]
    fn a_leading_tilde_is_the_home_folder() {
        let home = std::env::home_dir().unwrap();
        assert_eq!(expand_home(Path::new("~")), home);
        assert_eq!(
            expand_home(Path::new("~/a b/c.png")),
            home.join("a b/c.png")
        );
        // Only a whole leading `~` component.
        for p in ["~user/c.png", "/a/~/c.png", "a~/c.png"] {
            assert_eq!(expand_home(Path::new(p)), Path::new(p), "{p}");
        }
    }

    /// A fresh, empty folder.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("keyline-local-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn paths_are_off_unless_a_folder_is_allowed() {
        let none = AllowedDirs::new(&[]).unwrap();
        assert!(
            none.read("/etc/hosts", 1 << 20)
                .unwrap_err()
                .contains("--allow-read")
        );
        assert!(AllowedDirs::new(&["/no/such/folder".into()]).is_err());
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
        let dirs = AllowedDirs::new(std::slice::from_ref(&allowed)).unwrap();

        assert_eq!(
            dirs.read(allowed.join("sub/logo.svg"), 1024).unwrap(),
            b"<svg/>"
        );
        // `..` out of the folder, and a missing file.
        let up = allowed.join("../outside/secret.txt");
        assert!(
            dirs.read(up, 1024)
                .unwrap_err()
                .contains("outside the folders")
        );
        assert!(
            dirs.read(allowed.join("nope.png"), 1024)
                .unwrap_err()
                .contains("no such file")
        );
        // A folder, and a file over the limit.
        assert!(
            dirs.read(allowed.join("sub"), 1024)
                .unwrap_err()
                .contains("isn't a file")
        );
        assert!(
            dirs.read(allowed.join("sub/logo.svg"), 3)
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
                    dirs.read(&p, 1024)
                        .unwrap_err()
                        .contains("outside the folders"),
                    "{p:?}"
                );
            }
            // A symlink that stays inside is fine.
            std::os::unix::fs::symlink(allowed.join("sub/logo.svg"), allowed.join("alias.svg"))
                .unwrap();
            assert_eq!(
                dirs.read(allowed.join("alias.svg"), 1024).unwrap(),
                b"<svg/>"
            );
        }
    }
}
