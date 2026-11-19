//! Versioned GPUI operator persistence. Legacy version-zero JSON remains readable.
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const VERSION: u32 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision([u8; 32]);
pub struct OperatorStore {
    path: PathBuf,
}
impl OperatorStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        let file = fs::File::open(&self.path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("Operator settings exceed 8 MiB".into());
        }
        Ok(bytes)
    }
    fn schema(bytes: &[u8]) -> Result<(), String> {
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let version = value
            .get("version")
            .map(|v| v.as_u64().ok_or("Invalid operator settings version"))
            .transpose()?
            .unwrap_or(0);
        if version > u64::from(VERSION) {
            return Err(format!("Unsupported operator settings version {version}"));
        }
        Ok(())
    }
    pub fn load<T: DeserializeOwned>(&self) -> Result<(T, Revision), String> {
        let bytes = self.bytes()?;
        Self::schema(&bytes)?;
        Ok((
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
            Revision(Sha256::digest(&bytes).into()),
        ))
    }
    pub fn save<T: Serialize>(
        &self,
        value: &T,
        expected: Option<Revision>,
    ) -> Result<Revision, String> {
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(self.path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let actual = if self.path.exists() {
            let old = self.bytes()?;
            Self::schema(&old)?;
            Some(Revision(Sha256::digest(old).into()))
        } else {
            None
        };
        if actual != expected {
            return Err("Operator settings changed on disk; reload before saving".into());
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        Self::schema(&bytes)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temp.write_all(&bytes)
            .and_then(|()| temp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        temp.persist(&self.path).map_err(|e| e.to_string())?;
        Ok(Revision(Sha256::digest(bytes).into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_legacy_and_rejects_stale_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("operator.json");
        fs::write(&path, b"{\"theme\":2}").unwrap();
        let store = OperatorStore::new(path);
        let (old, revision) = store.load::<serde_json::Value>().unwrap();
        assert_eq!(old["theme"], 2);
        let updated = serde_json::json!({"version":1,"theme":3});
        store.save(&updated, Some(revision)).unwrap();
        assert!(store
            .save(&old, Some(revision))
            .unwrap_err()
            .contains("changed on disk"));
    }
    #[test]
    fn preserves_malformed_and_future_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("operator.json");
        let store = OperatorStore::new(&path);
        for bytes in [b"broken".as_slice(), b"{\"version\":99}".as_slice()] {
            fs::write(&path, bytes).unwrap();
            assert!(store.load::<serde_json::Value>().is_err());
            assert!(store.save(&serde_json::json!({"version":1}), None).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
}
