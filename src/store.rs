//! On-disk storage in a server-owned directory: scene JSON, content-addressed
//! asset bytes, and rendered PNGs. No tool argument is ever a path.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::scene::Scene;

/// The server-owned data directory.
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// `data` (`--data`), else `.keyline-mcp` in the user's home folder
    /// (`%USERPROFILE%` on Windows).
    ///
    /// # Errors
    /// No `data` and no home folder, or the directories can't be created.
    pub fn open_default(data: Option<PathBuf>) -> Result<Self> {
        let root = match data {
            Some(dir) => dir,
            None => std::env::home_dir()
                .context("no home folder; pass --data <folder>")?
                .join(".keyline-mcp"),
        };
        Self::open(root)
    }

    /// Opens (and creates) a store at `root`.
    ///
    /// # Errors
    /// When the directories can't be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for dir in ["scenes", "assets", "renders"] {
            std::fs::create_dir_all(root.join(dir))
                .with_context(|| format!("creating {}", root.join(dir).display()))?;
        }
        Ok(Store { root })
    }

    /// Where asset bytes live, named by SHA-256.
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// A fresh scene id.
    pub fn new_scene_id(&self) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("s{}", &sha256_hex(&nanos.to_le_bytes())[..10])
    }

    /// Loads a scene.
    ///
    /// # Errors
    /// Bad id, missing scene, or invalid JSON.
    pub fn load(&self, id: &str) -> Result<Scene> {
        let path = self.scene_path(id)?;
        let json = std::fs::read(&path).with_context(|| format!("no scene {id}"))?;
        Ok(serde_json::from_slice(&json)?)
    }

    /// Saves a scene atomically.
    ///
    /// # Errors
    /// Bad id or I/O failure.
    pub fn save(&self, id: &str, scene: &Scene) -> Result<()> {
        write_atomic(&self.scene_path(id)?, &serde_json::to_vec_pretty(scene)?)
    }

    /// Stores bytes under their SHA-256 and returns the hash.
    pub fn put_asset(&self, bytes: &[u8]) -> Result<String> {
        let hash = sha256_hex(bytes);
        let path = self.assets_dir().join(&hash);
        if !path.exists() {
            std::fs::write(&path, bytes)?;
        }
        Ok(hash)
    }

    /// Where a render of `size_id` at scene `version` is written.
    pub fn render_path(
        &self,
        scene_id: &str,
        version: u64,
        size_id: &str,
        ext: &str,
    ) -> Result<PathBuf> {
        check_id(scene_id)?;
        check_id(size_id)?;
        let dir = self.root.join("renders").join(scene_id);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{size_id}-v{version}.{ext}")))
    }

    fn scene_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self.root.join("scenes").join(format!("{id}.json")))
    }

    /// The store's directory.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Ids become file names, so allow only `[A-Za-z0-9_-]`.
pub(crate) fn check_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("bad id {id:?}: use letters, digits, _ or -");
    }
    Ok(())
}

/// Writes `bytes` to `path` through a fresh temporary file in the same
/// folder, renamed into place: a crash never leaves a half-written file, and
/// two servers sharing a folder never write the same temporary file.
///
/// # Errors
/// I/O failure.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .with_context(|| format!("no folder for {}", path.display()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    std::io::Write::write_all(&mut tmp, bytes)?;
    tmp.persist(path)?;
    Ok(())
}

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> Store {
        let dir =
            std::env::temp_dir().join(format!("keyline-mcp-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Store::open(dir).unwrap()
    }

    #[test]
    fn ids_cannot_escape_the_store() {
        let s = temp_store("ids");
        assert!(s.load("../etc/passwd").is_err());
        assert!(s.render_path("ok", 1, "a/b", "png").is_err());
        assert!(check_id("portrait_2-x").is_ok());
    }

    #[test]
    fn assets_are_content_addressed() {
        let s = temp_store("assets");
        let a = s.put_asset(b"hello").unwrap();
        assert_eq!(a, s.put_asset(b"hello").unwrap());
        assert_eq!(
            a,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(std::fs::read(s.assets_dir().join(a)).unwrap(), b"hello");
    }

    #[test]
    fn scenes_round_trip() {
        let s = temp_store("scenes");
        let scene: Scene = serde_json::from_value(serde_json::json!({
            "width": 10, "height": 10, "sizes": [{"id": "a", "width": 10, "height": 10}]
        }))
        .unwrap();
        s.save("s1", &scene).unwrap();
        assert_eq!(s.load("s1").unwrap(), scene);
    }

    #[test]
    fn atomic_writes_replace_and_leave_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
