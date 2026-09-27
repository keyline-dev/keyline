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
    /// `$KEYLINE_MCP_DATA`, else `~/.keyline-mcp`.
    pub fn open_default() -> Result<Self> {
        let root = match std::env::var_os("KEYLINE_MCP_DATA") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?)
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
        let path = self.scene_path(id)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(scene)?)?;
        // Rename is atomic, so a crash never leaves a half-written scene.
        std::fs::rename(&tmp, &path)?;
        Ok(())
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
    pub fn render_path(&self, scene_id: &str, version: u64, size_id: &str) -> Result<PathBuf> {
        check_id(scene_id)?;
        check_id(size_id)?;
        let dir = self.root.join("renders").join(scene_id);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{size_id}-v{version}.png")))
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
        assert!(s.render_path("ok", 1, "a/b").is_err());
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
}
