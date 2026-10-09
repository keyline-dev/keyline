//! On-disk storage: content-addressed asset bytes and the web-font cache in
//! a server-owned directory, and each design in a folder of its own,
//! `<workspace>/<id>/`, holding `<id>.keyline.json` and `renders/`. Scenes
//! made before design folders stay in the data directory's `scenes/`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::scene::Scene;

/// The data directory, and the workspace folders designs live in.
pub struct Store {
    root: PathBuf,
    /// Folders designs are looked up in; new ones go in the first. None:
    /// scenes stay in the data directory.
    workspace: Vec<PathBuf>,
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
        Ok(Store {
            root,
            workspace: Vec::new(),
        })
    }

    /// The store with designs kept in folders of `workspace`, the first
    /// taking new ones.
    #[must_use]
    pub fn with_workspace(mut self, workspace: Vec<PathBuf>) -> Self {
        self.workspace = workspace;
        self
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

    /// Saves a new scene as `name`, or under a fresh id, in a design
    /// folder of its own when there is a workspace. Returns its id and
    /// that folder.
    ///
    /// # Errors
    /// A bad or taken name, or I/O failure.
    pub fn create(&self, name: Option<&str>, scene: &Scene) -> Result<(String, Option<PathBuf>)> {
        let id = name.map_or_else(|| self.new_scene_id(), str::to_owned);
        check_id(&id)?;
        if self.scene_path(&id)?.is_file() {
            bail!("a design named {id} exists; edit it by that id, or pick another name");
        }
        let dir = self.workspace.first().map(|w| w.join(&id));
        if let Some(dir) = &dir {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        self.save(&id, scene)?;
        Ok((id, dir))
    }

    /// Loads a scene.
    ///
    /// # Errors
    /// Bad id, missing scene, or invalid JSON.
    pub fn load(&self, id: &str) -> Result<Scene> {
        let path = self.scene_path(id)?;
        let Ok(json) = std::fs::read(&path) else {
            let names = self.designs();
            if names.is_empty() {
                bail!("no scene {id}");
            }
            bail!("no scene {id}; designs: {}", names.join(", "));
        };
        Ok(serde_json::from_slice(&json)?)
    }

    /// Deletes a scene kept in the data directory, and its renders, as far
    /// as it can. A design folder is never touched: it holds the user's files.
    pub fn discard(&self, id: &str) {
        if check_id(id).is_err() || self.design_dir(id).is_some() {
            return;
        }
        let _ = std::fs::remove_file(self.root.join("scenes").join(format!("{id}.json")));
        let _ = std::fs::remove_dir_all(self.root.join("renders").join(id));
    }

    /// The designs in the workspace's first folder, by name.
    // ponytail: every one, unpaged; cap the list if a folder holds hundreds.
    fn designs(&self) -> Vec<String> {
        let Some(Ok(entries)) = self.workspace.first().map(std::fs::read_dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| self.design_dir(n).is_some())
            .collect();
        names.sort_unstable();
        names
    }

    /// The design folder holding scene `id`, if it has one.
    fn design_dir(&self, id: &str) -> Option<PathBuf> {
        self.workspace
            .iter()
            .map(|w| w.join(id))
            .find(|d| d.join(scene_file(id)).is_file())
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
        let dir = match self.design_dir(scene_id) {
            Some(d) => d.join("renders"),
            None => self.root.join("renders").join(scene_id),
        };
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{size_id}-v{version}.{ext}")))
    }

    /// Where scene `id` is, or goes: its design folder, a scene from before
    /// design folders, or a new design's folder in the workspace.
    fn scene_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        if let Some(d) = self.design_dir(id) {
            return Ok(d.join(scene_file(id)));
        }
        let old = self.root.join("scenes").join(format!("{id}.json"));
        Ok(match self.workspace.first() {
            Some(w) if !old.is_file() => w.join(id).join(scene_file(id)),
            _ => old,
        })
    }

    /// The store's directory.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// A design's scene file, in its folder.
fn scene_file(id: &str) -> String {
    format!("{id}.keyline.json")
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
    fn designs_live_in_folders_of_their_own() {
        let work = tempfile::tempdir().unwrap();
        let s = temp_store("designs");
        let scene: Scene = serde_json::from_value(serde_json::json!({
            "width": 10, "height": 10, "sizes": [{"id": "a", "width": 10, "height": 10}]
        }))
        .unwrap();
        // A scene from before design folders still loads, and renders where it did.
        s.save("old", &scene).unwrap();
        let s = s.with_workspace(vec![work.path().to_path_buf()]);
        assert_eq!(s.load("old").unwrap(), scene);
        assert!(
            s.render_path("old", 1, "a", "png")
                .unwrap()
                .starts_with(s.root())
        );

        let (id, dir) = s.create(Some("sale"), &scene).unwrap();
        let dir = dir.unwrap();
        assert_eq!(
            (id.as_str(), dir.clone()),
            ("sale", work.path().join("sale"))
        );
        assert!(dir.join("sale.keyline.json").is_file());
        assert_eq!(s.load("sale").unwrap(), scene);
        assert_eq!(
            s.render_path("sale", 2, "a", "png").unwrap(),
            dir.join("renders/a-v2.png")
        );
        assert!(
            s.create(Some("sale"), &scene)
                .unwrap_err()
                .to_string()
                .contains("exists")
        );
        assert!(s.create(Some("old"), &scene).is_err());
        // A missing design's error names the ones there are.
        assert_eq!(
            s.load("sael").unwrap_err().to_string(),
            "no scene sael; designs: sale"
        );
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
