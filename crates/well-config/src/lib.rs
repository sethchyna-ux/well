//! Validated user preferences with atomic persistence.
pub mod theia;
pub use theia::TheiasPrismPanel;

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAX_CONFIG_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub font_family: String,
    pub font_size: f32,
    pub line_spacing: f32,
    pub padding: u16,
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub geometric_border: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_family: if cfg!(target_os = "macos") {
                "Menlo"
            } else {
                "DejaVu Sans Mono"
            }
            .into(),
            font_size: 14.0,
            line_spacing: 2.0,
            padding: 8,
            foreground: [208, 216, 225],
            background: [27, 35, 45],
            geometric_border: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub version: u32,
    pub appearance: Appearance,
    pub scroll_lines: u16,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            appearance: Appearance::default(),
            scroll_lines: 3,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("Unsupported settings version {}", self.version));
        }
        let a = &self.appearance;
        if a.font_family.trim().is_empty()
            || a.font_family.len() > 128
            || a.font_family.chars().any(char::is_control)
        {
            return Err("Font family must contain 1–128 bytes without control characters".into());
        }
        if !a.font_size.is_finite() || !(8.0..=48.0).contains(&a.font_size) {
            return Err("Font size must be between 8 and 48".into());
        }
        if !a.line_spacing.is_finite() || !(0.0..=16.0).contains(&a.line_spacing) {
            return Err("Line spacing must be between 0 and 16".into());
        }
        if a.padding > 64 {
            return Err("Padding must be at most 64".into());
        }
        if !(1..=20).contains(&self.scroll_lines) {
            return Err("Scroll speed must be between 1 and 20 lines".into());
        }
        Ok(())
    }
}

/// Keeps the last file contents to avoid overwriting changes made by another instance.
pub struct ConfigStore {
    path: PathBuf,
    original: Option<Vec<u8>>,
}
impl ConfigStore {
    pub fn default_path() -> io::Result<PathBuf> {
        if let Some(path) = std::env::var_os("WELL_CONFIG_FILE") {
            return Ok(PathBuf::from(path));
        }
        let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?;
        #[cfg(target_os = "macos")]
        let base = PathBuf::from(home).join("Library/Application Support/Well");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| PathBuf::from(home).join(".config"))
            .join("well");
        Ok(base.join("settings.toml"))
    }
    pub fn load(path: PathBuf) -> io::Result<(Self, Preferences)> {
        let original = read_file(&path)?;
        let prefs = if let Some(bytes) = &original {
            let text = std::str::from_utf8(bytes).map_err(invalid)?;
            toml::from_str::<Preferences>(text).map_err(invalid)?
        } else {
            Preferences::default()
        };
        prefs.validate().map_err(invalid)?;
        Ok((Self { path, original }, prefs))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn save(&mut self, prefs: &Preferences) -> io::Result<()> {
        prefs.validate().map_err(invalid)?;
        let bytes = toml::to_string_pretty(prefs).map_err(invalid)?.into_bytes();
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent)?;
        let lock_path = self.path.with_extension("toml.lock");
        if let Ok(metadata) = std::fs::symlink_metadata(&lock_path) {
            if !metadata.is_file() {
                return Err(invalid("Settings lock must be a regular file"));
            }
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path)?;
        lock.try_lock().map_err(|_| {
            io::Error::other("Another Well instance is saving settings. Try again.")
        })?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        if read_file(&self.path)? != self.original {
            return Err(io::Error::other(
                "Settings changed on disk. Restart Well to load them before saving.",
            ));
        }
        temp.persist(&self.path).map_err(|e| e.error)?;
        self.original = Some(bytes);
        // A directory flush improves crash durability where the filesystem supports it.
        // The atomic rename already succeeded: never report a failed save after commit.
        let _ = std::fs::File::open(parent).and_then(|f| f.sync_all());
        Ok(())
    }
}
fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
fn read_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !metadata.is_file() {
        return Err(invalid(
            "Settings path must be a regular file, not a symlink or directory",
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(invalid("Settings file exceeds 64 KiB"));
    }
    Ok(Some(bytes))
}
