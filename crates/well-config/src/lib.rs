//! Validated user preferences with atomic persistence.
pub mod theia;
pub use theia::{TheiaTelemetry, TheiasPrismPanel};

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAX_CONFIG_BYTES: u64 = 64 * 1024;

pub const CHROMATIC_CAPITAL_PALETTE: &[[u8; 3]] = &[
    [255, 195, 45],  // Amber Gold
    [60, 230, 255],  // Neon Cyan
    [255, 95, 175],  // Hot Pink / Magenta
    [120, 255, 120], // Lime Green
    [255, 140, 50],  // Vivid Orange
    [185, 125, 255], // Electric Violet
    [90, 215, 255],  // Sky Cerulean
    [255, 85, 105],  // Coral Rose
];

pub fn capital_color_at(ch: char, seed: usize, base: [u8; 3], multicolor: bool) -> [u8; 3] {
    if !multicolor {
        return base;
    }
    let idx =
        ((ch as usize).wrapping_mul(13) ^ (seed.wrapping_mul(7))) % CHROMATIC_CAPITAL_PALETTE.len();
    CHROMATIC_CAPITAL_PALETTE[idx]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyntaxTheme {
    pub command: [u8; 3],
    pub builtin: [u8; 3],
    pub valid_path: [u8; 3],
    pub invalid_path: [u8; 3],
    pub keyword: [u8; 3],
    pub string: [u8; 3],
    pub number: [u8; 3],
    pub comment: [u8; 3],
    pub parameter: [u8; 3],
    pub operator: [u8; 3],
    pub delimiter: [u8; 3],
    pub variable: [u8; 3],
    pub error: [u8; 3],
}

impl Default for SyntaxTheme {
    fn default() -> Self {
        Self {
            command: [100, 245, 97],
            builtin: [60, 240, 200],
            valid_path: [100, 245, 97],
            invalid_path: [255, 170, 50],
            keyword: [181, 112, 246],
            string: [245, 215, 110],
            number: [255, 140, 100],
            comment: [110, 125, 150],
            parameter: [80, 210, 245],
            operator: [200, 140, 255],
            delimiter: [180, 170, 225],
            variable: [120, 220, 255],
            error: [255, 75, 75],
        }
    }
}

impl SyntaxTheme {
    pub fn cyberpunk() -> Self {
        Self {
            command: [0, 255, 159],
            builtin: [0, 230, 255],
            valid_path: [0, 255, 159],
            invalid_path: [255, 150, 0],
            keyword: [255, 0, 128],
            string: [255, 225, 53],
            number: [255, 110, 30],
            comment: [90, 110, 130],
            parameter: [0, 180, 255],
            operator: [240, 60, 180],
            delimiter: [160, 140, 240],
            variable: [80, 240, 255],
            error: [255, 30, 60],
        }
    }

    pub fn tokyo_neon() -> Self {
        Self {
            command: [65, 167, 252],
            builtin: [125, 207, 255],
            valid_path: [115, 218, 202],
            invalid_path: [255, 158, 100],
            keyword: [187, 154, 247],
            string: [158, 206, 106],
            number: [255, 158, 100],
            comment: [86, 95, 137],
            parameter: [224, 175, 104],
            operator: [137, 221, 255],
            delimiter: [192, 202, 245],
            variable: [192, 202, 245],
            error: [247, 118, 142],
        }
    }

    pub fn monokai() -> Self {
        Self {
            command: [166, 226, 46],
            builtin: [102, 217, 239],
            valid_path: [166, 226, 46],
            invalid_path: [253, 151, 31],
            keyword: [249, 38, 114],
            string: [230, 219, 116],
            number: [174, 129, 255],
            comment: [117, 113, 94],
            parameter: [253, 151, 31],
            operator: [249, 38, 114],
            delimiter: [248, 248, 242],
            variable: [102, 217, 239],
            error: [249, 38, 114],
        }
    }

    pub fn nord() -> Self {
        Self {
            command: [163, 190, 140],
            builtin: [136, 192, 208],
            valid_path: [143, 188, 187],
            invalid_path: [208, 135, 112],
            keyword: [180, 142, 173],
            string: [235, 203, 139],
            number: [180, 142, 173],
            comment: [94, 129, 172],
            parameter: [129, 161, 193],
            operator: [129, 161, 193],
            delimiter: [229, 233, 240],
            variable: [136, 192, 208],
            error: [191, 97, 106],
        }
    }
}

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
    pub crt_enabled: bool,
    pub crt_curvature: f32,
    pub crt_scanlines: f32,
    pub crt_bloom: f32,
    pub background_opacity: f32,
    pub capital_color: [u8; 3],
    pub highlight_capitals: bool,
    pub multicolor_capitals: bool,
    pub prediction_color: [u8; 3],
    pub syntax_theme: SyntaxTheme,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_family: "OpenDyslexic Nerd Font".into(),
            font_size: 14.0,
            line_spacing: 2.0,
            padding: 8,
            foreground: [208, 216, 225],
            background: [27, 35, 45],
            geometric_border: true,
            crt_enabled: false,
            crt_curvature: 0.04,
            crt_scanlines: 0.15,
            crt_bloom: 0.0015,
            background_opacity: 1.0,
            capital_color: [255, 195, 45],
            highlight_capitals: true,
            multicolor_capitals: true,
            prediction_color: [80, 225, 245],
            syntax_theme: SyntaxTheme::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AiImageConfig {
    pub enabled: bool,
    pub model_path: String,
    pub format: String,         // "safetensors", "gguf", "onnx"
    pub architecture: String,   // "sd15", "sdxl", "flux", "custom"
    pub compute_device: String, // "mps", "cpu", "gpu"
    pub steps: u32,
    pub guidance_scale: f32,
    pub default_width: u32,
    pub default_height: u32,
    pub seed: i64,
    pub prompt: String,
    pub negative_prompt: String,
}
impl Default for AiImageConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_path: "".into(),
            format: "safetensors".into(),
            architecture: "sdxl".into(),
            compute_device: "mps".into(),
            steps: 25,
            guidance_scale: 7.5,
            default_width: 512,
            default_height: 512,
            seed: -1,
            prompt: "cyberpunk terminal workstation, retro sci-fi CRT monitors, glowing green phosphorus, high resolution".into(),
            negative_prompt: "blurry, low quality, artifacts, distorted".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub version: u32,
    pub appearance: Appearance,
    pub scroll_lines: u16,
    pub prompt: well_prompt::AstraeaConfig,
    pub ai_image: AiImageConfig,
    pub initial_cols: u16,
    pub initial_rows: u16,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            appearance: Appearance::default(),
            scroll_lines: 3,
            prompt: well_prompt::AstraeaConfig::default(),
            ai_image: AiImageConfig::default(),
            initial_cols: 100,
            initial_rows: 30,
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
        if !a.crt_curvature.is_finite() || !(0.0..=0.2).contains(&a.crt_curvature) {
            return Err("CRT curvature must be between 0.0 and 0.2".into());
        }
        if !a.crt_scanlines.is_finite() || !(0.0..=1.0).contains(&a.crt_scanlines) {
            return Err("CRT scanlines intensity must be between 0.0 and 1.0".into());
        }
        if !a.crt_bloom.is_finite() || !(0.0..=0.05).contains(&a.crt_bloom) {
            return Err("CRT phosphor bloom must be between 0.0 and 0.05".into());
        }
        if !a.background_opacity.is_finite() || !(0.1..=1.0).contains(&a.background_opacity) {
            return Err("Background opacity must be between 0.1 and 1.0".into());
        }
        if !(1..=20).contains(&self.scroll_lines) {
            return Err("Scroll speed must be between 1 and 20 lines".into());
        }
        if self.initial_cols < 20 || self.initial_cols > 500 {
            return Err("Initial columns must be between 20 and 500".into());
        }
        if self.initial_rows < 5 || self.initial_rows > 200 {
            return Err("Initial rows must be between 5 and 200".into());
        }
        if self.prompt.transient_symbol.trim().is_empty() {
            return Err("Prompt transient symbol cannot be empty".into());
        }
        if self.ai_image.steps == 0 || self.ai_image.steps > 150 {
            return Err("AI Image inference steps must be between 1 and 150".into());
        }
        if !self.ai_image.guidance_scale.is_finite()
            || !(0.0..=30.0).contains(&self.ai_image.guidance_scale)
        {
            return Err("AI Image guidance scale must be between 0.0 and 30.0".into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences_valid() {
        let prefs = Preferences::default();
        assert!(prefs.validate().is_ok());
        assert_eq!(prefs.appearance.font_family, "OpenDyslexic Nerd Font");
        assert_eq!(prefs.appearance.background_opacity, 1.0);
        assert!(!prefs.appearance.crt_enabled);
        assert_eq!(prefs.appearance.crt_curvature, 0.04);
        assert_eq!(prefs.appearance.crt_scanlines, 0.15);
        assert_eq!(prefs.appearance.crt_bloom, 0.0015);
        assert_eq!(prefs.ai_image.format, "safetensors");
        assert_eq!(prefs.ai_image.steps, 25);
    }

    #[test]
    fn test_appearance_crt_validation() {
        let mut prefs = Preferences::default();
        prefs.appearance.crt_curvature = 0.5; // Exceeds 0.2
        assert!(prefs.validate().is_err());

        prefs.appearance.crt_curvature = 0.05;
        prefs.appearance.crt_scanlines = 1.5; // Exceeds 1.0
        assert!(prefs.validate().is_err());

        prefs.appearance.crt_scanlines = 0.2;
        prefs.appearance.background_opacity = 0.05; // Below 0.1
        assert!(prefs.validate().is_err());

        prefs.appearance.background_opacity = 0.85;
        assert!(prefs.validate().is_ok());
    }

    #[test]
    fn test_ai_image_config_validation() {
        let mut prefs = Preferences::default();
        prefs.ai_image.steps = 0;
        assert!(prefs.validate().is_err());

        prefs.ai_image.steps = 50;
        prefs.ai_image.guidance_scale = 50.0; // Exceeds 30.0
        assert!(prefs.validate().is_err());

        prefs.ai_image.guidance_scale = 7.5;
        assert!(prefs.validate().is_ok());
    }

    #[test]
    fn test_theia_panel_toml_validation_gate() {
        let prefs = Preferences::default();
        let mut panel = TheiasPrismPanel::new(&prefs);
        assert!(panel.toml_error.is_none());

        // Valid edit
        panel.raw_toml = toml::to_string_pretty(&prefs).unwrap();
        assert!(toml::from_str::<Preferences>(&panel.raw_toml).is_ok());

        // Corrupt syntax
        panel.raw_toml = "broken = [toml, syntax".to_string();
        let parse_result = toml::from_str::<Preferences>(&panel.raw_toml);
        assert!(parse_result.is_err());
    }
}
