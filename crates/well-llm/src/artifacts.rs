//! Credential-free local image history with cache-scoped deletion.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub prompt: String,
    pub provider: crate::LlmProvider,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug)]
pub struct ImageArtifact {
    pub path: PathBuf,
    pub metadata: Option<ImageMetadata>,
}
pub struct ImageStore {
    root: PathBuf,
}
impl ImageStore {
    pub fn open_default() -> Result<Self, String> {
        Ok(Self {
            root: super::image_cache_directory()?,
        })
    }
    pub fn list(&self) -> Result<Vec<ImageArtifact>, String> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() || !managed(&path) {
                continue;
            }
            let metadata = fs::read(path.with_extension("json"))
                .ok()
                .filter(|b| b.len() < 65536)
                .and_then(|b| serde_json::from_slice(&b).ok());
            let time = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            files.push((time, ImageArtifact { path, metadata }));
        }
        files.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
        Ok(files.into_iter().take(500).map(|(_, a)| a).collect())
    }
    pub fn remove(&self, path: &Path) -> Result<(), String> {
        let root = self.root.canonicalize().map_err(|e| e.to_string())?;
        if !managed(path)
            || fs::symlink_metadata(path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            || path.canonicalize().map_err(|e| e.to_string())?.parent() != Some(root.as_path())
        {
            return Err("Image deletion is restricted to managed cache files".into());
        }
        fs::remove_file(path).map_err(|e| e.to_string())?;
        match fs::remove_file(path.with_extension("json")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
fn managed(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.starts_with("image_"))
        && path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| matches!(s, "png" | "jpg" | "jpeg" | "webp"))
}
pub(crate) fn record(path: &Path, metadata: ImageMetadata) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?;
    super::write_image_artifact(&path.with_extension("json"), &data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deletion_refuses_escape_and_symlink() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let store = ImageStore {
            root: root.path().into(),
        };
        let path = outside.path().join("image_outside.png");
        fs::write(&path, b"outside").unwrap();
        assert!(store.remove(&path).is_err());
        #[cfg(unix)]
        {
            let link = root.path().join("image_link.png");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(store.remove(&link).is_err());
        }
        let inside = root.path().join("image_local.png");
        fs::write(&inside, b"owned").unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        store.remove(&inside).unwrap();
        assert!(path.exists());
    }
}
