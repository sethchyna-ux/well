//! well-config: Theia's Prism Immediate-Mode Configuration Dashboard
//!
//! Subsystems:
//! - TheiasPrismPanel: Comprehensive immediate-mode settings GUI built on egui for
//!   GPU-accelerated terminal configuration synchronized natively over Hermes Seqlock.
//! - Hyprlang: Native configuration parser and emitter for Hyprlang-syntax (.hl) configs.

mod credentials;
pub mod hyprlang;
pub use hyprlang::{HyprlangDocument, HyprlangEmitter, HyprlangParser};

use credentials::{
    resolve_credential, AiCredential, CredentialStore, OsCredentialStore, KEYCHAIN_SERVICE,
};
use egui::{pos2, Color32, Context, Rect, RichText, ScrollArea, Slider, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use well_core::ai::detect_destructive_command;
use well_ipc::{HermesChannel, TheiaConfigPayload};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShellAbbreviation {
    pub keyword: String,
    pub expansion: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeybindingRow {
    pub chord: String,
    pub action: String,
    pub conflict: bool,
}

/// Actions the desktop host can execute from a user-configured shortcut.
///
/// Keep this list deliberately small and tied to real desktop behavior. A
/// settings entry must not imply that arbitrary commands, panes, or plugins
/// can be bound when the host has no execution path for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    ToggleSettings,
    TogglePythia,
    OpenCommandPalette,
    SelectAllTerminalText,
    FindInScrollback,
    ClearBuffer,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    NewTab,
    CloseTab,
    SplitHorizontal,
    SplitVertical,
    NextTab,
    PrevTab,
    ToggleInlineEditor,
}

impl ShortcutAction {
    pub const ALL: [Self; 16] = [
        Self::ToggleSettings,
        Self::TogglePythia,
        Self::OpenCommandPalette,
        Self::SelectAllTerminalText,
        Self::FindInScrollback,
        Self::ClearBuffer,
        Self::IncreaseFontSize,
        Self::DecreaseFontSize,
        Self::ResetFontSize,
        Self::NewTab,
        Self::CloseTab,
        Self::SplitHorizontal,
        Self::SplitVertical,
        Self::NextTab,
        Self::PrevTab,
        Self::ToggleInlineEditor,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ToggleSettings => "Toggle Settings Menu",
            Self::TogglePythia => "Toggle Pythia AI Translator",
            Self::OpenCommandPalette => "Open Command Palette",
            Self::SelectAllTerminalText => "Select All Terminal Text",
            Self::FindInScrollback => "Find in Scrollback",
            Self::ClearBuffer => "Clear Buffer",
            Self::IncreaseFontSize => "Increase Font Size",
            Self::DecreaseFontSize => "Decrease Font Size",
            Self::ResetFontSize => "Reset Font Size",
            Self::NewTab => "New Tab",
            Self::CloseTab => "Close Tab or Pane",
            Self::SplitHorizontal => "Split Pane Horizontally",
            Self::SplitVertical => "Split Pane Vertically",
            Self::NextTab => "Next Tab",
            Self::PrevTab => "Previous Tab",
            Self::ToggleInlineEditor => "Toggle Mneme Inline Editor",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.label().eq_ignore_ascii_case(value.trim()))
    }
}

fn default_keybindings() -> Vec<KeybindingRow> {
    vec![
        KeybindingRow {
            chord: "Cmd+, / Ctrl+, / F1 / F12".to_string(),
            action: ShortcutAction::ToggleSettings.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+I / Ctrl+I".to_string(),
            action: ShortcutAction::TogglePythia.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+Shift+P / Cmd+P / Ctrl+Shift+P".to_string(),
            action: ShortcutAction::OpenCommandPalette.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+A / Ctrl+Shift+A".to_string(),
            action: ShortcutAction::SelectAllTerminalText.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+F / Ctrl+F".to_string(),
            action: ShortcutAction::FindInScrollback.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+K".to_string(),
            action: ShortcutAction::ClearBuffer.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+=".to_string(),
            action: ShortcutAction::IncreaseFontSize.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+-".to_string(),
            action: ShortcutAction::DecreaseFontSize.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+0".to_string(),
            action: ShortcutAction::ResetFontSize.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+T / Ctrl+Shift+T".to_string(),
            action: ShortcutAction::NewTab.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+W / Ctrl+Shift+W".to_string(),
            action: ShortcutAction::CloseTab.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+D".to_string(),
            action: ShortcutAction::SplitHorizontal.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+Shift+D".to_string(),
            action: ShortcutAction::SplitVertical.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+] / Ctrl+Tab".to_string(),
            action: ShortcutAction::NextTab.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Cmd+[ / Ctrl+Shift+Tab".to_string(),
            action: ShortcutAction::PrevTab.label().to_string(),
            conflict: false,
        },
        KeybindingRow {
            chord: "Ctrl+E / Cmd+E".to_string(),
            action: ShortcutAction::ToggleInlineEditor.label().to_string(),
            conflict: false,
        },
    ]
}

fn canonical_shortcut_alternatives(chord: &str) -> Option<Vec<String>> {
    let mut alternatives = Vec::new();

    for raw_alternative in chord.split('/') {
        let mut modifiers = Vec::new();
        let mut key = None;

        for raw_part in raw_alternative.split('+') {
            let part = raw_part.trim();
            if part.is_empty() {
                return None;
            }

            let modifier = match part.to_ascii_lowercase().as_str() {
                "cmd" | "command" | "super" | "meta" => Some("CMD"),
                "ctrl" | "control" => Some("CTRL"),
                "shift" => Some("SHIFT"),
                "alt" | "option" => Some("ALT"),
                _ => None,
            };

            if let Some(modifier) = modifier {
                if modifiers.contains(&modifier) {
                    return None;
                }
                modifiers.push(modifier);
            } else if key.replace(part.to_ascii_uppercase()).is_some() {
                return None;
            }
        }

        let key = key?;
        if !is_supported_shortcut_key(&key) {
            return None;
        }
        modifiers.sort_unstable();
        let mut normalized = modifiers
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        normalized.push(key);
        alternatives.push(normalized.join("+"));
    }

    alternatives.sort();
    alternatives.dedup();
    (!alternatives.is_empty()).then_some(alternatives)
}

fn is_supported_shortcut_key(key: &str) -> bool {
    let single_ascii_key = {
        let mut chars = key.chars();
        matches!(
            (chars.next(), chars.next()),
            (Some(character), None) if character.is_ascii_alphanumeric()
        )
    };

    single_ascii_key
        || matches!(key, "," | "=" | "+" | "-" | "[" | "]")
        || matches!(
            key,
            "F1" | "F2"
                | "F3"
                | "F4"
                | "F5"
                | "F6"
                | "F7"
                | "F8"
                | "F9"
                | "F10"
                | "F11"
                | "F12"
                | "ESC"
                | "ESCAPE"
                | "ENTER"
                | "RETURN"
                | "TAB"
                | "HOME"
                | "END"
                | "PGUP"
                | "PAGEUP"
                | "PGDN"
                | "PAGEDOWN"
        )
}

pub const PERSISTENT_CONFIG_VERSION: u32 = 2;
pub const CANONICAL_CONFIG_FORMAT: &str = "json";
pub const TOP_BAR_HEIGHT: f32 = 36.0;
pub const FRAME_MARGIN: f32 = 16.0;

#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowControlAction {
    None,
    Close,
    Minimize,
    ToggleFullscreen,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(skip)]
    pub timestamp: Option<Instant>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageArtifactRecord {
    pub path: String,
    pub prompt: String,
    pub provider: String,
    pub width: u32,
    pub height: u32,
    pub created_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ImageArtifactManifest {
    #[serde(default)]
    artifacts: Vec<ImageArtifactRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingImageGeneration {
    prompt: String,
    width: u32,
    height: u32,
    force_regeneration: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentConfig {
    #[serde(default = "current_config_version")]
    pub version: u32,
    pub theia: TheiaConfigPayload,
    pub shell_path: String,
    pub active_font_name: String,
    pub abbreviations: Vec<ShellAbbreviation>,
    pub keybindings: Vec<KeybindingRow>,
    #[serde(default, skip_serializing)]
    pub hf_token: Option<String>,
}

fn current_config_version() -> u32 {
    PERSISTENT_CONFIG_VERSION
}

impl PersistentConfig {
    pub fn from_panel(panel: &TheiasPrismPanel) -> Self {
        let mut theia = panel.config.clone();
        theia.hf_token.clear();
        theia.gemini_api_key.clear();

        Self {
            version: PERSISTENT_CONFIG_VERSION,
            theia,
            shell_path: panel.shell_path.clone(),
            active_font_name: panel.active_font_name.clone(),
            abbreviations: panel.abbreviations.clone(),
            keybindings: panel.keybindings.clone(),
            hf_token: None,
        }
    }

    pub fn from_json_str(data: &str) -> Result<Self, String> {
        let mut value = serde_json::from_str::<serde_json::Value>(data)
            .map_err(|error| format!("Failed to parse config: {error}"))?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "Failed to parse config: root must be a JSON object".to_string())?;
        let source_version = match object.get("version") {
            Some(version) => version.as_u64().ok_or_else(|| {
                "Failed to parse config: version must be a non-negative integer".to_string()
            })?,
            None => 0,
        };
        if source_version > u64::from(PERSISTENT_CONFIG_VERSION) {
            return Err(format!(
                "Config version {source_version} is newer than supported version {PERSISTENT_CONFIG_VERSION}"
            ));
        }

        object.insert(
            "version".to_string(),
            serde_json::Value::from(PERSISTENT_CONFIG_VERSION),
        );
        let mut config = serde_json::from_value::<Self>(value)
            .map_err(|error| format!("Failed to parse config: {error}"))?;
        config.normalize();
        Ok(config)
    }

    pub fn to_json_string(&self) -> Result<String, String> {
        let mut config = self.clone();
        config.normalize();
        config.theia.hf_token.clear();
        config.theia.gemini_api_key.clear();
        config.hf_token = None;
        serde_json::to_string_pretty(&config)
            .map_err(|error| format!("Failed to serialize config: {error}"))
    }

    fn normalize(&mut self) {
        self.version = PERSISTENT_CONFIG_VERSION;
        let defaults = TheiaConfigPayload::default();

        self.theia.theme_id = self.theia.theme_id.min(3);
        self.theia.background_opacity = self.theia.background_opacity.clamp(0.05, 1.0);
        self.theia.glass_blur_radius = self.theia.glass_blur_radius.clamp(0.0, 64.0);
        self.theia.scan_timeout_ms = self.theia.scan_timeout_ms.clamp(1, 60_000);
        self.theia.command_timeout_ms = self.theia.command_timeout_ms.clamp(50, 600_000);
        self.theia.scrollback_limit = self.theia.scrollback_limit.clamp(100, 1_000_000);
        self.theia.font_size = self.theia.font_size.clamp(8.0, 72.0);
        self.theia.cursor_style = self.theia.cursor_style.min(2);
        self.theia.screen_curvature = self.theia.screen_curvature.clamp(0.0, 0.5);
        self.theia.scanline_frequency = self.theia.scanline_frequency.clamp(0.0, 1_000.0);
        self.theia.glow_radius = self.theia.glow_radius.clamp(0.0, 10.0);
        self.theia.line_height = self.theia.line_height.clamp(0.8, 3.0);

        if self.shell_path.trim().is_empty() {
            self.shell_path = if self.theia.shell_path.trim().is_empty() {
                defaults.shell_path.clone()
            } else {
                self.theia.shell_path.clone()
            };
        }
        self.theia.shell_path = self.shell_path.clone();
        if self.active_font_name.trim().is_empty() {
            self.active_font_name = defaults.font_name.clone();
        }
        if self.theia.font_name.trim().is_empty() {
            self.theia.font_name = defaults.font_name;
        }
        if self.theia.prompt_preset.trim().is_empty() {
            self.theia.prompt_preset = defaults.prompt_preset;
        }
        if self.theia.pythia_provider.trim().is_empty() {
            self.theia.pythia_provider = defaults.pythia_provider;
        }
        if self.theia.hf_model.trim().is_empty() {
            self.theia.hf_model = defaults.hf_model;
        }
        if self.theia.gemini_model.trim().is_empty() {
            self.theia.gemini_model = defaults.gemini_model;
        }
        if self.theia.ollama_url.trim().is_empty() {
            self.theia.ollama_url = defaults.ollama_url;
        }
        if self.theia.ollama_model.trim().is_empty() {
            self.theia.ollama_model = defaults.ollama_model;
        }
        if self
            .theia
            .profile_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            self.theia.profile_name = None;
        }
    }
}

impl Default for PersistentConfig {
    fn default() -> Self {
        let theia = TheiaConfigPayload::default();
        Self {
            version: PERSISTENT_CONFIG_VERSION,
            shell_path: theia.shell_path.clone(),
            active_font_name: theia.font_name.clone(),
            theia,
            abbreviations: Vec::new(),
            keybindings: Vec::new(),
            hf_token: None,
        }
    }
}

/// Loads a persisted Well configuration from an explicit path.
///
/// A missing configuration is a normal first-run condition. Invalid or unreadable
/// configurations are returned to the caller so startup can keep the terminal
/// usable while surfacing a clear diagnostic to the user.
pub fn load_persistent_config_from_path(path: &Path) -> Result<Option<PersistentConfig>, String> {
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read {}: {error}", path.display())),
    };

    PersistentConfig::from_json_str(&data)
        .map(Some)
        .map_err(|error| format!("{} ({})", error, path.display()))
}

/// Loads the user's persisted Well configuration, if one exists.
pub fn load_persistent_config() -> Result<Option<PersistentConfig>, String> {
    let path = TheiasPrismPanel::config_path().ok_or("Could not determine HOME directory")?;
    load_persistent_config_from_path(&path)
}

fn write_config_atomically(path: &Path, contents: &str) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Invalid config path {}", path.display()))?;
    let temporary_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary_path)
            .map_err(|error| format!("Failed to write {}: {error}", temporary_path.display()))?;
        file.write_all(contents.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Failed to write {}: {error}", temporary_path.display()))?;
        std::fs::rename(&temporary_path, path).map_err(|error| {
            format!(
                "Failed to replace config {} atomically: {error}",
                path.display()
            )
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result
}

fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty()
        || name.trim() != name
        || name.len() > 80
        || matches!(name, "." | "..")
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ' ')
        })
    {
        return Err(
            "Profile name must be a single non-empty file name using letters, numbers, spaces, '-' or '_'"
                .to_string(),
        );
    }
    Ok(())
}

fn profile_dir() -> Result<PathBuf, String> {
    let config_path = TheiasPrismPanel::config_path()
        .ok_or_else(|| "Could not determine config directory".to_string())?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| format!("Invalid config path {}", config_path.display()))?;
    Ok(config_dir.join("profiles"))
}

fn profile_path(name: &str) -> Result<PathBuf, String> {
    validate_profile_name(name)?;
    Ok(profile_dir()?.join(format!("{name}.json")))
}

fn profile_json_from_panel(panel: &TheiasPrismPanel, name: &str) -> Result<String, String> {
    validate_profile_name(name)?;
    let mut persistent = PersistentConfig::from_panel(panel);
    persistent.theia.profile_name = Some(name.to_string());
    persistent.normalize();
    persistent.to_json_string()
}

fn profile_config_from_json(
    data: &str,
    name: &str,
    current_hf_token: &str,
    current_gemini_api_key: &str,
) -> Result<PersistentConfig, String> {
    validate_profile_name(name)?;
    let value = serde_json::from_str::<serde_json::Value>(data)
        .map_err(|error| format!("Failed to parse profile: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "Failed to parse profile: root must be a JSON object".to_string())?;

    let mut persistent = if object.contains_key("theia")
        || object.contains_key("version")
        || object.contains_key("keybindings")
        || object.contains_key("abbreviations")
    {
        PersistentConfig::from_json_str(data)
    } else {
        let mut payload = serde_json::from_value::<TheiaConfigPayload>(value)
            .map_err(|error| format!("Failed to parse legacy profile: {error}"))?;
        payload.profile_name = Some(name.to_string());
        let mut persistent = PersistentConfig {
            shell_path: payload.shell_path.clone(),
            active_font_name: payload.font_name.clone(),
            theia: payload,
            ..PersistentConfig::default()
        };
        persistent.normalize();
        Ok(persistent)
    }?;

    persistent.theia.profile_name = Some(name.to_string());
    persistent.theia.hf_token = current_hf_token.to_string();
    persistent.theia.gemini_api_key = current_gemini_api_key.to_string();
    persistent.hf_token = None;
    persistent.normalize();
    Ok(persistent)
}

fn generated_image_cache_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "Could not determine home directory for generated images".to_string())?;
    let cache_dir = PathBuf::from(home).join(".well").join("generated");
    std::fs::create_dir_all(&cache_dir).map_err(|error| {
        format!(
            "Failed to create generated-image cache {}: {error}",
            cache_dir.display()
        )
    })?;
    Ok(cache_dir)
}

fn image_manifest_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("manifest.json")
}

fn load_image_artifacts_from_dir(cache_dir: &Path) -> Vec<ImageArtifactRecord> {
    let path = image_manifest_path(cache_dir);
    let Ok(data) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(mut manifest) = serde_json::from_str::<ImageArtifactManifest>(&data) else {
        return Vec::new();
    };

    manifest
        .artifacts
        .retain(|artifact| Path::new(&artifact.path).is_file());
    manifest
        .artifacts
        .sort_by_key(|artifact| std::cmp::Reverse(artifact.created_at_unix));
    manifest.artifacts.truncate(50);
    manifest.artifacts
}

fn save_image_artifacts_to_dir(
    cache_dir: &Path,
    artifacts: &[ImageArtifactRecord],
) -> Result<(), String> {
    let manifest = ImageArtifactManifest {
        artifacts: artifacts.to_vec(),
    };
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("Failed to serialize image manifest: {error}"))?;
    write_config_atomically(&image_manifest_path(cache_dir), &json)
}

fn image_artifact_is_inside_cache(cache_dir: &Path, artifact_path: &Path) -> bool {
    let Ok(cache_dir) = cache_dir.canonicalize() else {
        return false;
    };
    let Ok(artifact_path) = artifact_path.canonicalize() else {
        return false;
    };
    artifact_path.starts_with(cache_dir)
}

fn image_generation_supported_by_provider(provider: &str, config: &TheiaConfigPayload) -> bool {
    match provider {
        "Google Gemini 2.0" => {
            !config.gemini_api_key.trim().is_empty()
                || std::env::var("GEMINI_API_KEY")
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
        }
        "Gemma Abliterated (Hugging Face)" => {
            !config.hf_token.trim().is_empty()
                || std::env::var("HF_TOKEN")
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
        }
        _ => false,
    }
}

fn image_provider_status(provider: &str, config: &TheiaConfigPayload) -> (&'static str, Color32) {
    match provider {
        "Google Gemini 2.0" | "Gemma Abliterated (Hugging Face)"
            if image_generation_supported_by_provider(provider, config) =>
        {
            ("ready", Color32::from_rgb(57, 255, 20))
        }
        "Google Gemini 2.0" => ("Gemini key required", Color32::from_rgb(250, 204, 21)),
        "Gemma Abliterated (Hugging Face)" => (
            "Hugging Face token required",
            Color32::from_rgb(250, 204, 21),
        ),
        "Ollama (Local)" => (
            "Ollama image backend not available",
            Color32::from_rgb(248, 113, 113),
        ),
        _ => (
            "select Gemini or Hugging Face",
            Color32::from_rgb(248, 113, 113),
        ),
    }
}

pub struct TheiasPrismPanel {
    pub channel: Arc<HermesChannel<TheiaConfigPayload>>,
    pub config: TheiaConfigPayload,
    pub shell_path: String,
    pub active_font_name: String,
    pub abbreviations: Vec<ShellAbbreviation>,
    pub new_keyword: String,
    pub new_expansion: String,
    pub keybindings: Vec<KeybindingRow>,
    pub new_chord: String,
    pub new_action: String,
    pub active_tab: usize,
    pub is_open: bool,
    pub status_notification: Option<(String, Instant)>,
    pub pending_window_action: WindowControlAction,
    // Undo/Redo stacks for config snapshots
    pub undo_stack: Vec<TheiaConfigPayload>,
    pub redo_stack: Vec<TheiaConfigPayload>,
    // Current profile name (if any)
    pub current_profile: Option<String>,
    pub new_profile_name: String,
    // Pythia LLM Translation HUD state
    pub show_pythia_hud: bool,
    pub pythia_query: String,
    pub pythia_result: Option<well_llm::TranslationResult>,
    pub image_generation_in_flight: bool,
    image_generation_result_rx: Option<std::sync::mpsc::Receiver<well_llm::TranslationResult>>,
    image_generation_pending: Option<PendingImageGeneration>,
    pub image_artifacts: Vec<ImageArtifactRecord>,
    pub pythia_mode: u8, // 0 = Natural Language -> Shell, 1 = Error Diagnosis
    pub pythia_image_dim: (u32, u32),
    pub pythia_destructive_confirmation: String,
    pub pending_pty_injection: Option<String>,
    // URI of the last displayed image — we call ctx.forget_image() on it
    // before loading a new one to bypass egui's texture cache.
    pub last_image_uri: Option<String>,
    // Hugging Face token (optional, overrides env var if set)
    pub hf_token: String,
    // AI Agent Lifecycle Events for BlockList
    pub agent_events: Vec<AgentEvent>,
}

impl TheiasPrismPanel {
    pub fn new(channel: Arc<HermesChannel<TheiaConfigPayload>>) -> Self {
        let initial_config = channel.read_state();
        let initial_abbrevs = vec![
            ShellAbbreviation {
                keyword: "gco".to_string(),
                expansion: "git checkout".to_string(),
            },
            ShellAbbreviation {
                keyword: "lg".to_string(),
                expansion: "lazygit".to_string(),
            },
            ShellAbbreviation {
                keyword: "hx".to_string(),
                expansion: "helix".to_string(),
            },
            ShellAbbreviation {
                keyword: "gp".to_string(),
                expansion: "git push".to_string(),
            },
            ShellAbbreviation {
                keyword: "ll".to_string(),
                expansion: "ls -la".to_string(),
            },
        ];

        let initial_bindings = default_keybindings();

        let shell_path = initial_config.shell_path.clone();

        let active_font_name = if initial_config.font_name == "System Monospace" {
            "JetBrainsMono Nerd Font (Active)".to_string()
        } else {
            "OpenDyslexic Nerd Font (Active)".to_string()
        };

        let mut panel = Self {
            channel,
            config: initial_config,
            shell_path,
            active_font_name,
            abbreviations: initial_abbrevs,
            new_keyword: String::new(),
            new_expansion: String::new(),
            keybindings: initial_bindings,
            new_chord: String::new(),
            new_action: ShortcutAction::OpenCommandPalette.label().to_string(),
            active_tab: 0,
            is_open: false, // Closed by default on launch for clean workspace
            status_notification: None,
            pending_window_action: WindowControlAction::None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_profile: None,
            new_profile_name: String::new(),
            show_pythia_hud: false,
            pythia_query: String::new(),
            pythia_result: None,
            image_generation_in_flight: false,
            image_generation_result_rx: None,
            image_generation_pending: None,
            image_artifacts: generated_image_cache_dir()
                .map(|dir| load_image_artifacts_from_dir(&dir))
                .unwrap_or_default(),
            pythia_mode: 0,
            pythia_image_dim: (1024, 1024),
            pythia_destructive_confirmation: String::new(),
            pending_pty_injection: None,
            last_image_uri: None,
            // Initialize HF token from environment variable if present
            hf_token: std::env::var("HF_TOKEN").unwrap_or_default(),
            agent_events: Vec::new(),
        };

        // Attempt to load saved config from disk if available
        panel.try_load_from_disk();
        panel.refresh_keybinding_conflicts();
        #[cfg(not(test))]
        panel.load_runtime_credentials();
        panel
    }

    pub fn set_notification(&mut self, text: impl Into<String>) {
        self.status_notification = Some((text.into(), Instant::now()));
    }

    fn sync_public_config(&self) {
        let mut public_config = self.config.clone();
        public_config.hf_token.clear();
        public_config.gemini_api_key.clear();
        self.channel.sync_state(public_config);
    }

    #[cfg_attr(test, allow(dead_code))]
    fn load_runtime_credentials(&mut self) {
        let store = OsCredentialStore;
        let credentials = [
            (AiCredential::HuggingFaceToken, self.config.hf_token.clone()),
            (
                AiCredential::GeminiApiKey,
                self.config.gemini_api_key.clone(),
            ),
        ];

        for (credential, legacy_value) in credentials {
            let environment_value = std::env::var(credential.environment_variable()).ok();
            match resolve_credential(credential, environment_value, Some(legacy_value), &store) {
                Ok(resolved) => {
                    let value = resolved.value.unwrap_or_default();
                    match credential {
                        AiCredential::HuggingFaceToken => {
                            self.config.hf_token = value.clone();
                            self.hf_token = value;
                        }
                        AiCredential::GeminiApiKey => self.config.gemini_api_key = value,
                    }
                }
                Err(error) => {
                    if std::env::var_os("WELL_LOG_CREDENTIAL_ERRORS").is_some() {
                        eprintln!("[Well] {error}");
                    }
                }
            }
        }
    }

    fn save_ai_credential(&mut self, credential: AiCredential) {
        let value = match credential {
            AiCredential::GeminiApiKey => self.config.gemini_api_key.trim(),
            AiCredential::HuggingFaceToken => self.config.hf_token.trim(),
        };
        let store = OsCredentialStore;
        match store.save(credential, value) {
            Ok(()) => self.set_notification(format!(
                "Saved {} securely in OS credential storage ({KEYCHAIN_SERVICE})",
                credential.account()
            )),
            Err(error) => self.set_notification(error),
        }
    }

    fn remove_ai_credential(&mut self, credential: AiCredential) {
        let store = OsCredentialStore;
        match store.delete(credential) {
            Ok(()) => {
                let environment_value = std::env::var(credential.environment_variable())
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_default();
                match credential {
                    AiCredential::GeminiApiKey => self.config.gemini_api_key = environment_value,
                    AiCredential::HuggingFaceToken => {
                        self.config.hf_token = environment_value.clone();
                        self.hf_token = environment_value;
                    }
                }
                self.set_notification(format!(
                    "Removed {} from OS credential storage",
                    credential.account()
                ));
            }
            Err(error) => self.set_notification(error),
        }
    }

    pub fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("well")
                .join("config.json"),
        )
    }

    pub fn hyprlang_path() -> Option<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("well")
                .join("well.hl"),
        )
    }

    pub fn save_to_disk(&mut self) -> Result<(), String> {
        let path = Self::config_path().ok_or("Could not determine HOME directory")?;
        self.save_to_path(&path)
    }

    fn save_to_path(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create config directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }

        let persistent = PersistentConfig::from_panel(self);
        let json = persistent.to_json_string()?;

        write_config_atomically(path, &json)?;

        self.sync_public_config();
        self.status_notification = Some((format!("Saved to {}", path.display()), Instant::now()));
        Ok(())
    }

    pub fn try_load_from_disk(&mut self) {
        if let Err(error) = self.load_from_disk() {
            self.set_notification(format!("Config invalid; using safe defaults: {error}"));
        }
    }

    pub fn load_from_disk(&mut self) -> Result<bool, String> {
        let path = Self::config_path().ok_or("Could not determine HOME directory")?;
        self.load_from_path(&path)
    }

    fn load_from_path(&mut self, path: &Path) -> Result<bool, String> {
        let Some(persistent) = load_persistent_config_from_path(path)? else {
            return Ok(false);
        };

        self.apply_persistent_config(persistent);
        Ok(true)
    }

    fn apply_persistent_config(&mut self, persistent: PersistentConfig) {
        let mut config = persistent.theia;
        let hf_token = if config.hf_token.trim().is_empty() {
            persistent
                .hf_token
                .unwrap_or_else(|| std::env::var("HF_TOKEN").unwrap_or_default())
        } else {
            config.hf_token.clone()
        };
        config.hf_token = hf_token.clone();

        self.config = config;
        self.shell_path = persistent.shell_path;
        self.active_font_name = persistent.active_font_name;
        self.abbreviations = persistent.abbreviations;
        self.keybindings = persistent.keybindings;
        self.refresh_keybinding_conflicts();
        self.hf_token = hf_token;
        self.current_profile = self.config.profile_name.clone();
        self.sync_public_config();
    }

    fn refresh_keybinding_conflicts(&mut self) {
        let mut assigned_chords = HashSet::new();

        for binding in &mut self.keybindings {
            let Some(alternatives) = canonical_shortcut_alternatives(&binding.chord) else {
                binding.conflict = true;
                continue;
            };

            let supported_action = ShortcutAction::from_label(&binding.action).is_some();
            let overlaps_existing = alternatives
                .iter()
                .any(|alternative| assigned_chords.contains(alternative));

            binding.conflict = !supported_action || overlaps_existing;
            if !binding.conflict {
                assigned_chords.extend(alternatives);
            }
        }
    }

    fn import_hyprlang_from_path(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
        let document = HyprlangParser::parse(&content).map_err(|error| error.to_string())?;
        let mut imported = HyprlangParser::to_theia_config(&document);
        imported.hf_token = self.config.hf_token.clone();
        imported.gemini_api_key = self.config.gemini_api_key.clone();
        let mut persistent = PersistentConfig::from_panel(self);
        persistent.shell_path = imported.shell_path.clone();
        persistent.theia = imported;
        persistent.keybindings = HyprlangParser::to_keybinding_rows(&document);
        persistent.normalize();
        self.apply_persistent_config(persistent);
        self.status_notification =
            Some((format!("Imported from {}", path.display()), Instant::now()));
        Ok(())
    }

    fn export_hyprlang_to_path(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create config directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let hyprlang = HyprlangEmitter::emit(&self.config, &self.keybindings);
        write_config_atomically(path, &hyprlang)?;
        self.status_notification =
            Some((format!("Exported to {}", path.display()), Instant::now()));
        Ok(())
    }

    pub fn reset_to_defaults(&mut self) {
        self.config = TheiaConfigPayload::default();
        self.active_font_name = "OpenDyslexic Nerd Font (Active)".to_string();
        self.keybindings = default_keybindings();
        self.refresh_keybinding_conflicts();
        self.sync_public_config();
        self.status_notification = Some(("Reset to factory defaults".to_string(), Instant::now()));
        // Clear undo/redo history on reset
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_profile = None;
        self.new_profile_name.clear();
    }

    pub fn render_window(&mut self, ctx: &Context) {
        // 1. Configure dark glassmorphic styling on egui context
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgba_premultiplied(13, 16, 24, 242);
        visuals.window_stroke =
            Stroke::new(1.0f32, Color32::from_rgba_premultiplied(56, 189, 248, 80));
        visuals.window_rounding = egui::Rounding::same(14.0);
        visuals.window_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 24.0,
            spread: 0.0,
            color: Color32::from_black_alpha(220),
        };
        visuals.panel_fill = Color32::from_rgba_premultiplied(18, 22, 32, 245);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgba_premultiplied(22, 28, 40, 190);
        visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(26, 33, 48, 200);
        visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(38, 48, 70, 255);
        visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20));
        visuals.widgets.active.bg_fill = Color32::from_rgba_premultiplied(48, 62, 88, 255);
        visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(57, 255, 20, 50);
        visuals.selection.stroke = Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20));
        ctx.set_visuals(visuals);

        self.render_agent_sidebar(ctx);

        let mut toggle_clicked = false;
        let screen_rect = ctx.screen_rect();
        let screen_w = screen_rect.width();
        let _screen_h = screen_rect.height();
        let top_bar_h = TOP_BAR_HEIGHT;
        let margin = FRAME_MARGIN;

        // 1. Draw Oroboros scale frame around the terminal viewport
        let bg_painter = ctx.layer_painter(egui::LayerId::background());
        draw_oroboros_scale_frame(&bg_painter, screen_rect, top_bar_h, margin);

        // 2. Defined Full-Width Top Bar
        let top_bar_rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(screen_w, top_bar_h));

        // Dark obsidian header fill with subtle top rounding
        bg_painter.rect_filled(
            top_bar_rect,
            egui::Rounding {
                nw: 10.0,
                ne: 10.0,
                sw: 0.0,
                se: 0.0,
            },
            Color32::from_rgba_premultiplied(12, 16, 24, 252),
        );

        // Defined bottom divider line separating top bar from terminal
        bg_painter.line_segment(
            [pos2(0.0, top_bar_h), pos2(screen_w, top_bar_h)],
            Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 18)),
        );

        // Central glowing cyber accent seam (electric violet blending into neon cyan)
        let cx = screen_w * 0.5;
        let seam_half_w = (screen_w * 0.16).clamp(50.0, 140.0);
        bg_painter.line_segment(
            [pos2(cx - seam_half_w, top_bar_h), pos2(cx, top_bar_h)],
            Stroke::new(1.5_f32, Color32::from_rgba_premultiplied(168, 85, 247, 190)),
        );
        bg_painter.line_segment(
            [pos2(cx, top_bar_h), pos2(cx + seam_half_w, top_bar_h)],
            Stroke::new(1.5_f32, Color32::from_rgba_premultiplied(56, 189, 248, 190)),
        );

        // Top bar interactive UI layer: Single unified area across top
        egui::Area::new(egui::Id::new("top_bar_unified_area"))
            .fixed_pos(pos2(12.0, 5.0))
            .show(ctx, |ui| {
                ui.set_width(screen_w - 24.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                    ui.label(
                        RichText::new("⟳")
                            .size(15.0)
                            .color(Color32::from_rgb(192, 132, 252))
                            .strong(),
                    );
                    ui.label(
                        RichText::new("WELL")
                            .size(13.0)
                            .color(Color32::from_rgb(240, 245, 255))
                            .strong(),
                    );
                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(22, 28, 42, 180))
                        .stroke(Stroke::new(
                            0.8_f32,
                            Color32::from_rgba_premultiplied(56, 189, 248, 80),
                        ))
                        .rounding(egui::Rounding::same(4.0))
                        .inner_margin(egui::Margin::symmetric(5.0, 1.5))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("SHELL")
                                    .size(9.5)
                                    .color(Color32::from_rgb(140, 165, 195))
                                    .strong(),
                            );
                        });

                    let ai_btn = egui::Button::new(
                        RichText::new("✦ Pythia AI")
                            .size(10.0)
                            .color(Color32::from_rgb(192, 132, 252))
                            .strong(),
                    )
                    .fill(Color32::from_rgba_premultiplied(28, 18, 44, 200))
                    .stroke(Stroke::new(
                        1.0_f32,
                        Color32::from_rgba_premultiplied(168, 85, 247, 120),
                    ))
                    .rounding(egui::Rounding::same(4.0));
                    if ui
                        .add(ai_btn)
                        .on_hover_text("Open Pythia Shell Copilot (Cmd+I)")
                        .clicked()
                    {
                        self.show_pythia_hud = !self.show_pythia_hud;
                    }

                    let img_btn = egui::Button::new(
                        RichText::new("🖼 Image Gen")
                            .size(10.5)
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong(),
                    )
                    .fill(Color32::from_rgba_premultiplied(16, 36, 60, 230))
                    .stroke(Stroke::new(1.2_f32, Color32::from_rgb(56, 189, 248)))
                    .rounding(egui::Rounding::same(6.0));
                    if ui
                        .add(img_btn)
                        .on_hover_text("Open Cybernetic Image Studio (Cmd+I)")
                        .clicked()
                    {
                        self.show_pythia_hud = true;
                        if self.pythia_query.trim().is_empty() {
                            self.pythia_query =
                                "/image synthwave neon grid wireframe sunset".to_string();
                        }
                    }

                    // Right-aligned controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (btn_text, text_color, border_color) = if self.is_open {
                            (
                                "✕ Close (Esc)",
                                Color32::from_rgb(255, 100, 130),
                                Color32::from_rgba_premultiplied(255, 80, 120, 130),
                            )
                        } else {
                            (
                                "⚙ Settings",
                                Color32::from_rgb(180, 205, 230),
                                Color32::from_rgba_premultiplied(56, 189, 248, 80),
                            )
                        };
                        let btn = egui::Button::new(
                            RichText::new(btn_text)
                                .size(11.0)
                                .strong()
                                .color(text_color),
                        )
                        .fill(Color32::from_rgba_premultiplied(16, 21, 32, 220))
                        .stroke(Stroke::new(1.0f32, border_color))
                        .rounding(egui::Rounding::same(8.0));
                        if ui.add(btn).clicked() {
                            toggle_clicked = true;
                        }

                        ui.label(
                            RichText::new("UTF-8")
                                .size(9.5)
                                .color(Color32::from_rgb(100, 120, 150)),
                        );
                        ui.label(
                            RichText::new("●")
                                .size(8.0)
                                .color(Color32::from_rgb(57, 255, 20)),
                        );
                    });
                });
            });

        // Keep window actions centered and independent from variable-width status controls.
        egui::Area::new(egui::Id::new("top_bar_window_controls"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 6.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(14, 18, 28, 220))
                    .stroke(Stroke::new(
                        0.8_f32,
                        Color32::from_rgba_premultiplied(255, 255, 255, 24),
                    ))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::symmetric(3.0, 1.5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                            let minimize = egui::Button::new(
                                RichText::new("─")
                                    .color(Color32::from_rgb(56, 189, 248))
                                    .size(10.0),
                            )
                            .min_size(egui::vec2(22.0, 17.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(egui::Rounding::same(4.0));
                            if ui.add(minimize).on_hover_text("Minimize (Cmd+M)").clicked() {
                                self.pending_window_action = WindowControlAction::Minimize;
                            }

                            let fullscreen = egui::Button::new(
                                RichText::new("□")
                                    .color(Color32::from_rgb(192, 132, 252))
                                    .size(10.0),
                            )
                            .min_size(egui::vec2(22.0, 17.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(egui::Rounding::same(4.0));
                            if ui
                                .add(fullscreen)
                                .on_hover_text("Toggle Fullscreen (Ctrl+Cmd+F)")
                                .clicked()
                            {
                                self.pending_window_action = WindowControlAction::ToggleFullscreen;
                            }

                            let close = egui::Button::new(
                                RichText::new("✕")
                                    .color(Color32::from_rgb(248, 113, 113))
                                    .size(10.0),
                            )
                            .min_size(egui::vec2(22.0, 17.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(egui::Rounding::same(4.0));
                            if ui.add(close).on_hover_text("Close Well (Cmd+Q)").clicked() {
                                self.pending_window_action = WindowControlAction::Close;
                            }
                        });
                    });
            });

        if toggle_clicked {
            self.is_open = !self.is_open;
        }

        if !self.is_open {
            self.render_pythia_hud(ctx);
            return;
        }

        let mut is_open = self.is_open;
        let mut close_clicked = false;
        let mut apply_clicked = false;
        let mut save_clicked = false;
        let mut reset_clicked = false;
        let mut undo_clicked = false;
        let mut redo_clicked = false;

        egui::Window::new(
            RichText::new("WELL // CONTROL CENTER")
                .color(Color32::from_rgb(34, 180, 50))
                .strong()
                .size(13.5),
        )
        .id(egui::Id::new("theia_control_center_window"))
        .open(&mut is_open)
        .default_width(760.0)
        .default_height(580.0)
        .min_width(620.0)
        .min_height(440.0)
        .resizable(true)
        .movable(true)
        .collapsible(false)
        .default_pos(egui::pos2(100.0, 50.0))
        .show(ctx, |ui| {
            // Sleek Two-Column Category Rail Navigation (No Tabs)
            let sidebar_w = 170.0;
            let available_h = ui.available_height() - 48.0;

            ui.horizontal(|ui| {
                // Left Column: Category Sidebar Rail
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_w, available_h),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let categories: [(usize, &str, Color32); 8] = [
                            (0, "🎨  Appearance", Color32::from_rgb(57, 255, 20)),
                            (1, "🔤  Typography", Color32::from_rgb(56, 189, 248)),
                            (2, "⌨  Shortcuts", Color32::from_rgb(250, 204, 21)),
                            (3, "🐚  Shell", Color32::from_rgb(255, 0, 127)),
                            (5, "📑  Profiles & HL", Color32::from_rgb(0, 240, 255)),
                            (7, "📖  Help", Color32::from_rgb(148, 163, 184)),
                            (10, "✦  Pythia AI", Color32::from_rgb(192, 132, 252)),
                            (11, "🖼  Image Studio", Color32::from_rgb(56, 189, 248)),
                        ];

                        ScrollArea::vertical()
                            .id_salt("theia_category_sidebar")
                            .auto_shrink([false, false])
                            .max_width(sidebar_w)
                            .show(ui, |ui| {
                                for (idx, label, accent) in categories {
                                    let is_active = self.active_tab == idx;
                                    let (fill, stroke, text_color) = if is_active {
                                        (
                                            Color32::from_rgba_premultiplied(
                                                accent.r() / 6,
                                                accent.g() / 6,
                                                accent.b() / 6,
                                                230,
                                            ),
                                            Stroke::new(1.2f32, accent),
                                            accent,
                                        )
                                    } else {
                                        (
                                            Color32::from_rgba_premultiplied(16, 22, 34, 150),
                                            Stroke::new(
                                                1.0f32,
                                                Color32::from_rgba_premultiplied(255, 255, 255, 15),
                                            ),
                                            Color32::from_rgb(148, 163, 184),
                                        )
                                    };

                                    let btn = egui::Button::new(
                                        RichText::new(label).size(11.5).strong().color(text_color),
                                    )
                                    .fill(fill)
                                    .stroke(stroke)
                                    .rounding(egui::Rounding::same(6.0));

                                    if ui.add_sized([sidebar_w - 8.0, 30.0], btn).clicked() {
                                        self.active_tab = idx;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                    },
                );

                ui.separator();

                // Right Column: Category Content View
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), available_h),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ScrollArea::vertical()
                            .id_salt("theia_category_content_area")
                            .auto_shrink([false, false])
                            .show(ui, |ui| match self.active_tab {
                                0 => self.render_appearance_tab(ui),
                                1 => self.render_typography_tab(ui),
                                2 => self.render_shortcuts_tab(ui),
                                3 => self.render_shell_tab(ui),
                                5 => self.render_profiles_tab(ui),
                                7 => self.render_help_tab(ui),
                                10 => self.render_pythia_tab(ui),
                                11 => self.render_image_studio_tab(ui),
                                _ => self.render_appearance_tab(ui),
                            });
                    },
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Bottom Action & Telemetry Bar
            ui.horizontal(|ui| {
                let apply_btn = egui::Button::new(
                    RichText::new("⚡ Apply Live")
                        .color(Color32::from_rgb(57, 255, 20))
                        .strong(),
                )
                .fill(Color32::from_rgba_premultiplied(20, 50, 30, 220))
                .stroke(Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20)))
                .rounding(egui::Rounding::same(6.0));
                if ui.add(apply_btn).clicked() {
                    apply_clicked = true;
                }

                let save_btn = egui::Button::new(
                    RichText::new("💾 Save Config").color(Color32::from_rgb(56, 189, 248)),
                )
                .fill(Color32::from_rgba_premultiplied(20, 40, 60, 220))
                .stroke(Stroke::new(1.0f32, Color32::from_rgb(56, 189, 248)))
                .rounding(egui::Rounding::same(6.0));
                if ui.add(save_btn).clicked() {
                    save_clicked = true;
                }

                let can_undo = !self.undo_stack.is_empty();
                let can_redo = !self.redo_stack.is_empty();
                ui.add_enabled_ui(can_undo, |ui| {
                    if ui.button("↶ Undo").clicked() {
                        undo_clicked = true;
                    }
                });
                ui.add_enabled_ui(can_redo, |ui| {
                    if ui.button("↷ Redo").clicked() {
                        redo_clicked = true;
                    }
                });

                let reset_btn = egui::Button::new(
                    RichText::new("↺ Reset").color(Color32::from_rgb(255, 100, 130)),
                )
                .fill(Color32::from_rgba_premultiplied(40, 20, 30, 200))
                .stroke(Stroke::new(
                    1.0f32,
                    Color32::from_rgba_premultiplied(255, 100, 130, 120),
                ))
                .rounding(egui::Rounding::same(6.0));
                if ui.add(reset_btn).clicked() {
                    reset_clicked = true;
                }

                let studio_btn = egui::Button::new(
                    RichText::new("🖼 Image Studio")
                        .color(Color32::from_rgb(56, 189, 248))
                        .strong(),
                )
                .fill(Color32::from_rgba_premultiplied(16, 32, 54, 220))
                .stroke(Stroke::new(
                    1.0f32,
                    Color32::from_rgba_premultiplied(56, 189, 248, 160),
                ))
                .rounding(egui::Rounding::same(6.0));
                if ui
                    .add(studio_btn)
                    .on_hover_text("Jump to Cybernetic Image Studio Tab")
                    .clicked()
                {
                    self.active_tab = 11;
                }

                let close_btn = egui::Button::new(
                    RichText::new("✕ Close (Esc)")
                        .color(Color32::from_rgb(255, 120, 140))
                        .strong(),
                )
                .fill(Color32::from_rgba_premultiplied(45, 20, 30, 220))
                .stroke(Stroke::new(
                    1.0f32,
                    Color32::from_rgba_premultiplied(255, 120, 140, 150),
                ))
                .rounding(egui::Rounding::same(6.0));
                if ui.add(close_btn).clicked() {
                    close_clicked = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, created)) = &self.status_notification {
                        if created.elapsed().as_secs() < 4 {
                            ui.label(
                                RichText::new(format!("● {}", msg))
                                    .color(Color32::from_rgb(57, 255, 20))
                                    .size(11.5),
                            );
                        } else {
                            ui.label(
                                RichText::new("● Hermes Seqlock Active")
                                    .color(Color32::from_rgb(100, 120, 140))
                                    .size(11.5),
                            );
                        }
                    } else {
                        let seq = self.channel.sequence();
                        ui.label(
                            RichText::new(format!("● Hermes Seqlock Active (Seq #{})", seq))
                                .color(Color32::from_rgb(57, 255, 20))
                                .size(11.5),
                        );
                    }
                });
            });
        });

        if undo_clicked {
            if let Some(prev) = self.undo_stack.pop() {
                self.redo_stack.push(self.config.clone());
                self.config = prev;
                self.sync_public_config();
            }
        }
        if redo_clicked {
            if let Some(next) = self.redo_stack.pop() {
                self.undo_stack.push(self.config.clone());
                self.config = next;
                self.sync_public_config();
            }
        }
        if apply_clicked {
            self.undo_stack.push(self.config.clone());
            self.redo_stack.clear();
            self.sync_public_config();
            self.status_notification = Some((
                "Live synced over Hermes Seqlock".to_string(),
                Instant::now(),
            ));
        }
        if save_clicked {
            let _ = self.save_to_disk();
        }
        if reset_clicked {
            self.reset_to_defaults();
        }

        self.is_open = !close_clicked && is_open;

        self.render_pythia_hud(ctx);
    }

    fn render_appearance_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Color Palette & Window Appearance")
                .color(Color32::from_rgb(57, 255, 20)),
        );
        ui.add_space(4.0);

        ui.label(RichText::new("Select Active Color Theme:").strong());
        let mut new_theme = None;
        ui.horizontal(|ui| {
            let themes = [
                (0, "Cyber-Neon (Active)", Color32::from_rgb(57, 255, 20)),
                (1, "Tokyo Night", Color32::from_rgb(168, 85, 247)),
                (2, "Matrix Green", Color32::from_rgb(0, 255, 102)),
                (3, "Synthwave '84", Color32::from_rgb(255, 0, 127)),
            ];

            for (id, name, color) in themes {
                let is_selected = self.config.theme_id == id;
                if ui
                    .selectable_label(
                        is_selected,
                        RichText::new(name).color(if is_selected { color } else { Color32::GRAY }),
                    )
                    .clicked()
                {
                    new_theme = Some(id);
                }
            }
        });
        if let Some(id) = new_theme {
            self.config.theme_id = id;
            self.sync_public_config();
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Theme Swatch Preview (Cyber-Neon):").weak());
        ui.horizontal(|ui| {
            let swatches = [
                ("Neon Green", Color32::from_rgb(57, 255, 20)),
                ("Light Blue", Color32::from_rgb(56, 189, 248)),
                ("Magenta", Color32::from_rgb(255, 0, 127)),
                ("Purple", Color32::from_rgb(168, 85, 247)),
                ("Grey", Color32::from_rgb(136, 146, 154)),
                ("Black", Color32::from_rgb(5, 5, 8)),
            ];
            for (label, color) in swatches {
                ui.vertical(|ui| {
                    let (rect, _response) =
                        ui.allocate_exact_size(egui::vec2(24.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, color);
                    ui.label(RichText::new(label).size(10.0));
                });
            }
        });

        ui.separator();
        ui.label(RichText::new("Window Transparency:").strong());
        if ui
            .add(
                Slider::new(&mut self.config.background_opacity, 0.2..=1.0)
                    .text("Background Opacity"),
            )
            .changed()
        {
            self.sync_public_config();
        }
        ui.small("Native compositor blur is not enabled in the current desktop renderer.");

        ui.separator();
        ui.label(RichText::new("Cursor Styling:").strong());
        let mut new_cursor = None;
        ui.horizontal(|ui| {
            let cursors = [(0, "Block (█)"), (1, "Beam (|)"), (2, "Underline (_) ")];
            for (style, name) in cursors {
                if ui
                    .selectable_label(self.config.cursor_style == style, name)
                    .clicked()
                {
                    new_cursor = Some(style);
                }
            }
        });
        if let Some(style) = new_cursor {
            self.config.cursor_style = style;
            self.sync_public_config();
        }
        if ui
            .checkbox(&mut self.config.cursor_blink, "Enable Cursor Blinking")
            .changed()
        {
            self.sync_public_config();
        }
    }

    fn render_typography_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Typography & Font Sizing").color(Color32::from_rgb(56, 189, 248)),
        );
        ui.add_space(4.0);

        ui.label(RichText::new("Terminal metrics:").strong());
        ui.small("Well currently uses its bundled renderer font. Font-family switching is deferred so it cannot silently change terminal metrics.");
        ui.add_space(8.0);
        if ui
            .add(Slider::new(&mut self.config.font_size, 10.0..=32.0).text("Font Size (pt)"))
            .changed()
        {
            self.sync_public_config();
        }
        if ui
            .add(
                Slider::new(&mut self.config.line_height, 1.0..=2.0).text("Line Height Multiplier"),
            )
            .changed()
        {
            self.sync_public_config();
        }
    }

    fn render_shortcuts_tab(&mut self, ui: &mut Ui) {
        self.refresh_keybinding_conflicts();
        ui.heading(
            RichText::new("Command & Keybinding Mapping").color(Color32::from_rgb(250, 204, 21)),
        );
        ui.label("Bindings apply immediately to the desktop terminal and are saved with your Well configuration.");
        ui.small("Use one chord or slash-separated alternatives, for example Cmd+Shift+P / Ctrl+Shift+P.");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_sized(
                [160.0, 20.0],
                egui::Label::new(
                    RichText::new("Shortcut Chord")
                        .strong()
                        .color(Color32::from_rgb(56, 189, 248)),
                ),
            );
            ui.add_sized(
                [220.0, 20.0],
                egui::Label::new(
                    RichText::new("Target Action")
                        .strong()
                        .color(Color32::from_rgb(57, 255, 20)),
                ),
            );
            ui.label(
                RichText::new("Status")
                    .strong()
                    .color(Color32::from_rgb(148, 163, 184)),
            );
        });
        ui.separator();

        let mut bindings_changed = false;
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                let mut to_delete = None;
                for (idx, kb) in self.keybindings.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized([160.0, 22.0], egui::TextEdit::singleline(&mut kb.chord))
                            .changed()
                        {
                            bindings_changed = true;
                        }

                        let selected_action = ShortcutAction::from_label(&kb.action)
                            .map(ShortcutAction::label)
                            .unwrap_or("Unsupported action");
                        egui::ComboBox::from_id_salt(("shortcut-action", idx))
                            .selected_text(selected_action)
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for action in ShortcutAction::ALL {
                                    if ui
                                        .selectable_label(
                                            kb.action == action.label(),
                                            action.label(),
                                        )
                                        .clicked()
                                    {
                                        kb.action = action.label().to_string();
                                        bindings_changed = true;
                                    }
                                }
                            });

                        if kb.conflict {
                            let label = if ShortcutAction::from_label(&kb.action).is_none() {
                                "Unsupported action"
                            } else {
                                "Duplicate or malformed"
                            };
                            ui.label(RichText::new(label).color(Color32::from_rgb(255, 130, 130)));
                        } else {
                            ui.label(RichText::new("Active").color(Color32::from_rgb(57, 255, 20)));
                        }
                        if ui.button("❌").clicked() {
                            to_delete = Some(idx);
                        }
                    });
                }
                if let Some(idx) = to_delete {
                    self.keybindings.remove(idx);
                    bindings_changed = true;
                }
            });

        ui.add_space(6.0);
        ui.separator();
        ui.label(
            RichText::new("Add terminal shortcut:")
                .strong()
                .color(Color32::from_rgb(250, 204, 21)),
        );
        ui.horizontal(|ui| {
            ui.add_sized(
                [160.0, 22.0],
                egui::TextEdit::singleline(&mut self.new_chord).hint_text("e.g. Cmd+Shift+P"),
            );
            egui::ComboBox::from_id_salt("new-shortcut-action")
                .selected_text(&self.new_action)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for action in ShortcutAction::ALL {
                        ui.selectable_value(
                            &mut self.new_action,
                            action.label().to_string(),
                            action.label(),
                        );
                    }
                });
            if ui.button("➕ Add Shortcut").clicked() && !self.new_chord.is_empty() {
                self.keybindings.push(KeybindingRow {
                    chord: self.new_chord.clone(),
                    action: self.new_action.clone(),
                    conflict: false,
                });
                self.new_chord.clear();
                bindings_changed = true;
            }
        });

        if bindings_changed {
            self.refresh_keybinding_conflicts();
        }
    }

    fn render_shell_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Shell & Terminal Session").color(Color32::from_rgb(255, 0, 127)));
        ui.add_space(4.0);

        ui.label("Executable Shell Binary:");
        if ui.text_edit_singleline(&mut self.shell_path).changed() {
            self.config.shell_path = self.shell_path.clone();
            self.sync_public_config();
        }
        ui.small(
            "Use an executable path. Save, then restart Well to apply it to the terminal session.",
        );

        ui.separator();
        ui.label(RichText::new("Scrollback:").strong());
        if ui
            .add(
                Slider::new(&mut self.config.scrollback_limit, 10_000..=500_000)
                    .text("Scrollback Buffer Depth"),
            )
            .changed()
        {
            self.sync_public_config();
        }
        ui.small("Save and restart Well to apply this capacity to the next terminal session.");

        ui.separator();
        ui.label(
            RichText::new("Starship Presets:")
                .strong()
                .color(Color32::from_rgb(0, 240, 255)),
        );
        ui.label("Select and instantly apply a Starship prompt preset to ~/.config/starship.toml:");

        let presets = [
            "Jetpack",
            "Tokyo Night",
            "Pure Minimal",
            "Gruvbox Rainbow",
            "Pastel Powerline",
        ];

        ui.horizontal(|ui| {
            ui.label("Active Preset:");
            egui::ComboBox::from_id_salt("starship_preset_combobox")
                .selected_text(
                    RichText::new(&self.config.prompt_preset)
                        .strong()
                        .color(Color32::from_rgb(57, 255, 20)),
                )
                .show_ui(ui, |ui| {
                    for p in &presets {
                        if ui
                            .selectable_label(self.config.prompt_preset == *p, *p)
                            .clicked()
                        {
                            self.config.prompt_preset = p.to_string();
                            self.sync_public_config();
                        }
                    }
                });
        });

        // Preset Highlights Card
        if self.config.prompt_preset == "Jetpack" {
            ui.group(|ui| {
                ui.label(RichText::new("🚀 Jetpack Preset (Recommended)").color(Color32::from_rgb(250, 204, 21)).strong());
                ui.label("• Ultra high-speed asynchronous prompt with detailed Git metrics (adds/deletes).");
                ui.label("• Command execution timer (◄ 12ms), context symbols, and responsive battery/time status.");
                ui.label("• Custom character states: [◎] success, [○] error, [■] Vim mode.");
            });
        }

        ui.horizontal(|ui| {
            let apply_preset_btn = egui::Button::new(
                RichText::new(format!(
                    "🚀 Apply '{}' Preset to Starship",
                    self.config.prompt_preset
                ))
                .color(Color32::from_rgb(57, 255, 20))
                .strong(),
            )
            .fill(Color32::from_rgba_premultiplied(20, 50, 30, 220))
            .stroke(Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20)))
            .rounding(egui::Rounding::same(6.0));

            if ui.add(apply_preset_btn).clicked() {
                match apply_starship_preset(&self.config.prompt_preset) {
                    Ok(msg) => {
                        self.set_notification(msg);
                        self.sync_public_config();
                    }
                    Err(err) => {
                        self.set_notification(format!("Error: {}", err));
                    }
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("Fish configuration remains owned by Fish.").strong());
        ui.small("Well does not write abbreviations, prompts, or startup hooks into your shell configuration.");
    }

    // These archived panels intentionally remain outside the production build
    // until their controls have a real desktop runtime path.
    #[cfg(any())]
    fn render_shaders_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Orpheus CRT Shaders & Phosphor Effects")
                .color(Color32::from_rgb(168, 85, 247)),
        );
        ui.add_space(4.0);

        if ui
            .add(
                Slider::new(&mut self.config.screen_curvature, 0.0..=0.5)
                    .text("CRT Barrel Curvature"),
            )
            .changed()
        {
            self.sync_public_config();
        }
        if ui
            .add(
                Slider::new(&mut self.config.scanline_frequency, 0.0..=2.0)
                    .text("Scanline Frequency"),
            )
            .changed()
        {
            self.sync_public_config();
        }
        if ui
            .add(Slider::new(&mut self.config.glow_radius, 0.0..=3.0).text("Phosphor Glow Radius"))
            .changed()
        {
            self.sync_public_config();
        }
    }

    fn render_profiles_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Configuration Profiles & Hyprlang/Hermes")
                .color(Color32::from_rgb(0, 240, 255)),
        );
        ui.add_space(4.0);

        let profiles = self.list_profile_names();
        ui.horizontal(|ui| {
            ui.label("Current Profile:");
            egui::ComboBox::from_label("Profile")
                .selected_text(
                    self.current_profile
                        .clone()
                        .unwrap_or_else(|| "<none>".to_string()),
                )
                .show_ui(ui, |ui| {
                    for name in &profiles {
                        ui.selectable_value(&mut self.current_profile, Some(name.clone()), name);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Profile Name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_profile_name)
                    .hint_text("work, travel, demo")
                    .desired_width(180.0),
            );
            if ui.button("Use Selected").clicked() {
                self.new_profile_name = self.current_profile.clone().unwrap_or_default();
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("Hermes IPC Telemetry:").strong());
        ui.label(format!("Active Theme: {}", self.config.theme_id));
        ui.label(format!("Font Size: {} pt", self.config.font_size));
        ui.label(format!(
            "Scrollback Buffer: {} lines",
            self.config.scrollback_limit
        ));
        ui.label(format!(
            "Background Opacity: {:.2}",
            self.config.background_opacity
        ));

        ui.separator();
        let mut save_profile_req = false;
        let mut load_profile_req = false;
        let mut delete_profile_req = false;
        let mut import_hyprlang_req = false;
        let mut export_hyprlang_req = false;

        ui.horizontal(|ui| {
            if ui.button("Save / Create Profile").clicked() {
                save_profile_req = true;
            }
            if ui.button("Load Profile").clicked() {
                load_profile_req = true;
            }
            if ui.button("Delete Profile").clicked() {
                delete_profile_req = true;
            }
            if ui.button("Import Hyprlang (.hl)").clicked() {
                import_hyprlang_req = true;
            }
            if ui.button("Export Hyprlang (.hl)").clicked() {
                export_hyprlang_req = true;
            }
            if ui.button("Reset Defaults").clicked() {
                self.reset_to_defaults();
            }
        });

        if save_profile_req {
            let profile_name = if self.new_profile_name.trim().is_empty() {
                self.current_profile.clone().unwrap_or_default()
            } else {
                self.new_profile_name.trim().to_string()
            };
            if profile_name.is_empty() {
                self.set_notification("Enter a profile name before saving");
            } else if let Err(error) = self.save_profile(&profile_name) {
                self.set_notification(format!("Profile save failed: {error}"));
            }
        }
        if load_profile_req {
            if let Some(name) = self.current_profile.clone() {
                if let Err(error) = self.load_profile(&name) {
                    self.set_notification(format!("Profile load failed: {error}"));
                }
            } else {
                self.set_notification("Select a profile before loading");
            }
        }
        if delete_profile_req {
            if let Some(name) = self.current_profile.clone() {
                if let Err(error) = self.delete_profile(&name) {
                    self.set_notification(format!("Profile delete failed: {error}"));
                }
            } else {
                self.set_notification("Select a profile before deleting");
            }
        }
        if import_hyprlang_req {
            if let Some(path) = Self::hyprlang_path() {
                if let Err(error) = self.import_hyprlang_from_path(&path) {
                    self.set_notification(format!("Hyprlang import failed: {error}"));
                }
            }
        }
        if export_hyprlang_req {
            if let Some(path) = Self::hyprlang_path() {
                if let Err(error) = self.export_hyprlang_to_path(&path) {
                    self.set_notification(format!("Hyprlang export failed: {error}"));
                }
            }
        }
    }

    #[cfg(any())]
    fn render_script_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Automations & HyperShell Execution")
                .color(Color32::from_rgb(57, 255, 20)),
        );
        ui.add_space(6.0);
        ui.label(RichText::new("HyperShell Async Engine").strong());
        ui.label("Integrated non-blocking command execution pipeline with sub-millisecond task dispatch.");
        ui.add_space(8.0);
        ui.label(RichText::new("Quick Automation Triggers:").strong());
        ui.horizontal(|ui| {
            if ui.button("Run Benchmark Suite (Hyperfine)").clicked() {
                let _ = std::process::Command::new("./well-benchmarks.sh").spawn();
                self.status_notification =
                    Some(("Launched Hyperfine benchmark".into(), Instant::now()));
            }
            if ui.button("Launch Fish Transience Check").clicked() {
                let _ = std::process::Command::new("/opt/homebrew/bin/fish")
                    .arg("-c")
                    .arg("echo 'Fish active'")
                    .spawn();
            }
        });
    }

    fn render_help_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Well desktop terminal help").color(Color32::from_rgb(0, 240, 255)),
        );
        ui.add_space(4.0);

        ui.label("Well runs one interactive PTY session, renders its terminal screen locally, and keeps its settings in ~/.config/well/config.json.");
        ui.small("Settings explicitly marked restart apply to the next terminal session. Well does not modify Fish configuration.");
        ui.add_space(8.0);

        ui.label(
            RichText::new("Keyboard Shortcuts:")
                .strong()
                .color(Color32::from_rgb(250, 204, 21)),
        );
        egui::Grid::new("shortcuts_grid")
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Shortcut").strong());
                ui.label(RichText::new("Action").strong());
                ui.label(RichText::new("State").strong());
                ui.end_row();

                for binding in &self.keybindings {
                    ui.label(&binding.chord);
                    ui.label(&binding.action);
                    let state = if binding.conflict {
                        "Needs attention"
                    } else {
                        "Active"
                    };
                    let color = if binding.conflict {
                        Color32::from_rgb(255, 130, 130)
                    } else {
                        Color32::from_rgb(57, 255, 20)
                    };
                    ui.label(RichText::new(state).color(color));
                    ui.end_row();
                }

                ui.label("Cmd+C / Ctrl+Shift+C");
                ui.label("Copy terminal selection");
                ui.label("Built in");
                ui.end_row();

                ui.label("Cmd+V / Ctrl+Shift+V");
                ui.label("Paste into terminal");
                ui.label("Built in");
                ui.end_row();

                ui.label("Shift+PageUp / Shift+PageDown");
                ui.label("Scroll terminal history");
                ui.label("Built in");
                ui.end_row();

                ui.label("Ctrl+Shift+T");
                ui.label("Open/close scrollback timeline");
                ui.label("Built in");
                ui.end_row();
            });

        ui.add_space(10.0);
        ui.label(
            RichText::new("Current desktop scope:")
                .strong()
                .color(Color32::from_rgb(56, 189, 248)),
        );
        ui.label("• Single interactive terminal session with PTY resize, clipboard, selection, and scrollback.");
        ui.label("• Configurable desktop shortcuts, profiles, JSON persistence, and Hyprlang import/export.");
        ui.label("• Bounded Kitty image rendering for terminal graphics protocol output.");
        ui.label("• Optional Pythia command/image assistance with explicit provider and credential handling.");

        ui.add_space(8.0);
        ui.label(
            RichText::new("Deferred features:")
                .strong()
                .color(Color32::from_rgb(168, 85, 247)),
        );
        ui.label(
            "• Multiple tabs/panes, CRT tuning, and sandbox replay are not exposed by this build.",
        );
        ui.label("• See the repository ROADMAP.md for production work that remains.");
    }

    #[cfg(any())]
    fn render_outline_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("📐 Well Architecture & Subsystem Specification")
                .color(Color32::from_rgb(56, 189, 248))
                .size(18.0),
        );
        ui.label(RichText::new("Unified, sub-millisecond, memory-safe terminal, shell, prompt, and editor workspace running at the physical performance ceiling of modern hardware.").italics().color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        // Core Architectural Tenets Card
        egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(15, 23, 42, 200))
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgba_premultiplied(56, 189, 248, 100),
            ))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("⚡ Core Mandates:")
                            .strong()
                            .color(Color32::from_rgb(56, 189, 248)),
                    );
                    ui.label(
                        RichText::new("Sub-2.5ms latency")
                            .strong()
                            .color(Color32::from_rgb(57, 255, 20)),
                    );
                    ui.label("•");
                    ui.label(
                        RichText::new("< 30MB base RSS")
                            .strong()
                            .color(Color32::from_rgb(57, 255, 20)),
                    );
                    ui.label("•");
                    ui.label(
                        RichText::new("4ms I/O Batching (250 FPS)")
                            .strong()
                            .color(Color32::from_rgb(57, 255, 20)),
                    );
                    ui.label("•");
                    ui.label(
                        RichText::new("Zero Subprocess Forks")
                            .strong()
                            .color(Color32::from_rgb(57, 255, 20)),
                    );
                });
            });

        ui.add_space(10.0);
        ui.label(
            RichText::new("Subsystem Pantheon:")
                .strong()
                .size(15.0)
                .color(Color32::from_rgb(255, 255, 255)),
        );
        ui.add_space(4.0);

        let subsystems = [
            ("ATLAS", "Host Windowing & OS Loop", Color32::from_rgb(0, 240, 255), "winit event loop pacer, high-precision frame timers, raw modifier bitmask capture, split-pane multiplexing (Cmd+D / Cmd+Shift+D), and native Cocoa/Wayland window chrome integration."),
            ("ORPHEUS", "GPU Text Shaper & Pipeline", Color32::from_rgb(56, 189, 248), "Single-pass instanced drawIndexed calls, layered dynamic texture glyph array (monochrome ASCII, bold/italic, CJK, RGBA emojis), branchless WGSL fragment shading, dynamic CRT curvature & scanline passes, and DEC 2026 150ms timeout lock recovery."),
            ("METIS", "Shell Logic & History Engine", Color32::from_rgb(255, 0, 127), "Direct in-process shell execution, O(k) prefix-trie indexing up to 260,000 commands from history, sub-50µs ghost completions with Right Arrow/Ctrl+F full accept and Alt+Right Arrow token traversal."),
            ("MNEME", "Inline Composition Buffer", Color32::from_rgb(250, 204, 21), "In-place prompt expansion for unclosed multi-line commands (if, for, unclosed quotes) or Alt+E/Alt+V, backed by an O(log n) B-tree rope buffer (ropey), background incremental Tree-Sitter AST syntax validator (green/red diagnostics), and embedded LSP client."),
            ("ASTRAEA", "State-Vector Prompt Compiler", Color32::from_rgb(168, 85, 247), "Zero-fork background state vector daemon tracking SCM (Git branch, dirty files, ahead/behind, stash count), cloud environment, and battery without subprocess spawns. Compiles prompts in < 100µs with strict prompt transience collapsing past prompts to ❯."),
            ("HERMES", "PTY-Bypass Protocol & Seqlock IPC", Color32::from_rgb(57, 255, 20), "Lock-free atomic sequence lock (AtomicU64) ring queues mapped directly between shell logic and GPU renderer, zero mutex contention, and strongly typed binary IPC schemas."),
            ("CADUCEUS", "In-Process hyper RPC Daemon", Color32::from_rgb(244, 63, 94), "Local-only async HTTP/WebSocket server over Unix Domain Sockets (~/.well/well.sock) or Windows Named Pipes, enforced by kernel peer credential verification (SO_PEERCRED at 0600), exposing workspace.*, surface.*, and ai.* orchestration hooks."),
            ("THEIA'S PRISM", "Visual GPU Control Center", Color32::from_rgb(0, 240, 255), "Immediate-mode egui control center overlay (Cmd+, / F12), real-time telemetry HUD, Hyprlang/TOML two-way config synchronization, and live Tree-Sitter syntax validation gates preventing corrupt config persistence."),
        ];

        for (name, subtitle, color, desc) in subsystems {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(18, 24, 38, 180))
                .stroke(Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_premultiplied(
                        color.r() / 3,
                        color.g() / 3,
                        color.b() / 3,
                        160,
                    ),
                ))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("◈ {}", name))
                                .strong()
                                .color(color)
                                .size(13.0),
                        );
                        ui.label(
                            RichText::new(format!("— {}", subtitle))
                                .color(Color32::from_rgb(203, 213, 225))
                                .size(12.0),
                        );
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(desc)
                            .color(Color32::from_rgb(148, 163, 184))
                            .size(11.5),
                    );
                });
            ui.add_space(4.0);
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new("Strict Architectural Boundaries:")
                .strong()
                .size(13.0)
                .color(Color32::from_rgb(250, 204, 21)),
        );
        ui.label("1. Never spawn subprocesses in hot paths (prompts, suggestions, keystrokes).");
        ui.label("2. Every viewport redraw must occur in a single instanced GPU draw call.");
        ui.label("3. Memory footprint must stay strictly below 30MB baseline RSS.");
        ui.label("4. Redraw frequencies are capped at 250 FPS via 4ms batch epochs to prevent log-flood GPU saturation.");
    }

    #[cfg(any())]
    fn render_timeline_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("⏱️ Well 12-Week Production Build Roadmap")
                .color(Color32::from_rgb(244, 63, 94))
                .size(18.0),
        );
        ui.label(RichText::new("Strategic 6-phase engineering timeline from GPU & terminal core maturity to production cross-platform distribution.").italics().color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        let phases = [
            (
                "Phase 1: GPU Engine & Windowing Maturity",
                "Weeks 1–2",
                "90% Complete",
                Color32::from_rgb(57, 255, 20),
                "Split-pane multiplexing (Cmd+D / Cmd+Shift+D), CRT post-processing shader pass, 4ms I/O event coalescing, DEC 2026 150ms timeout safety gate, full Kitty Keyboard Protocol compliance.",
                "Target: 250 FPS ceiling under log flood; sub-millisecond redraws.",
            ),
            (
                "Phase 2: In-Process Shell Core & Predictive Engine",
                "Weeks 3–4",
                "60% Complete",
                Color32::from_rgb(56, 189, 248),
                "Metis prefix-trie history indexer (260,000 commands), sub-50µs ghost completion suggestions, path-token traversal (Alt+Right Arrow), in-namespace command execution.",
                "Target: < 50 µs ghost completion lookup time.",
            ),
            (
                "Phase 3: Astraea State-Vector Prompt Engine",
                "Weeks 5–6",
                "50% Complete",
                Color32::from_rgb(168, 85, 247),
                "File-watcher event-driven state vector (Git SCM, cloud, battery), zero-fork prompt compiler, strict prompt transience collapsing old prompts to ❯.",
                "Target: < 100 µs prompt compile time (verified by unit tests).",
            ),
            (
                "Phase 4: Mneme Inline Editor & Safety AST Validator",
                "Weeks 7–8",
                "40% Complete",
                Color32::from_rgb(250, 204, 21),
                "Dynamic in-place multi-line expansion on unclosed blocks or Alt+E/Alt+V, ropey B-tree buffer, live Tree-Sitter AST syntax validator (green valid / red error), embedded LSP diagnostic channel.",
                "Target: Live syntax validation with zero typing buffer lag.",
            ),
            (
                "Phase 5: Hermes Shared-Memory IPC & Caduceus RPC",
                "Weeks 9–10",
                "50% Complete",
                Color32::from_rgb(0, 240, 255),
                "Lock-free Seqlock ring buffers, local Unix domain socket daemon (~/.well/well.sock) with SO_PEERCRED validation, agent orchestration endpoints (workspace.*, surface.*, ai.*).",
                "Target: Sub-millisecond RPC roundtrip and zero mutex contention.",
            ),
            (
                "Phase 6: Hardening, Packaging & Distribution",
                "Weeks 11–12",
                "25% Complete",
                Color32::from_rgb(244, 63, 94),
                "Fat LTO release profile verification (<30MB RSS, <2.5ms latency), cross-platform packaging (macOS Universal .dmg, Linux AppImage/deb/rpm, Windows MSI/zip), vttest compliance suite.",
                "Target: Zero-flicker 144Hz+ rendering, full terminal compatibility.",
            ),
        ];

        for (title, horizon, progress, color, deliverables, metric) in phases {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(18, 24, 38, 180))
                .stroke(Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_premultiplied(
                        color.r() / 3,
                        color.g() / 3,
                        color.b() / 3,
                        160,
                    ),
                ))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(title).strong().color(color).size(13.5));
                        ui.label(
                            RichText::new(format!("— {}", horizon))
                                .color(Color32::from_rgb(203, 213, 225))
                                .size(12.0),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(progress).strong().color(color).size(11.5));
                        });
                    });
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(deliverables)
                            .color(Color32::from_rgb(148, 163, 184))
                            .size(11.5),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(metric)
                            .italics()
                            .color(Color32::from_rgb(250, 204, 21))
                            .size(11.0),
                    );
                });
            ui.add_space(4.0);
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new("Milestone Summary Schedule:")
                .strong()
                .size(13.0)
                .color(Color32::from_rgb(56, 189, 248)),
        );
        egui::Grid::new("timeline_milestone_grid")
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Milestone")
                        .strong()
                        .color(Color32::from_rgb(56, 189, 248)),
                );
                ui.label(
                    RichText::new("Horizon")
                        .strong()
                        .color(Color32::from_rgb(56, 189, 248)),
                );
                ui.label(
                    RichText::new("Primary Focus")
                        .strong()
                        .color(Color32::from_rgb(56, 189, 248)),
                );
                ui.label(
                    RichText::new("Success Verification")
                        .strong()
                        .color(Color32::from_rgb(56, 189, 248)),
                );
                ui.end_row();

                let milestones = [
                    (
                        "M1",
                        "Week 2",
                        "Multiplexing & Shaders",
                        "250 FPS ceiling under log flood",
                    ),
                    (
                        "M2",
                        "Week 4",
                        "Predictive Shell Engine",
                        "< 50 µs ghost completion lookup",
                    ),
                    (
                        "M3",
                        "Week 6",
                        "State-Vector Prompts",
                        "< 100 µs compile time, transience",
                    ),
                    (
                        "M4",
                        "Week 8",
                        "Mneme Inline Editor",
                        "Live AST validation, zero buffer lag",
                    ),
                    (
                        "M5",
                        "Week 10",
                        "Hermes & Caduceus IPC",
                        "Sub-millisecond IPC roundtrip",
                    ),
                    (
                        "M6",
                        "Week 12",
                        "Production Release",
                        "< 30 MB RSS, < 2.5 ms latency",
                    ),
                ];

                for (m, horizon, focus, verify) in milestones {
                    ui.label(
                        RichText::new(m)
                            .strong()
                            .color(Color32::from_rgb(255, 255, 255)),
                    );
                    ui.label(RichText::new(horizon).color(Color32::from_rgb(203, 213, 225)));
                    ui.label(RichText::new(focus).color(Color32::from_rgb(203, 213, 225)));
                    ui.label(RichText::new(verify).color(Color32::from_rgb(57, 255, 20)));
                    ui.end_row();
                }
            });
    }

    fn render_pythia_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("✦ Pythia LLM Translation & Diagnostic Layer")
                .color(Color32::from_rgb(192, 132, 252)),
        );
        ui.label(
            RichText::new(
                "Configure high-speed Natural Language to Shell synthesis and error diagnosis.",
            )
            .color(Color32::from_rgb(148, 163, 184)),
        );
        ui.add_space(8.0);

        ui.label(RichText::new("Primary AI Engine:").strong());
        let providers = [
            (
                "Gemma Abliterated (Hugging Face)",
                "Cloud request to Hugging Face when HF_TOKEN is configured",
            ),
            (
                "Google Gemini 2.0",
                "Cloud request to Google Gemini when GEMINI_API_KEY is configured",
            ),
            (
                "Ollama (Local)",
                "Sends prompts to the configured Ollama endpoint; localhost by default",
            ),
            (
                "Offline Semantic Rules",
                "Local rule-based fallback; no provider request",
            ),
        ];
        for (p, desc) in providers {
            let selected = self.config.pythia_provider == p;
            ui.horizontal(|ui| {
                if ui.selectable_label(selected, p).clicked() {
                    self.config.pythia_provider = p.to_string();
                }
                ui.label(
                    RichText::new(format!("— {}", desc))
                        .size(11.0)
                        .color(Color32::from_rgb(120, 140, 165)),
                );
            });
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            RichText::new("1. Hugging Face Configuration")
                .strong()
                .color(Color32::from_rgb(250, 204, 21)),
        );
        ui.label(RichText::new("Cloud provider: sends prompts and lightweight shell context to Hugging Face when a token is configured. Review commands before use.").size(11.0).color(Color32::from_rgb(160, 180, 205)));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Model ID:");
            ui.text_edit_singleline(&mut self.config.hf_model);
            if ui.button("Gemma 2 9B (Default)").clicked() {
                self.config.hf_model = "failspy/gemma-2-9b-it-abliterated".to_string();
            }
            if ui.button("Gemma 2 2B (Lightweight)").clicked() {
                self.config.hf_model = "mlabonne/gemma-2-2b-it-abliterated".to_string();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Hugging Face Token (HF_TOKEN):");
            ui.add(egui::TextEdit::singleline(&mut self.config.hf_token).password(true));
            if ui.button("Save to Keychain").clicked() {
                self.hf_token = self.config.hf_token.clone();
                self.save_ai_credential(AiCredential::HuggingFaceToken);
            }
            if ui.button("Remove from Keychain").clicked() {
                self.remove_ai_credential(AiCredential::HuggingFaceToken);
            }
        });
        ui.label(
            RichText::new("Never written to Well's config. HF_TOKEN overrides OS Keychain; use Save to Keychain for secure reuse.")
                .size(10.5)
                .color(Color32::from_rgb(120, 140, 165)),
        );

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            RichText::new("2. Google Gemini Configuration")
                .strong()
                .color(Color32::from_rgb(56, 189, 248)),
        );
        ui.label(RichText::new("Cloud provider: sends prompts, diagnostics, and image prompts to Gemini when an API key is configured.").size(11.0).color(Color32::from_rgb(160, 180, 205)));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Gemini API Key:");
            ui.add(egui::TextEdit::singleline(&mut self.config.gemini_api_key).password(true));
            if ui.button("Save to Keychain").clicked() {
                self.save_ai_credential(AiCredential::GeminiApiKey);
            }
            if ui.button("Remove from Keychain").clicked() {
                self.remove_ai_credential(AiCredential::GeminiApiKey);
            }
        });
        ui.label(
            RichText::new("Never written to Well's config. GEMINI_API_KEY overrides OS Keychain; use Save to Keychain for secure reuse.")
                .size(10.5)
                .color(Color32::from_rgb(120, 140, 165)),
        );
        ui.horizontal(|ui| {
            ui.label("Model ID:");
            ui.text_edit_singleline(&mut self.config.gemini_model);
            if ui.button("gemini-2.0-flash").clicked() {
                self.config.gemini_model = "gemini-2.0-flash".to_string();
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            RichText::new("3. Local Ollama Configuration")
                .strong()
                .color(Color32::from_rgb(57, 255, 20)),
        );
        ui.horizontal(|ui| {
            ui.label("Ollama Endpoint:");
            ui.text_edit_singleline(&mut self.config.ollama_url);
        });
        ui.horizontal(|ui| {
            ui.label("Model Name:");
            ui.text_edit_singleline(&mut self.config.ollama_model);
            if ui.button("qwen2.5-coder").clicked() {
                self.config.ollama_model = "qwen2.5-coder".to_string();
            }
            if ui.button("llama3.2").clicked() {
                self.config.ollama_model = "llama3.2".to_string();
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            RichText::new("4. Cybernetic Image & Graphic Generation Studio")
                .strong()
                .color(Color32::from_rgb(192, 132, 252)),
        );
        ui.label(RichText::new("Generate high-resolution PNG terminal graphics, background textures, and icons using Gemini Omni Flash or Hugging Face Stable Diffusion.").size(11.0).color(Color32::from_rgb(160, 180, 205)));
        ui.add_space(6.0);

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Storage Cache:")
                    .size(11.0)
                    .color(Color32::from_rgb(120, 140, 165)),
            );
            ui.label(
                RichText::new(cache_dir.to_string_lossy())
                    .size(11.0)
                    .monospace()
                    .color(Color32::from_rgb(56, 189, 248)),
            );
            if ui.button("📁 Open Folder").clicked() {
                let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
            }
        });
        ui.add_space(6.0);

        ui.label(
            RichText::new("One-Click Graphic Presets:")
                .size(11.5)
                .strong()
                .color(Color32::from_rgb(200, 215, 235)),
        );
        ui.horizontal_wrapped(|ui| {
            let presets = [
                (
                    "🖼 Synthwave Grid",
                    "synthwave neon grid wireframe sunset",
                    1024,
                    1024,
                ),
                (
                    "🌆 Cyber City",
                    "cyberpunk matrix rain city skyline",
                    1024,
                    1024,
                ),
                (
                    "👾 Terminal Icon",
                    "retro phosphor green terminal icon",
                    512,
                    512,
                ),
                (
                    "🌌 Deep Space",
                    "deep space cosmic nebula stars",
                    1024,
                    1024,
                ),
                (
                    "⚡ Quantum Flux",
                    "quantum computing golden glowing circuit board",
                    1024,
                    1024,
                ),
            ];
            for (label, prompt, w, h) in presets {
                let btn = egui::Button::new(
                    RichText::new(label)
                        .size(11.0)
                        .strong()
                        .color(Color32::from_rgb(192, 132, 252)),
                )
                .fill(Color32::from_rgba_premultiplied(32, 20, 48, 220))
                .stroke(Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_premultiplied(168, 85, 247, 120),
                ))
                .rounding(egui::Rounding::same(6.0));
                if ui.add(btn).clicked() {
                    self.pythia_query = format!("/image {}", prompt);
                    self.trigger_image_generation(prompt, w, h);
                }
            }
        });

        if let Some(ref res) = self.pythia_result {
            if let Some(ref path) = res.generated_image_path {
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(12, 16, 28, 240))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("🖼 Active Artifact:")
                                    .strong()
                                    .color(Color32::from_rgb(192, 132, 252))
                                    .size(12.0),
                            );
                            ui.label(
                                RichText::new(path)
                                    .monospace()
                                    .color(Color32::from_rgb(148, 163, 184))
                                    .size(11.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            if ui.button("🔍 Open Image").clicked() {
                                let _ = std::process::Command::new("open").arg(path).spawn();
                            }
                            if ui.button("📁 Reveal in Finder").clicked() {
                                let _ = std::process::Command::new("open")
                                    .arg("-R")
                                    .arg(path)
                                    .spawn();
                            }
                            if ui.button("⚡ Inject Path to Shell").clicked() {
                                self.pending_pty_injection = Some(format!("{}\n", path));
                            }
                        });
                    });
            }
        }
    }

    pub fn trigger_image_generation(&mut self, prompt: &str, width: u32, height: u32) {
        self.start_image_generation(prompt, width, height, false);
    }

    pub fn regenerate_image(&mut self, prompt: &str, width: u32, height: u32) {
        self.start_image_generation(prompt, width, height, true);
    }

    fn refresh_image_artifacts(&mut self) {
        if let Ok(cache_dir) = generated_image_cache_dir() {
            self.image_artifacts = load_image_artifacts_from_dir(&cache_dir);
        }
    }

    fn persist_image_artifacts(&mut self) -> Result<(), String> {
        let cache_dir = generated_image_cache_dir()?;
        save_image_artifacts_to_dir(&cache_dir, &self.image_artifacts)
    }

    fn remember_generated_image(&mut self, result: &well_llm::TranslationResult) {
        let Some(path) = result.generated_image_path.clone() else {
            return;
        };
        let Some(request) = self.image_generation_pending.clone() else {
            return;
        };

        let created_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        let record = ImageArtifactRecord {
            path,
            prompt: request.prompt,
            provider: result.provider_used.clone(),
            width: request.width,
            height: request.height,
            created_at_unix,
        };

        self.image_artifacts
            .retain(|artifact| artifact.path != record.path);
        self.image_artifacts.insert(0, record);
        self.image_artifacts.truncate(50);
        if let Err(error) = self.persist_image_artifacts() {
            self.set_notification(format!("Image generated; manifest update failed: {error}"));
        }
    }

    fn delete_image_artifact(&mut self, path: &str) {
        let path_buf = PathBuf::from(path);
        let cache_dir = match generated_image_cache_dir() {
            Ok(cache_dir) => cache_dir,
            Err(error) => {
                self.set_notification(error);
                return;
            }
        };

        if !image_artifact_is_inside_cache(&cache_dir, &path_buf) {
            self.set_notification("Refusing to delete an image outside Well's generated cache");
            return;
        }

        match std::fs::remove_file(&path_buf) {
            Ok(()) => {
                self.image_artifacts
                    .retain(|artifact| artifact.path != path);
                if self
                    .pythia_result
                    .as_ref()
                    .and_then(|result| result.generated_image_path.as_deref())
                    == Some(path)
                {
                    self.pythia_result = None;
                }
                if let Err(error) = self.persist_image_artifacts() {
                    self.set_notification(format!(
                        "Deleted image; manifest update failed: {error}"
                    ));
                } else {
                    self.set_notification("Deleted generated image artifact");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.image_artifacts
                    .retain(|artifact| artifact.path != path);
                let _ = self.persist_image_artifacts();
                self.set_notification("Removed missing image from history");
            }
            Err(error) => self.set_notification(format!("Unable to delete image: {error}")),
        }
    }

    fn start_image_generation(
        &mut self,
        prompt: &str,
        width: u32,
        height: u32,
        force_image_regeneration: bool,
    ) {
        if self.image_generation_in_flight {
            self.set_notification("Image generation is already in progress");
            return;
        }

        // Clear last URI so that the render function will call forget_image() on
        // it before attempting to load the new file, bypassing egui's stale cache.
        self.last_image_uri = None;
        let provider = match self.config.pythia_provider.as_str() {
            "Gemma Abliterated (Hugging Face)" => {
                well_llm::LlmProvider::HuggingFaceGemmaAbliterated
            }
            "Google Gemini 2.0" => well_llm::LlmProvider::Gemini,
            "Ollama (Local)" => well_llm::LlmProvider::Ollama,
            _ => well_llm::LlmProvider::OfflineRules,
        };

        let req = well_llm::TranslationRequest {
            query: String::new(),
            shell: self.config.shell_path.clone(),
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            provider,
            gemini_api_key: if self.config.gemini_api_key.trim().is_empty() {
                None
            } else {
                Some(self.config.gemini_api_key.trim().to_string())
            },
            gemini_model: Some(self.config.gemini_model.clone()),
            ollama_url: Some(self.config.ollama_url.clone()),
            ollama_model: Some(self.config.ollama_model.clone()),
            hf_token: if self.config.hf_token.trim().is_empty() {
                None
            } else {
                Some(self.config.hf_token.trim().to_string())
            },
            hf_model: Some(self.config.hf_model.clone()),
            image_prompt: Some(prompt.to_string()),
            image_width: Some(width),
            image_height: Some(height),
            force_image_regeneration,
        };

        self.pythia_destructive_confirmation.clear();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        match std::thread::Builder::new()
            .name("well-image-generation".to_string())
            .spawn(move || {
                let _ = result_tx.send(well_llm::PythiaTranslator::translate(&req));
            }) {
            Ok(_) => {
                self.image_generation_result_rx = Some(result_rx);
                self.image_generation_pending = Some(PendingImageGeneration {
                    prompt: prompt.to_string(),
                    width,
                    height,
                    force_regeneration: force_image_regeneration,
                });
                self.image_generation_in_flight = true;
                self.pythia_result = None;
                self.set_notification("Generating image in the background…");
            }
            Err(error) => {
                self.pythia_result = Some(well_llm::TranslationResult {
                    command: String::new(),
                    explanation: format!("Unable to start image generation: {error}"),
                    confidence: 0.0,
                    is_destructive: false,
                    provider_used: "Image generation unavailable".to_string(),
                    fell_back_to_offline: false,
                    generated_image_path: None,
                });
                self.image_generation_pending = None;
                self.set_notification("Unable to start image generation");
            }
        }
    }

    /// Collects a completed image-generation job without blocking the UI thread.
    /// Returns true only when the visible result changed.
    pub fn poll_image_generation(&mut self) -> bool {
        let result = match self.image_generation_result_rx.as_ref() {
            Some(receiver) => receiver.try_recv(),
            None => return false,
        };

        match result {
            Ok(result) => {
                let succeeded = result.generated_image_path.is_some();
                if succeeded {
                    self.remember_generated_image(&result);
                }
                self.pythia_result = Some(result);
                self.image_generation_result_rx = None;
                self.image_generation_pending = None;
                self.image_generation_in_flight = false;
                self.set_notification(if succeeded {
                    "Image generation complete"
                } else {
                    "Image generation failed; see Pythia for details"
                });
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pythia_result = Some(well_llm::TranslationResult {
                    command: String::new(),
                    explanation: "The image-generation worker stopped before returning a result."
                        .to_string(),
                    confidence: 0.0,
                    is_destructive: false,
                    provider_used: "Image generation unavailable".to_string(),
                    fell_back_to_offline: false,
                    generated_image_path: None,
                });
                self.image_generation_result_rx = None;
                self.image_generation_pending = None;
                self.image_generation_in_flight = false;
                self.set_notification("Image generation worker stopped unexpectedly");
                true
            }
        }
    }

    fn render_image_studio_tab(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("🖼 Cybernetic Image & Graphic Studio")
                .color(Color32::from_rgb(56, 189, 248))
                .size(18.0),
        );
        ui.label(RichText::new("Generate provider-backed image artifacts, background textures, and icons from prompts.").color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        if self.image_generation_in_flight {
            ui.label(
                RichText::new("◌ Generating with the selected provider in the background…")
                    .color(Color32::from_rgb(250, 204, 21)),
            );
            ui.add_space(6.0);
        }

        let cache_dir = generated_image_cache_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (provider_status, provider_status_color) =
            image_provider_status(&self.config.pythia_provider, &self.config);

        // Studio Info Box
        egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(12, 18, 30, 220))
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgba_premultiplied(56, 189, 248, 100),
            ))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("📁 Storage Cache:")
                            .strong()
                            .color(Color32::from_rgb(120, 140, 165)),
                    );
                    ui.label(
                        RichText::new(cache_dir.to_string_lossy())
                            .monospace()
                            .color(Color32::from_rgb(56, 189, 248)),
                    );
                    if ui.button("📁 Open Folder in Finder").clicked() {
                        let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Provider:")
                            .strong()
                            .color(Color32::from_rgb(120, 140, 165)),
                    );
                    ui.label(
                        RichText::new(&self.config.pythia_provider)
                            .color(Color32::from_rgb(200, 215, 235)),
                    );
                    ui.label(RichText::new(provider_status).color(provider_status_color));
                    if ui.button("Refresh History").clicked() {
                        self.refresh_image_artifacts();
                    }
                });
            });

        ui.add_space(10.0);
        ui.label(
            RichText::new("⚡ One-Click Generation Presets:")
                .strong()
                .size(13.0)
                .color(Color32::from_rgb(200, 215, 235)),
        );
        ui.add_space(4.0);

        let presets = [
            (
                "🖼 Synthwave Neon Grid",
                "synthwave neon grid wireframe sunset",
                1024,
                1024,
                Color32::from_rgb(255, 0, 128),
            ),
            (
                "🌆 Cyberpunk Matrix City",
                "cyberpunk matrix rain city skyline",
                1024,
                1024,
                Color32::from_rgb(57, 255, 20),
            ),
            (
                "👾 Retro Terminal Icon",
                "retro phosphor green terminal icon",
                512,
                512,
                Color32::from_rgb(56, 189, 248),
            ),
            (
                "🌌 Deep Space Nebula",
                "deep space cosmic nebula stars",
                1024,
                1024,
                Color32::from_rgb(192, 132, 252),
            ),
            (
                "⚡ Quantum Golden Circuit",
                "quantum computing golden glowing circuit board",
                1024,
                1024,
                Color32::from_rgb(250, 204, 21),
            ),
        ];

        for (label, prompt, w, h, accent) in presets {
            ui.horizontal(|ui| {
                let btn = egui::Button::new(RichText::new(label).strong().color(accent).size(12.0))
                    .fill(Color32::from_rgba_premultiplied(
                        accent.r() / 8,
                        accent.g() / 8,
                        accent.b() / 8,
                        220,
                    ))
                    .stroke(Stroke::new(1.0_f32, accent))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add_sized([240.0, 30.0], btn).clicked() {
                    self.pythia_query = format!("/image {}", prompt);
                    self.trigger_image_generation(prompt, w, h);
                }
                ui.label(
                    RichText::new(format!("{}x{} • \"{}\"", w, h, prompt))
                        .color(Color32::from_rgb(140, 155, 175))
                        .size(11.0),
                );
            });
            ui.add_space(3.0);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            RichText::new("Custom Image Generator:")
                .strong()
                .size(13.0)
                .color(Color32::from_rgb(56, 189, 248)),
        );
        ui.add_space(4.0);

        let mut do_generate = false;
        ui.horizontal(|ui| {
            ui.label("Prompt:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.pythia_query)
                    .hint_text("e.g. 'synthwave sunset wireframe grid' or '/image ...'")
                    .desired_width(ui.available_width() - 180.0),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                do_generate = true;
            }

            let gen_btn = egui::Button::new(
                RichText::new("⚡ Generate")
                    .strong()
                    .color(Color32::from_rgb(56, 189, 248)),
            )
            .fill(Color32::from_rgba_premultiplied(16, 32, 54, 220))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
            .rounding(egui::Rounding::same(6.0));
            if ui.add_sized([100.0, 26.0], gen_btn).clicked() {
                do_generate = true;
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Dimensions:")
                    .size(11.0)
                    .color(Color32::from_rgb(120, 140, 165)),
            );
            let dims = [((512, 512), "512 x 512"), ((1024, 1024), "1024 x 1024")];
            for (d, name) in dims {
                let sel = self.pythia_image_dim == d;
                let btn = egui::Button::new(RichText::new(name).size(10.5).color(if sel {
                    Color32::BLACK
                } else {
                    Color32::from_rgb(56, 189, 248)
                }))
                .fill(if sel {
                    Color32::from_rgb(56, 189, 248)
                } else {
                    Color32::from_rgba_premultiplied(16, 28, 44, 180)
                })
                .rounding(egui::Rounding::same(4.0));
                if ui.add(btn).clicked() {
                    self.pythia_image_dim = d;
                }
            }
        });

        if do_generate {
            let prompt = self
                .pythia_query
                .trim_start_matches("/image ")
                .trim_start_matches("/img ")
                .trim()
                .to_string();
            if !prompt.is_empty() {
                self.trigger_image_generation(
                    &prompt,
                    self.pythia_image_dim.0,
                    self.pythia_image_dim.1,
                );
            }
        }

        if let Some(ref res) = self.pythia_result {
            if res.generated_image_path.is_none() {
                ui.add_space(10.0);
                ui.colored_label(
                    Color32::from_rgb(248, 113, 113),
                    format!("{}: {}", res.provider_used, res.explanation),
                );
            }
            if let Some(ref img_path) = res.generated_image_path {
                ui.add_space(12.0);
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(12, 16, 28, 240))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("🖼 Active Artifact:")
                                    .strong()
                                    .color(Color32::from_rgb(192, 132, 252))
                                    .size(13.0),
                            );
                            ui.label(
                                RichText::new(img_path)
                                    .monospace()
                                    .color(Color32::from_rgb(148, 163, 184))
                                    .size(11.5),
                            );
                        });
                        ui.add_space(6.0);
                        // In-window visual image preview via egui texture
                        // Forget any previously cached texture for this path to ensure
                        // fresh file reads after each generation.
                        let img_uri = format!("file://{}", img_path);
                        if self.last_image_uri.as_deref() != Some(img_path.as_str()) {
                            if let Some(ref old_uri) = self.last_image_uri {
                                ui.ctx().forget_image(old_uri);
                            }
                            // Also forget the current URI to force a fresh load
                            ui.ctx().forget_image(&img_uri);
                            self.last_image_uri = Some(img_path.clone());
                        }
                        ui.add(
                            egui::Image::new(&img_uri)
                                .max_width(320.0)
                                .max_height(320.0)
                                .rounding(egui::Rounding::same(6.0)),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let open_btn = egui::Button::new(
                                RichText::new("🔍 Open in Viewer")
                                    .strong()
                                    .color(Color32::from_rgb(57, 255, 20)),
                            )
                            .fill(Color32::from_rgba_premultiplied(18, 42, 28, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(57, 255, 20)))
                            .rounding(egui::Rounding::same(6.0));
                            if ui.add(open_btn).clicked() {
                                let _ = std::process::Command::new("open").arg(img_path).spawn();
                            }

                            let reveal_btn = egui::Button::new(
                                RichText::new("📁 Reveal in Finder")
                                    .strong()
                                    .color(Color32::from_rgb(56, 189, 248)),
                            )
                            .fill(Color32::from_rgba_premultiplied(16, 28, 44, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                            .rounding(egui::Rounding::same(6.0));
                            if ui.add(reveal_btn).clicked() {
                                let _ = std::process::Command::new("open")
                                    .arg("-R")
                                    .arg(img_path)
                                    .spawn();
                            }

                            let inject_btn = egui::Button::new(
                                RichText::new("⚡ Inject Path to Shell")
                                    .strong()
                                    .color(Color32::from_rgb(250, 204, 21)),
                            )
                            .fill(Color32::from_rgba_premultiplied(40, 30, 10, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(250, 204, 21)))
                            .rounding(egui::Rounding::same(6.0));
                            if ui.add(inject_btn).clicked() {
                                self.pending_pty_injection = Some(format!("{}\n", img_path));
                            }
                        });
                    });
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Recent Artifacts")
                    .strong()
                    .size(13.0)
                    .color(Color32::from_rgb(200, 215, 235)),
            );
            ui.label(
                RichText::new(format!("{} kept", self.image_artifacts.len()))
                    .size(11.0)
                    .color(Color32::from_rgb(120, 140, 165)),
            );
        });

        let mut delete_artifact: Option<String> = None;
        let mut regenerate_artifact: Option<ImageArtifactRecord> = None;
        let artifacts = self.image_artifacts.clone();
        if artifacts.is_empty() {
            ui.label(
                RichText::new("No generated images yet.")
                    .color(Color32::from_rgb(120, 140, 165))
                    .size(11.5),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for artifact in artifacts {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(10, 14, 24, 220))
                            .stroke(Stroke::new(
                                1.0_f32,
                                Color32::from_rgba_premultiplied(56, 189, 248, 70),
                            ))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{}x{}",
                                            artifact.width, artifact.height
                                        ))
                                        .monospace()
                                        .color(Color32::from_rgb(56, 189, 248)),
                                    );
                                    ui.label(
                                        RichText::new(&artifact.prompt)
                                            .color(Color32::from_rgb(200, 215, 235)),
                                    );
                                    ui.label(
                                        RichText::new(&artifact.provider)
                                            .size(10.5)
                                            .color(Color32::from_rgb(120, 140, 165)),
                                    );
                                });
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Open").clicked() {
                                        let _ = std::process::Command::new("open")
                                            .arg(&artifact.path)
                                            .spawn();
                                    }
                                    if ui.button("Reveal").clicked() {
                                        let _ = std::process::Command::new("open")
                                            .arg("-R")
                                            .arg(&artifact.path)
                                            .spawn();
                                    }
                                    if ui.button("Copy Path").clicked() {
                                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                            let _ = clipboard.set_text(artifact.path.clone());
                                        }
                                    }
                                    if ui.button("Insert Path").clicked() {
                                        self.pending_pty_injection =
                                            Some(format!("{}\n", artifact.path));
                                    }
                                    if ui.button("Regenerate").clicked() {
                                        regenerate_artifact = Some(artifact.clone());
                                    }
                                    if ui.button("Delete").clicked() {
                                        delete_artifact = Some(artifact.path.clone());
                                    }
                                });
                                ui.label(
                                    RichText::new(&artifact.path)
                                        .monospace()
                                        .size(10.0)
                                        .color(Color32::from_rgb(90, 110, 135)),
                                );
                            });
                        ui.add_space(6.0);
                    }
                });
        }

        if let Some(artifact) = regenerate_artifact {
            self.pythia_query = format!("/image {}", artifact.prompt);
            self.regenerate_image(&artifact.prompt, artifact.width, artifact.height);
        }
        if let Some(path) = delete_artifact {
            self.delete_image_artifact(&path);
        }
    }

    pub fn render_agent_sidebar(&mut self, ctx: &egui::Context) {
        if self.agent_events.is_empty() {
            return;
        }

        egui::SidePanel::right("agent_activity_sidebar")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading(
                    egui::RichText::new("✦ Agent Activity")
                        .color(egui::Color32::from_rgb(168, 85, 247))
                        .strong(),
                );
                ui.add_space(4.0);
                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let events = self.agent_events.clone(); // Clone to avoid borrow checker issues during render
                        for ev in events {
                            ui.add_space(8.0);

                            let event_color = match ev.event_type.as_str() {
                                "ai.session_start" => egui::Color32::from_rgb(57, 255, 20),
                                "ai.tool_use" => egui::Color32::from_rgb(56, 189, 248),
                                "ai.stream_diff" => egui::Color32::from_rgb(250, 204, 21),
                                _ => egui::Color32::from_rgb(200, 200, 200),
                            };

                            ui.label(
                                egui::RichText::new(&ev.event_type)
                                    .color(event_color)
                                    .strong(),
                            );

                            // Render BlockList
                            if let Some(blocks) =
                                ev.payload.get("blocks").and_then(|b| b.as_array())
                            {
                                for block in blocks {
                                    if let Some(content) =
                                        block.get("content").and_then(|c| c.as_str())
                                    {
                                        if block.get("type").and_then(|t| t.as_str())
                                            == Some("code")
                                        {
                                            ui.group(|ui| {
                                                ui.label(
                                                    egui::RichText::new(content)
                                                        .monospace()
                                                        .color(egui::Color32::LIGHT_GRAY),
                                                );
                                                ui.horizontal(|ui| {
                                                    if ui.button("📋 Copy").clicked() {
                                                        ui.output_mut(|o| {
                                                            o.copied_text = content.to_string()
                                                        });
                                                    }
                                                    if ui.button("▶ Run").clicked() {
                                                        // Set pending_pty_injection to inject into terminal
                                                        self.pending_pty_injection =
                                                            Some(format!("{}\n", content));
                                                    }
                                                });
                                            });
                                        } else {
                                            ui.label(content);
                                        }
                                    }
                                }
                            } else {
                                // Fallback rendering
                                ui.label(ev.payload.to_string());
                            }
                            ui.separator();
                        }
                    });
            });
    }

    pub fn render_pythia_hud(&mut self, ctx: &egui::Context) {
        if !self.show_pythia_hud {
            return;
        }

        let screen_rect = ctx.screen_rect();
        let modal_w = (screen_rect.width() * 0.72).clamp(420.0, 750.0);

        let mut close_modal = false;
        let mut do_translate = false;
        let mut pending_image_generation: Option<(String, u32, u32, bool)> = None;

        egui::Window::new("✦ Pythia Cybernetic Translation HUD")
            .id(egui::Id::new("pythia_floating_hud_modal"))
            .open(&mut self.show_pythia_hud)
            .collapsible(false)
            .resizable(true)
            .default_width(modal_w)
            .default_pos(pos2((screen_rect.width() - modal_w) * 0.5, 55.0))
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(12, 16, 26, 250))
                    .stroke(Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(168, 85, 247, 220)))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✦ Pythia Translation Layer").strong().size(14.0).color(Color32::from_rgb(192, 132, 252)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let provider_color = match self.config.pythia_provider.as_str() {
                            "Gemma Abliterated (Hugging Face)" => Color32::from_rgb(250, 204, 21),
                            "Google Gemini 2.0" => Color32::from_rgb(56, 189, 248),
                            "Ollama (Local)" => Color32::from_rgb(57, 255, 20),
                            _ => Color32::from_rgb(168, 85, 247),
                        };
                        ui.label(RichText::new(&self.config.pythia_provider).size(11.0).color(provider_color).strong());
                        ui.label(RichText::new("Engine:").size(11.0).color(Color32::from_rgb(120, 140, 165)));
                    });
                });

                ui.add_space(6.0);

                // Provider quick switcher pills
                ui.horizontal(|ui| {
                    let providers = [
                        ("Gemma Obliterated (HF)", "Gemma Abliterated (Hugging Face)"),
                        ("Gemini 2.0 Flash", "Google Gemini 2.0"),
                        ("Ollama Local", "Ollama (Local)"),
                        ("Offline Rules", "Offline Semantic Rules"),
                    ];
                    for (label, val) in providers {
                        let selected = self.config.pythia_provider == val;
                        let btn = egui::Button::new(RichText::new(label).size(10.0).color(if selected { Color32::BLACK } else { Color32::from_rgb(200, 215, 235) }).strong())
                            .fill(if selected { Color32::from_rgb(192, 132, 252) } else { Color32::from_rgba_premultiplied(22, 28, 44, 200) })
                            .rounding(egui::Rounding::same(4.0));
                        if ui.add(btn).clicked() {
                            self.config.pythia_provider = val.to_string();
                            let mut public_config = self.config.clone();
                            public_config.hf_token.clear();
                            public_config.gemini_api_key.clear();
                            self.channel.sync_state(public_config);
                        }
                    }
                });

                ui.add_space(8.0);

                // Query input row
                let mut do_generate_image = false;
                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::singleline(&mut self.pythia_query)
                        .hint_text("English query (e.g. 'kill port 3000') or '/image <prompt>' for graphics...")
                        .desired_width((ui.available_width() - 310.0).max(120.0));
                    let resp = ui.add(text_edit);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if self.pythia_query.trim().starts_with("/image ") || self.pythia_query.trim().starts_with("/img ") {
                            do_generate_image = true;
                        } else {
                            do_translate = true;
                        }
                    }

                    if ui.button("✂ Cut").on_hover_text("Cut query text to clipboard").clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(self.pythia_query.clone());
                        }
                        self.pythia_query.clear();
                    }

                    if ui.button("📋 Copy").on_hover_text("Copy query text to clipboard").clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(self.pythia_query.clone());
                        }
                    }

                    if ui.button("📋 Paste").on_hover_text("Paste clipboard into query field").clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            if let Ok(text) = cb.get_text() {
                                self.pythia_query = text;
                            }
                        }
                    }

                    let translate_btn = egui::Button::new(RichText::new("✦ Translate").strong().color(Color32::from_rgb(57, 255, 20)))
                        .fill(Color32::from_rgba_premultiplied(18, 42, 28, 220))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(57, 255, 20)))
                        .rounding(egui::Rounding::same(6.0));
                    if ui.add(translate_btn).clicked() {
                        do_translate = true;
                    }

                    let image_btn = egui::Button::new(RichText::new("🖼 Gen Image").strong().color(Color32::from_rgb(192, 132, 252)))
                        .fill(Color32::from_rgba_premultiplied(32, 20, 48, 220))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(192, 132, 252)))
                        .rounding(egui::Rounding::same(6.0));
                    if ui.add(image_btn).clicked() {
                        do_generate_image = true;
                    }
                });

                // One-click quick image generation prompt presets
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Quick Prompts:").size(10.5).color(Color32::from_rgb(120, 140, 165)));
                    let quick_prompts = [
                        ("🖼 Synthwave", "synthwave neon grid wireframe sunset"),
                        ("🌆 Cyber City", "cyberpunk matrix rain city skyline"),
                        ("👾 Retro Icon", "retro phosphor green terminal icon"),
                        ("🌌 Deep Space", "deep space cosmic nebula stars"),
                        ("⚡ Quantum", "quantum computing golden glowing circuit board"),
                    ];
                    for (lbl, p) in quick_prompts {
                        let btn = egui::Button::new(RichText::new(lbl).size(10.0).color(Color32::from_rgb(192, 132, 252)))
                            .fill(Color32::from_rgba_premultiplied(28, 18, 44, 180))
                            .stroke(Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(168, 85, 247, 80)))
                            .rounding(egui::Rounding::same(4.0));
                        if ui.add(btn).clicked() {
                            self.pythia_query = format!("/image {}", p);
                            do_generate_image = true;
                        }
                    }

                    ui.label(RichText::new("Resolution:").size(10.5).color(Color32::from_rgb(120, 140, 165)));
                    let dims = [((512, 512), "512x512"), ((1024, 1024), "1024x1024")];
                    for (dim, label) in dims {
                        let sel = self.pythia_image_dim == dim;
                        let btn = egui::Button::new(RichText::new(label).size(10.0).color(if sel { Color32::BLACK } else { Color32::from_rgb(56, 189, 248) }))
                            .fill(if sel { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgba_premultiplied(16, 28, 44, 180) })
                            .rounding(egui::Rounding::same(4.0));
                        if ui.add(btn).clicked() {
                            self.pythia_image_dim = dim;
                        }
                    }
                });

                let trimmed_query = self.pythia_query.trim();
                let is_image_prefix = trimmed_query.starts_with("/image ") || trimmed_query.starts_with("/img ");
                if (do_translate || do_generate_image || is_image_prefix) && !trimmed_query.is_empty() {
                    let provider = match self.config.pythia_provider.as_str() {
                        "Gemma Abliterated (Hugging Face)" => well_llm::LlmProvider::HuggingFaceGemmaAbliterated,
                        "Google Gemini 2.0" => well_llm::LlmProvider::Gemini,
                        "Ollama (Local)" => well_llm::LlmProvider::Ollama,
                        _ => well_llm::LlmProvider::OfflineRules,
                    };

                    let (actual_prompt, is_image_req) = if trimmed_query.starts_with("/image ") {
                        (trimmed_query.trim_start_matches("/image ").trim().to_string(), true)
                    } else if trimmed_query.starts_with("/img ") {
                        (trimmed_query.trim_start_matches("/img ").trim().to_string(), true)
                    } else if do_generate_image {
                        (trimmed_query.to_string(), true)
                    } else {
                        (trimmed_query.to_string(), false)
                    };

                    let req = well_llm::TranslationRequest {
                        query: if is_image_req { String::new() } else { actual_prompt.clone() },
                        shell: self.config.shell_path.clone(),
                        cwd: std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
                        provider,
                        gemini_api_key: if self.config.gemini_api_key.trim().is_empty() { None } else { Some(self.config.gemini_api_key.trim().to_string()) },
                        gemini_model: Some(self.config.gemini_model.clone()),
                        ollama_url: Some(self.config.ollama_url.clone()),
                        ollama_model: Some(self.config.ollama_model.clone()),
                        hf_token: if self.config.hf_token.trim().is_empty() { None } else { Some(self.config.hf_token.trim().to_string()) },
                        hf_model: Some(self.config.hf_model.clone()),
                        image_prompt: if is_image_req { Some(actual_prompt) } else { None },
                        image_width: if is_image_req { Some(self.pythia_image_dim.0) } else { None },
                        image_height: if is_image_req { Some(self.pythia_image_dim.1) } else { None },
                        force_image_regeneration: false,
                    };

                    self.pythia_destructive_confirmation.clear();
                    if is_image_req {
                        pending_image_generation = Some((
                            req.image_prompt.unwrap_or_default(),
                            self.pythia_image_dim.0,
                            self.pythia_image_dim.1,
                            false,
                        ));
                    } else {
                        self.pythia_result = Some(well_llm::PythiaTranslator::translate(&req));
                    }
                }

                if self.image_generation_in_flight {
                    ui.label(
                        RichText::new("◌ Generating image in the background…")
                            .color(Color32::from_rgb(250, 204, 21))
                            .size(11.0),
                    );
                }

                if let Some(ref res) = self.pythia_result {
                    // Re-evaluate the exact command displayed at the execution boundary. The
                    // provider/result flag is informative, but is never treated as authorization.
                    let command_is_destructive = detect_destructive_command(&res.command);
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Command preview box (only if command was generated)
                    if !res.command.is_empty() {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(8, 10, 18, 240))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(56, 189, 248, 120)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("$").strong().color(Color32::from_rgb(168, 85, 247)).monospace());
                                    ui.label(RichText::new(&res.command).strong().color(Color32::from_rgb(56, 189, 248)).monospace().size(13.0));
                                });
                            });
                        ui.add_space(4.0);
                    }

                    ui.label(RichText::new(&res.explanation).color(Color32::from_rgb(160, 180, 205)).size(11.5));

                    // Show generated image preview with copy and open actions
                    if let Some(ref img_path) = res.generated_image_path {
                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(12, 16, 28, 240))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("🖼 Generated Artifact:").strong().color(Color32::from_rgb(192, 132, 252)).size(12.0));
                                    ui.label(RichText::new(img_path).monospace().color(Color32::from_rgb(148, 163, 184)).size(11.0));
                                });
                                ui.add_space(4.0);
                                // Forget cached texture before loading to ensure fresh PNG read
                                let img_uri = format!("file://{}", img_path);
                                if self.last_image_uri.as_deref() != Some(img_path.as_str()) {
                                    if let Some(ref old_uri) = self.last_image_uri {
                                        ui.ctx().forget_image(old_uri);
                                    }
                                    ui.ctx().forget_image(&img_uri);
                                    self.last_image_uri = Some(img_path.clone());
                                }
                                ui.add(
                                    egui::Image::new(&img_uri)
                                        .max_width(280.0)
                                        .max_height(280.0)
                                        .rounding(egui::Rounding::same(6.0))
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui.button("🔍 Open Image").clicked() {
                                        let _ = std::process::Command::new("open").arg(img_path).spawn();
                                    }
                                    if ui.button("📁 Reveal in Finder").clicked() {
                                        let _ = std::process::Command::new("open").arg("-R").arg(img_path).spawn();
                                    }
                                    if ui.button("⚡ Insert Path to Prompt").clicked() {
                                        self.pending_pty_injection = Some(format!("{}\n", img_path));
                                        close_modal = true;
                                    }
                                    if ui.button("📁 Cache Folder").clicked() {
                                        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                        let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
                                        let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
                                    }
                                    if ui.button("↺ Regenerate").clicked() {
                                        let prompt = self
                                            .pythia_query
                                            .trim_start_matches("/image ")
                                            .trim_start_matches("/img ")
                                            .trim()
                                            .to_string();
                                        if !prompt.is_empty() {
                                            pending_image_generation = Some((
                                                prompt,
                                                self.pythia_image_dim.0,
                                                self.pythia_image_dim.1,
                                                true,
                                            ));
                                        }
                                    }
                                });
                            });
                    }

                    ui.add_space(6.0);
                    if command_is_destructive {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(48, 12, 12, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(239, 68, 68)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("⚠ SAFETY GATE: Potentially destructive disk/file operations detected. Verify thoroughly before execution.").color(Color32::from_rgb(248, 113, 113)).size(10.5));
                            });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Type RUN to enable direct execution:").color(Color32::from_rgb(248, 113, 113)).size(10.5));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.pythia_destructive_confirmation)
                                    .desired_width(72.0)
                                    .hint_text("RUN")
                            );
                        });
                        ui.add_space(4.0);
                    }
                    ui.horizontal(|ui| {
                        if command_is_destructive {
                            ui.label(RichText::new("⚠ DESTRUCTIVE").color(Color32::from_rgb(239, 68, 68)).strong().size(10.5));
                        } else {
                            ui.label(RichText::new("✓ SAFE").color(Color32::from_rgb(57, 255, 20)).strong().size(10.5));
                        }
                        ui.label(RichText::new(format!("Engine: {}", res.provider_used)).color(Color32::from_rgb(140, 160, 185)).size(10.5));
                        ui.label(RichText::new(format!("Confidence: {:.0}%", res.confidence * 100.0)).color(Color32::from_rgb(250, 204, 21)).size(10.5));
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if !res.command.is_empty() {
                            let destructive_confirmed = !command_is_destructive || pythia_destructive_run_confirmed(&self.pythia_destructive_confirmation);
                            let run_label = if command_is_destructive && !destructive_confirmed {
                                "⚠ Type RUN First"
                            } else {
                                "⚡ Run in Shell"
                            };
                            let run_btn = egui::Button::new(RichText::new(run_label).strong().color(Color32::from_rgb(57, 255, 20)))
                                .fill(Color32::from_rgba_premultiplied(18, 42, 28, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(57, 255, 20)))
                                .rounding(egui::Rounding::same(6.0));
                            ui.add_enabled_ui(destructive_confirmed, |ui| {
                                if ui.add(run_btn).on_hover_text("Execute directly in current terminal session").clicked() {
                                    self.pending_pty_injection = Some(format!("{}\n", res.command));
                                    self.pythia_destructive_confirmation.clear();
                                    close_modal = true;
                                }
                            });

                            let insert_btn = egui::Button::new(RichText::new("✏ Insert into Prompt").strong().color(Color32::from_rgb(56, 189, 248)))
                                .fill(Color32::from_rgba_premultiplied(16, 28, 44, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(insert_btn).on_hover_text("Insert command into prompt line for manual editing").clicked() {
                                self.pending_pty_injection = Some(res.command.clone());
                                close_modal = true;
                            }

                            let copy_cmd_btn = egui::Button::new(RichText::new("📋 Copy Command").strong().color(Color32::from_rgb(250, 204, 21)))
                                .fill(Color32::from_rgba_premultiplied(40, 30, 10, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(250, 204, 21)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(copy_cmd_btn).on_hover_text("Copy generated command to OS clipboard").clicked() {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(res.command.clone());
                                }
                            }
                        }

                        let copy_exp_btn = egui::Button::new(RichText::new("📄 Copy Explanation").strong().color(Color32::from_rgb(192, 132, 252)))
                            .fill(Color32::from_rgba_premultiplied(32, 20, 48, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(192, 132, 252)))
                            .rounding(egui::Rounding::same(6.0));
                        if ui.add(copy_exp_btn).on_hover_text("Copy full response/explanation to OS clipboard").clicked() {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(res.explanation.clone());
                            }
                        }

                        if ui.button("✕ Close").clicked() {
                            close_modal = true;
                        }
                    });
                }
            });

        if let Some((prompt, width, height, force)) = pending_image_generation {
            if force {
                self.regenerate_image(&prompt, width, height);
            } else {
                self.trigger_image_generation(&prompt, width, height);
            }
        }

        if close_modal {
            self.show_pythia_hud = false;
        }
    }

    // Helper to list profile file names (without extension)
    fn list_profile_names(&self) -> Vec<String> {
        if let Ok(dir) = profile_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                let mut names = entries
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        if !path.is_file()
                            || path.extension().and_then(|ext| ext.to_str()) != Some("json")
                        {
                            return None;
                        }
                        let stem = path.file_stem().and_then(|s| s.to_str())?;
                        if validate_profile_name(stem).is_ok() {
                            Some(stem.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                names.sort();
                names.dedup();
                return names;
            }
        }
        Vec::new()
    }

    // Save current config as a named profile JSON file
    fn save_profile(&mut self, name: &str) -> Result<(), String> {
        let path = profile_path(name)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create profile directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let json = profile_json_from_panel(self, name)?;
        write_config_atomically(&path, &json)?;
        self.current_profile = Some(name.to_string());
        self.new_profile_name = name.to_string();
        self.config.profile_name = Some(name.to_string());
        self.sync_public_config();
        self.status_notification = Some((format!("Saved profile {}", name), Instant::now()));
        Ok(())
    }

    // Load a named profile and apply it
    fn load_profile(&mut self, name: &str) -> Result<(), String> {
        let path = profile_path(name)?;
        let data = std::fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read profile {}: {error}", path.display()))?;
        let persistent = profile_config_from_json(
            &data,
            name,
            &self.config.hf_token,
            &self.config.gemini_api_key,
        )?;
        self.apply_persistent_config(persistent);
        self.current_profile = Some(name.to_string());
        self.new_profile_name = name.to_string();
        self.status_notification = Some((format!("Loaded profile {}", name), Instant::now()));
        Ok(())
    }

    fn delete_profile(&mut self, name: &str) -> Result<(), String> {
        let path = profile_path(name)?;
        match std::fs::remove_file(&path) {
            Ok(()) => {
                if self.current_profile.as_deref() == Some(name) {
                    self.current_profile = None;
                    self.new_profile_name.clear();
                    self.config.profile_name = None;
                    self.sync_public_config();
                }
                self.status_notification =
                    Some((format!("Deleted profile {}", name), Instant::now()));
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.status_notification = Some((
                    format!("Profile {} was already missing", name),
                    Instant::now(),
                ));
                Ok(())
            }
            Err(error) => Err(format!(
                "Failed to delete profile {}: {error}",
                path.display()
            )),
        }
    }

    pub fn render(&mut self, ctx: &Context) {
        self.render_window(ctx);
    }
}

pub const JETPACK_STARSHIP_PRESET: &str = r#"# version: 1.0.0
"$schema" = 'https://starship.rs/config-schema.json'

add_newline = true
continuation_prompt = "[▸▹ ](dimmed white)"

format = """($nix_shell$container$fill$git_metrics\n)$cmd_duration\
$hostname\
$localip\
$shlvl\
$shell\
$env_var\
$jobs\
$sudo\
$username\
$character"""

right_format = """
$singularity\
$kubernetes\
$directory\
$vcsh\
$fossil_branch\
$git_branch\
$git_commit\
$git_state\
$git_status\
$hg_branch\
$pijul_channel\
$docker_context\
$package\
$c\
$cpp\
$cmake\
$cobol\
$daml\
$dart\
$deno\
$dotnet\
$elixir\
$elm\
$erlang\
$fennel\
$fortran\
$golang\
$guix_shell\
$haskell\
$haxe\
$helm\
$java\
$julia\
$kotlin\
$gradle\
$lua\
$maven\
$nim\
$nodejs\
$bun\
$ocaml\
$opa\
$perl\
$php\
$pulumi\
$purescript\
$python\
$raku\
$rlang\
$red\
$ruby\
$rust\
$scala\
$solidity\
$swift\
$terraform\
$vlang\
$vagrant\
$xmake\
$zig\
$buf\
$conda\
$pixi\
$meson\
$spack\
$memory_usage\
$aws\
$gcloud\
$openstack\
$azure\
$crystal\
$custom\
$status\
$os\
$battery\
$time"""

[fill]
symbol = ' '

[character]
format = "$symbol "
success_symbol = "[◎](bold italic bright-yellow)"
error_symbol = "[○](italic purple)"
vimcmd_symbol = "[■](italic dimmed green)"
vimcmd_replace_one_symbol = "◌"
vimcmd_replace_symbol = "□"
vimcmd_visual_symbol = "▼"

[env_var.VIMSHELL]
format = "[$env_value]($style)"
style = 'green italic'

[sudo]
format = "[$symbol]($style)"
style = "bold italic bright-purple"
symbol = "⋈┈"
disabled = false

[username]
style_user = "bright-yellow bold italic"
style_root = "purple bold italic"
format = "[⭘ $user]($style) "
disabled = false
show_always = false

[directory]
home_symbol = "⌂"
truncation_length = 2
truncation_symbol = "□ "
read_only = " ◈"
use_os_path_sep = true
style = "italic blue"
format = '[$path]($style)[$read_only]($read_only_style)'
repo_root_style = 'bold blue'
repo_root_format = '[$before_root_path]($before_repo_root_style)[$repo_root]($repo_root_style)[$path]($style)[$read_only]($read_only_style) [△](bold bright-blue)'

[cmd_duration]
format = "[◄ $duration ](italic white)"

[jobs]
format = "[$symbol$number]($style) "
style = "white"
symbol = "[▶](blue italic)"

[localip]
ssh_only = true
format = " ◯[$localipv4](bold magenta)"
disabled = false

[time]
disabled = false
format = "[ $time]($style)"
time_format = "%R"
utc_time_offset = "local"
style = "italic dimmed white"

[battery]
format = "[ $percentage $symbol]($style)"
full_symbol = "█"
charging_symbol = "[↑](italic bold green)"
discharging_symbol = "↓"
unknown_symbol = "░"
empty_symbol = "▃"

[[battery.display]]
threshold = 20
style = "italic bold red"

[[battery.display]]
threshold = 60
style = "italic dimmed bright-purple"

[[battery.display]]
threshold = 70
style = "italic dimmed yellow"

[git_branch]
format = " [$branch(:$remote_branch)]($style)"
symbol = "[△](bold italic bright-blue)"
style = "italic bright-blue"
truncation_symbol = "⋯"
truncation_length = 11
ignore_branches = ["main", "master"]
only_attached = true

[git_metrics]
format = '([▴$added]($added_style))([▿$deleted]($deleted_style))'
added_style = 'italic dimmed green'
deleted_style = 'italic dimmed red'
ignore_submodules = true
disabled = false

[git_status]
style = "bold italic bright-blue"
format = "([⎪$ahead_behind$staged$modified$untracked$renamed$deleted$conflicted$stashed⎥]($style))"
conflicted = "[◪◦](italic bright-magenta)"
ahead = "[▴│[${count}](bold white)│](italic green)"
behind = "[▿│[${count}](bold white)│](italic red)"
diverged = "[◇ ▴┤[${ahead_count}](regular white)│▿┤[${behind_count}](regular white)│](italic bright-magenta)"
untracked = "[◌◦](italic bright-yellow)"
stashed = "[◃◈](italic white)"
modified = "[●◦](italic yellow)"
staged = "[▪┤[$count](bold white)│](italic bright-cyan)"
renamed = "[◎◦](italic bright-blue)"
deleted = "[✕](italic red)"

[deno]
format = " [deno](italic) [∫ $version](green bold)"
version_format = "${raw}"

[lua]
format = " [lua](italic) [${symbol}${version}]($style)"
version_format = "${raw}"
symbol = "⨀ "
style = "bold bright-yellow"

[nodejs]
format = " [node](italic) [◫ ($version)](bold bright-green)"
version_format = "${raw}"
detect_files = ["package-lock.json", "yarn.lock"]
detect_folders = ["node_modules"]
detect_extensions = []

[bun]
format = " [bun](italic) [◫ ($version)](bold bright-green)"
version_format = "${raw}"

[python]
format = " [py](italic) [${symbol}${version}]($style)"
symbol = "[⌉](bold bright-blue)⌊ "
version_format = "${raw}"
style = "bold bright-yellow"

[ruby]
format = " [rb](italic) [${symbol}${version}]($style)"
symbol = "◆ "
version_format = "${raw}"
style = "bold red"

[rust]
format = " [rs](italic) [$symbol$version]($style)"
symbol = "⊃ "
version_format = "${raw}"
style = "bold red"

[package]
format = " [pkg](italic dimmed) [$symbol$version]($style)"
version_format = "${raw}"
symbol = "◨ "
style = "dimmed yellow italic bold"

[swift]
format = " [sw](italic) [${symbol}${version}]($style)"
symbol = "◁ "
style = "bold bright-red"
version_format = "${raw}"
"#;

fn starship_preset_cli_name(preset_name: &str) -> Option<&'static str> {
    match preset_name.trim().to_ascii_lowercase().as_str() {
        "jetpack" => Some("jetpack"),
        "tokyo night" => Some("tokyo-night"),
        "pure minimal" => Some("pure-preset"),
        "gruvbox rainbow" => Some("gruvbox-rainbow"),
        "pastel powerline" => Some("pastel-powerline"),
        _ => None,
    }
}

fn starship_config_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("STARSHIP_CONFIG").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or("Could not determine the home directory for Starship")?;
    Ok(PathBuf::from(home).join(".config").join("starship.toml"))
}

fn find_starship_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|directory| directory.join("starship")));
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/starship"),
        PathBuf::from("/usr/local/bin/starship"),
        PathBuf::from("/usr/bin/starship"),
    ]);
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn write_starship_config(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }
    std::fs::write(path, contents)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

fn apply_starship_preset_to_path(
    preset_name: &str,
    starship_path: &Path,
    starship_executable: Option<&Path>,
) -> Result<String, String> {
    let cli_name = starship_preset_cli_name(preset_name)
        .ok_or_else(|| format!("Unsupported Starship preset: {preset_name}"))?;

    if let Some(executable) = starship_executable {
        match std::process::Command::new(executable)
            .args(["preset", cli_name])
            .output()
        {
            Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                write_starship_config(starship_path, &output.stdout)?;
                return Ok(format!(
                    "Activated '{preset_name}' in {}. Press Enter to redraw the prompt.",
                    starship_path.display()
                ));
            }
            Ok(output) if cli_name != "jetpack" => {
                let details = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "Starship could not generate '{preset_name}': {}",
                    details.trim()
                ));
            }
            Err(error) if cli_name != "jetpack" => {
                return Err(format!("Failed to run {}: {error}", executable.display()));
            }
            _ => {}
        }
    } else if cli_name != "jetpack" {
        return Err(
            "Starship CLI was not found in PATH, /opt/homebrew/bin, or /usr/local/bin".to_string(),
        );
    }

    write_starship_config(starship_path, JETPACK_STARSHIP_PRESET.as_bytes())?;
    Ok(format!(
        "Activated embedded Jetpack preset in {}. Press Enter to redraw the prompt.",
        starship_path.display()
    ))
}

pub fn apply_starship_preset(preset_name: &str) -> Result<String, String> {
    let starship_path = starship_config_path()?;
    let starship_executable = find_starship_executable();
    apply_starship_preset_to_path(preset_name, &starship_path, starship_executable.as_deref())
}

/// Paints the cybernetic Oroboros dragon-scale frame around the terminal viewport,
/// including repeating interlocking chevrons along borders and multi-layered armor brackets at corners.
pub fn draw_oroboros_scale_frame(
    painter: &egui::Painter,
    screen_rect: Rect,
    top_bar_h: f32,
    margin: f32,
) {
    let w = screen_rect.width();
    let h = screen_rect.height();
    if w <= 2.0 * margin || h <= top_bar_h + margin {
        return;
    }

    let viewport_rect = Rect::from_min_max(pos2(margin, top_bar_h), pos2(w - margin, h - margin));

    // 1. Dark outer bezel framing the window edges
    let bezel_fill = Color32::from_rgba_premultiplied(10, 13, 19, 250);
    painter.rect_filled(
        Rect::from_min_max(pos2(0.0, top_bar_h), pos2(margin, h)),
        0.0,
        bezel_fill,
    );
    painter.rect_filled(
        Rect::from_min_max(pos2(w - margin, top_bar_h), pos2(w, h)),
        0.0,
        bezel_fill,
    );
    painter.rect_filled(
        Rect::from_min_max(pos2(0.0, h - margin), pos2(w, h)),
        0.0,
        bezel_fill,
    );

    // 2. Inner hairline border enclosing the terminal viewport
    let inner_border_color = Color32::from_rgba_premultiplied(56, 189, 248, 65); // Cyber cyan hint
    painter.rect_stroke(
        viewport_rect,
        egui::Rounding::same(2.0),
        Stroke::new(1.0_f32, inner_border_color),
    );

    // 3. Repeating Ouroboros scales along Left & Right vertical borders
    let scale_step = 16.0;
    let mut y = top_bar_h + 8.0;
    let mut idx = 0;
    while y + scale_step <= h - margin - 4.0 {
        // Color rhythm: alternate between deep gunmetal, electric purple, and cyber cyan edge glows
        let glow_color = match idx % 4 {
            0 => Color32::from_rgba_premultiplied(168, 85, 247, 180), // Neon Purple
            1 => Color32::from_rgba_premultiplied(56, 189, 248, 160), // Electric Cyan
            2 => Color32::from_rgba_premultiplied(192, 132, 252, 140), // Light Violet
            _ => Color32::from_rgba_premultiplied(14, 165, 233, 120), // Deep Cyan
        };

        // Left border scale: Angular chevron plate pointing downwards
        let l_top = pos2(1.0, y);
        let l_mid = pos2(margin - 1.0, y + scale_step * 0.5);
        let l_bot = pos2(1.0, y + scale_step);
        let l_inner = pos2(2.5, y + scale_step * 0.5);

        painter.add(egui::Shape::convex_polygon(
            vec![l_top, l_mid, l_bot, l_inner],
            Color32::from_rgba_premultiplied(18, 24, 36, 220),
            Stroke::new(0.8_f32, glow_color),
        ));

        // Right border scale: Symmetric angular chevron plate
        let r_top = pos2(w - 1.0, y);
        let r_mid = pos2(w - margin + 1.0, y + scale_step * 0.5);
        let r_bot = pos2(w - 1.0, y + scale_step);
        let r_inner = pos2(w - 2.5, y + scale_step * 0.5);

        painter.add(egui::Shape::convex_polygon(
            vec![r_top, r_mid, r_bot, r_inner],
            Color32::from_rgba_premultiplied(18, 24, 36, 220),
            Stroke::new(0.8_f32, glow_color),
        ));

        y += scale_step;
        idx += 1;
    }

    // 4. Repeating Ouroboros scales along Bottom horizontal border
    let mut x = margin + 12.0;
    let b_scale_w = 18.0;
    let mut b_idx = 0;
    while x + b_scale_w <= w - margin - 12.0 {
        let glow_color = match b_idx % 4 {
            0 => Color32::from_rgba_premultiplied(168, 85, 247, 180), // Neon Purple
            1 => Color32::from_rgba_premultiplied(56, 189, 248, 160), // Electric Cyan
            2 => Color32::from_rgba_premultiplied(192, 132, 252, 140), // Light Violet
            _ => Color32::from_rgba_premultiplied(14, 165, 233, 120), // Deep Cyan
        };

        let b_left = pos2(x, h - margin + 1.0);
        let b_tip = pos2(x + b_scale_w * 0.5, h - 1.0);
        let b_right = pos2(x + b_scale_w, h - margin + 1.0);
        let b_inner = pos2(x + b_scale_w * 0.5, h - margin + 2.5);

        painter.add(egui::Shape::convex_polygon(
            vec![b_left, b_tip, b_right, b_inner],
            Color32::from_rgba_premultiplied(18, 24, 36, 220),
            Stroke::new(0.8_f32, glow_color),
        ));

        x += b_scale_w;
        b_idx += 1;
    }

    // 5. Four Corner Armor Scale Plates (Dragon Crest Brackets)
    let corner_size = 14.0;

    // Bottom-Left Corner Crest: overlapping layered scale chevron
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(0.5, h - corner_size * 1.6),
            pos2(corner_size * 1.6, h - 0.5),
            pos2(0.5, h - 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(168, 85, 247, 220)),
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(2.0, h - corner_size),
            pos2(corner_size, h - 2.0),
            pos2(2.0, h - 2.0),
        ],
        Color32::from_rgba_premultiplied(32, 40, 60, 255),
        Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(56, 189, 248, 200)),
    ));
    painter.circle_filled(pos2(4.5, h - 4.5), 1.5, Color32::from_rgb(180, 110, 255));

    // Bottom-Right Corner Crest: symmetric layered scale chevron
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(w - 0.5, h - corner_size * 1.6),
            pos2(w - corner_size * 1.6, h - 0.5),
            pos2(w - 0.5, h - 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(56, 189, 248, 220)),
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(w - 2.0, h - corner_size),
            pos2(w - corner_size, h - 2.0),
            pos2(w - 2.0, h - 2.0),
        ],
        Color32::from_rgba_premultiplied(32, 40, 60, 255),
        Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(168, 85, 247, 200)),
    ));
    painter.circle_filled(pos2(w - 4.5, h - 4.5), 1.5, Color32::from_rgb(56, 189, 248));

    // Top-Left Crest (just below top bar)
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(0.5, top_bar_h + corner_size),
            pos2(corner_size, top_bar_h + 0.5),
            pos2(0.5, top_bar_h + 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(168, 85, 247, 220)),
    ));
    painter.circle_filled(
        pos2(4.5, top_bar_h + 4.5),
        1.5,
        Color32::from_rgb(180, 110, 255),
    );

    // Top-Right Crest (just below top bar)
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(w - 0.5, top_bar_h + corner_size),
            pos2(w - corner_size, top_bar_h + 0.5),
            pos2(w - 0.5, top_bar_h + 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(56, 189, 248, 220)),
    ));
    painter.circle_filled(
        pos2(w - 4.5, top_bar_h + 4.5),
        1.5,
        Color32::from_rgb(56, 189, 248),
    );
}

fn pythia_destructive_run_confirmed(input: &str) -> bool {
    input.trim() == "RUN"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sample_persistent_config() -> PersistentConfig {
        PersistentConfig {
            version: PERSISTENT_CONFIG_VERSION,
            theia: TheiaConfigPayload::default(),
            shell_path: "/bin/zsh".to_string(),
            active_font_name: "JetBrainsMono Nerd Font (Active)".to_string(),
            abbreviations: vec![ShellAbbreviation {
                keyword: "gco".to_string(),
                expansion: "git checkout".to_string(),
            }],
            keybindings: vec![KeybindingRow {
                chord: "Cmd+,".to_string(),
                action: "Toggle Settings Menu".to_string(),
                conflict: false,
            }],
            hf_token: Some("test-token".to_string()),
        }
    }

    #[test]
    fn default_shortcuts_are_runtime_backed() {
        let bindings = default_keybindings();

        assert!(!bindings.is_empty());
        for binding in bindings {
            assert!(ShortcutAction::from_label(&binding.action).is_some());
            assert!(canonical_shortcut_alternatives(&binding.chord).is_some());
        }
    }

    #[test]
    fn shortcut_validation_flags_unknown_keys_actions_and_duplicates() {
        assert!(canonical_shortcut_alternatives("Cmd+NotAKey").is_none());
        assert!(canonical_shortcut_alternatives("Cmd+Shift+P / Ctrl+Shift+P").is_some());

        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        panel.keybindings = vec![
            KeybindingRow {
                chord: "Cmd+K".to_string(),
                action: ShortcutAction::ClearBuffer.label().to_string(),
                conflict: false,
            },
            KeybindingRow {
                chord: "Cmd+K".to_string(),
                action: ShortcutAction::FindInScrollback.label().to_string(),
                conflict: false,
            },
            KeybindingRow {
                chord: "Cmd+D".to_string(),
                action: "Split Pane Horizontal".to_string(),
                conflict: false,
            },
        ];

        panel.refresh_keybinding_conflicts();

        assert!(!panel.keybindings[0].conflict);
        assert!(panel.keybindings[1].conflict);
        assert!(panel.keybindings[2].conflict);
    }

    #[test]
    fn persistent_config_serializes_version() {
        let config = sample_persistent_config();

        let json = config.to_json_string().expect("config should serialize");
        let parsed = PersistentConfig::from_json_str(&json).expect("config should deserialize");

        assert_eq!(parsed.version, PERSISTENT_CONFIG_VERSION);
        assert_eq!(parsed.shell_path, "/bin/zsh");
        assert_eq!(parsed.abbreviations[0].keyword, "gco");
        assert_eq!(parsed.keybindings[0].chord, "Cmd+,");
        assert_eq!(parsed.hf_token, None);
        assert!(!json.contains("test-token"));
    }

    #[test]
    fn persistent_config_accepts_legacy_without_version() {
        let mut value = serde_json::to_value(sample_persistent_config())
            .expect("config should convert to JSON");
        value
            .as_object_mut()
            .expect("persistent config should be a JSON object")
            .remove("version");
        let legacy_json =
            serde_json::to_string_pretty(&value).expect("legacy config should serialize");

        let parsed = PersistentConfig::from_json_str(&legacy_json)
            .expect("legacy config should deserialize");

        assert_eq!(parsed.version, PERSISTENT_CONFIG_VERSION);
        assert_eq!(parsed.active_font_name, "JetBrainsMono Nerd Font (Active)");
    }

    #[test]
    fn persistent_config_migrates_v1_and_rejects_future_versions() {
        let mut value = serde_json::to_value(sample_persistent_config())
            .expect("config should convert to JSON");
        value["version"] = serde_json::Value::from(1);
        let migrated =
            PersistentConfig::from_json_str(&value.to_string()).expect("v1 config should migrate");
        assert_eq!(migrated.version, PERSISTENT_CONFIG_VERSION);

        value["version"] = serde_json::Value::from(PERSISTENT_CONFIG_VERSION + 1);
        let error = PersistentConfig::from_json_str(&value.to_string())
            .expect_err("future config should be rejected");
        assert!(error.contains("newer than supported"));
    }

    #[test]
    fn persistent_config_defaults_missing_fields_and_normalizes_values() {
        let json = r#"{
            "version": 1,
            "theia": {
                "theme_id": 99,
                "background_opacity": -4.0,
                "font_size": 400.0,
                "cursor_style": 18,
                "line_height": 0.1
            },
            "shell_path": ""
        }"#;

        let parsed = PersistentConfig::from_json_str(json)
            .expect("partial config should migrate with defaults");
        assert_eq!(parsed.version, PERSISTENT_CONFIG_VERSION);
        assert_eq!(parsed.theia.theme_id, 3);
        assert_eq!(parsed.theia.background_opacity, 0.05);
        assert_eq!(parsed.theia.font_size, 72.0);
        assert_eq!(parsed.theia.cursor_style, 2);
        assert_eq!(parsed.theia.line_height, 0.8);
        assert!(!parsed.shell_path.is_empty());
        assert_eq!(parsed.theia.shell_path, parsed.shell_path);
    }

    #[test]
    fn persistent_config_rejects_invalid_json() {
        let error = PersistentConfig::from_json_str("{ definitely not json }")
            .expect_err("invalid JSON should fail");

        assert!(error.contains("Failed to parse config"));
    }

    #[test]
    fn completed_image_generation_is_collected_without_blocking() {
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        let (tx, rx) = std::sync::mpsc::channel();
        panel.image_generation_result_rx = Some(rx);
        panel.image_generation_in_flight = true;
        tx.send(well_llm::TranslationResult {
            command: String::new(),
            explanation: "Generated image artifact for prompt: \"test\"".to_string(),
            confidence: 1.0,
            is_destructive: false,
            provider_used: "Test image provider".to_string(),
            fell_back_to_offline: false,
            generated_image_path: Some("/tmp/well-test-image.png".to_string()),
        })
        .expect("result receiver should be active");

        assert!(panel.poll_image_generation());
        assert!(!panel.image_generation_in_flight);
        assert_eq!(
            panel
                .pythia_result
                .as_ref()
                .and_then(|result| result.generated_image_path.as_deref()),
            Some("/tmp/well-test-image.png")
        );
        assert!(!panel.poll_image_generation());
    }

    #[test]
    fn image_artifact_manifest_sorts_prunes_and_round_trips() {
        let temp_dir =
            std::env::temp_dir().join(format!("well-image-manifest-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let older_path = temp_dir.join("older.png");
        let newer_path = temp_dir.join("newer.png");
        std::fs::write(&older_path, b"older").expect("older artifact should be written");
        std::fs::write(&newer_path, b"newer").expect("newer artifact should be written");

        let artifacts = vec![
            ImageArtifactRecord {
                path: older_path.to_string_lossy().to_string(),
                prompt: "older prompt".to_string(),
                provider: "Test Provider".to_string(),
                width: 512,
                height: 512,
                created_at_unix: 10,
            },
            ImageArtifactRecord {
                path: temp_dir.join("missing.png").to_string_lossy().to_string(),
                prompt: "missing prompt".to_string(),
                provider: "Test Provider".to_string(),
                width: 512,
                height: 512,
                created_at_unix: 30,
            },
            ImageArtifactRecord {
                path: newer_path.to_string_lossy().to_string(),
                prompt: "newer prompt".to_string(),
                provider: "Test Provider".to_string(),
                width: 1024,
                height: 1024,
                created_at_unix: 20,
            },
        ];

        save_image_artifacts_to_dir(&temp_dir, &artifacts)
            .expect("manifest should save successfully");
        let loaded = load_image_artifacts_from_dir(&temp_dir);

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].prompt, "newer prompt");
        assert_eq!(loaded[1].prompt, "older prompt");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn generated_image_delete_guard_requires_cache_containment() {
        let temp_dir =
            std::env::temp_dir().join(format!("well-image-delete-test-{}", std::process::id()));
        let cache_dir = temp_dir.join("cache");
        let outside_dir = temp_dir.join("outside");
        std::fs::create_dir_all(&cache_dir).expect("cache dir should be created");
        std::fs::create_dir_all(&outside_dir).expect("outside dir should be created");
        let inside = cache_dir.join("image.png");
        let outside = outside_dir.join("image.png");
        std::fs::write(&inside, b"inside").expect("inside artifact should be written");
        std::fs::write(&outside, b"outside").expect("outside artifact should be written");

        assert!(image_artifact_is_inside_cache(&cache_dir, &inside));
        assert!(!image_artifact_is_inside_cache(&cache_dir, &outside));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn load_from_path_applies_persistent_config() {
        let temp_dir =
            std::env::temp_dir().join(format!("well-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("config.json");
        let mut config = sample_persistent_config();
        config.shell_path = "/usr/local/bin/fish".to_string();
        std::fs::write(
            &path,
            config.to_json_string().expect("config should serialize"),
        )
        .expect("config should be written");

        let loaded_config = load_persistent_config_from_path(&path)
            .expect("config should load")
            .expect("config should exist");
        assert_eq!(loaded_config.shell_path, "/usr/local/bin/fish");

        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);

        let loaded = panel.load_from_path(&path).expect("config should load");

        assert!(loaded);
        assert_eq!(panel.shell_path, "/usr/local/bin/fish");
        assert_eq!(panel.channel.read_state().shell_path, "/usr/local/bin/fish");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn load_persistent_config_from_path_treats_missing_config_as_first_run() {
        let missing_path = std::env::temp_dir().join(format!(
            "well-missing-persistent-config-test-{}.json",
            std::process::id()
        ));

        let config = load_persistent_config_from_path(&missing_path)
            .expect("missing configuration should not be an error");

        assert!(config.is_none());
    }

    #[test]
    fn load_from_path_reports_missing_file_without_changing_config() {
        let missing_path = std::env::temp_dir().join(format!(
            "well-missing-config-test-{}.json",
            std::process::id()
        ));
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        let original_shell_path = panel.shell_path.clone();

        let loaded = panel
            .load_from_path(&missing_path)
            .expect("missing config is not an error");

        assert!(!loaded);
        assert_eq!(panel.shell_path, original_shell_path);
    }

    #[test]
    fn load_from_path_rejects_invalid_config_without_changing_state() {
        let temp_dir =
            std::env::temp_dir().join(format!("well-invalid-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("config.json");
        std::fs::write(&path, "{ not valid json").expect("invalid config should be written");
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        let original = panel.config.clone();

        let error = panel
            .load_from_path(&path)
            .expect_err("invalid config should fail safely");

        assert!(error.contains("Failed to parse config"));
        assert_eq!(panel.config.theme_id, original.theme_id);
        assert_eq!(panel.config.font_size, original.font_size);
        assert_eq!(panel.channel.read_state().theme_id, original.theme_id);

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn save_to_path_creates_parent_directory_and_writes_versioned_config() {
        let temp_dir =
            std::env::temp_dir().join(format!("well-save-config-test-{}", std::process::id()));
        let path = temp_dir.join("nested").join("config.json");
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        panel.shell_path = "/opt/homebrew/bin/fish".to_string();
        panel.config.hf_token = "secret-hf-token".to_string();
        panel.config.gemini_api_key = "secret-gemini-key".to_string();
        panel.hf_token = "legacy-panel-token".to_string();

        panel.save_to_path(&path).expect("config should save");

        let saved = std::fs::read_to_string(&path).expect("saved config should be readable");
        let parsed = PersistentConfig::from_json_str(&saved).expect("saved config should parse");
        assert_eq!(parsed.version, PERSISTENT_CONFIG_VERSION);
        assert_eq!(parsed.shell_path, "/opt/homebrew/bin/fish");
        assert_eq!(parsed.hf_token, None);
        assert!(parsed.theia.hf_token.is_empty());
        assert!(parsed.theia.gemini_api_key.is_empty());
        assert!(!saved.contains("secret-hf-token"));
        assert!(!saved.contains("secret-gemini-key"));
        assert!(!saved.contains("legacy-panel-token"));
        let temporary_files = std::fs::read_dir(path.parent().unwrap())
            .expect("config directory should be readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temporary_files, 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path)
                .expect("config metadata should be readable")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(temp_dir.join("nested"));
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn hyprlang_file_round_trip_preserves_secrets_and_rejects_malformed_input() {
        let temp_dir =
            std::env::temp_dir().join(format!("well-hyprlang-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("well.hl");
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        panel.config.theme_id = 2;
        panel.config.font_size = 17.25;
        panel.config.shell_path = "/bin/fish".to_string();
        panel.config.hf_token = "runtime-hf-secret".to_string();
        panel.config.gemini_api_key = "runtime-gemini-secret".to_string();
        panel.shell_path = panel.config.shell_path.clone();

        panel
            .export_hyprlang_to_path(&path)
            .expect("Hyprlang export should succeed");
        let exported = std::fs::read_to_string(&path).expect("export should be readable");
        assert!(!exported.contains("runtime-hf-secret"));
        assert!(!exported.contains("runtime-gemini-secret"));

        panel.config.theme_id = 0;
        panel.config.font_size = 10.0;
        panel
            .import_hyprlang_from_path(&path)
            .expect("Hyprlang import should succeed");
        assert_eq!(panel.config.theme_id, 2);
        assert_eq!(panel.config.font_size, 17.25);
        assert_eq!(panel.config.shell_path, "/bin/fish");
        assert_eq!(panel.config.hf_token, "runtime-hf-secret");
        assert_eq!(panel.config.gemini_api_key, "runtime-gemini-secret");

        let stable_theme = panel.config.theme_id;
        std::fs::write(&path, "appearance {\n theme_id = 3")
            .expect("malformed Hyprlang should be written");
        assert!(panel.import_hyprlang_from_path(&path).is_err());
        assert_eq!(panel.config.theme_id, stable_theme);

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn load_from_path_accepts_legacy_hf_token_fields() {
        let temp_dir = std::env::temp_dir().join(format!(
            "well-legacy-token-config-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("config.json");
        let mut config = sample_persistent_config();
        config.hf_token = Some("legacy-wrapper-token".to_string());
        config.theia.hf_token = "legacy-theia-token".to_string();
        let mut legacy = serde_json::to_value(&config).expect("legacy config should serialize");
        legacy["hf_token"] = serde_json::Value::from("legacy-wrapper-token");
        legacy["theia"]["hf_token"] = serde_json::Value::from("legacy-theia-token");
        std::fs::write(&path, legacy.to_string()).expect("config should be written");

        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);

        panel.load_from_path(&path).expect("config should load");

        assert_eq!(panel.config.hf_token, "legacy-theia-token");
        assert_eq!(panel.hf_token, "legacy-theia-token");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn load_from_path_accepts_older_wrapper_hf_token() {
        let temp_dir = std::env::temp_dir().join(format!(
            "well-wrapper-token-config-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("config.json");
        let mut config = sample_persistent_config();
        config.hf_token = Some("legacy-wrapper-token".to_string());
        config.theia.hf_token.clear();
        let mut legacy = serde_json::to_value(&config).expect("legacy config should serialize");
        legacy["hf_token"] = serde_json::Value::from("legacy-wrapper-token");
        std::fs::write(&path, legacy.to_string()).expect("config should be written");

        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);

        panel.load_from_path(&path).expect("config should load");

        assert_eq!(panel.config.hf_token, "legacy-wrapper-token");
        assert_eq!(panel.hf_token, "legacy-wrapper-token");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn save_to_path_reports_parent_directory_creation_failure() {
        let temp_dir = std::env::temp_dir().join(format!(
            "well-save-config-parent-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let file_parent = temp_dir.join("not-a-directory");
        std::fs::write(&file_parent, "not a directory").expect("parent file should be written");
        let path = file_parent.join("config.json");
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);

        let error = panel
            .save_to_path(&path)
            .expect_err("saving below a file should fail");

        assert!(error.contains("Failed to create config directory"));

        let _ = std::fs::remove_file(file_parent);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn pythia_destructive_confirmation_requires_exact_run_token() {
        assert!(pythia_destructive_run_confirmed("RUN"));
        assert!(pythia_destructive_run_confirmed("  RUN  "));
        assert!(!pythia_destructive_run_confirmed(""));
        assert!(!pythia_destructive_run_confirmed("run"));
        assert!(!pythia_destructive_run_confirmed("RUN rm -rf /"));
    }

    #[test]
    fn hermes_sync_never_exposes_provider_credentials() {
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(Arc::clone(&channel));
        panel.config.hf_token = "hf-secret".to_string();
        panel.config.gemini_api_key = "gemini-secret".to_string();

        panel.sync_public_config();

        let shared = channel.read_state();
        assert!(shared.hf_token.is_empty());
        assert!(shared.gemini_api_key.is_empty());
        assert_eq!(panel.config.hf_token, "hf-secret");
        assert_eq!(panel.config.gemini_api_key, "gemini-secret");
    }

    #[test]
    fn profile_names_cannot_escape_the_profile_directory() {
        assert!(validate_profile_name("work").is_ok());
        assert!(validate_profile_name("night-shift_2").is_ok());
        assert!(validate_profile_name("Demo Profile").is_ok());
        for invalid in [
            "",
            ".",
            "..",
            ".hidden",
            " leading",
            "trailing ",
            "too:punctuated",
            "../outside",
            "nested/profile",
            "nested\\profile",
        ] {
            assert!(
                validate_profile_name(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn profile_json_uses_versioned_persistent_config_without_credentials() {
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let mut panel = TheiasPrismPanel::new(channel);
        panel.config.theme_id = 2;
        panel.config.font_size = 18.0;
        panel.config.hf_token = "profile-hf-secret".to_string();
        panel.config.gemini_api_key = "profile-gemini-secret".to_string();
        panel.shell_path = "/bin/fish".to_string();
        panel.active_font_name = "JetBrainsMono Nerd Font (Active)".to_string();
        panel.abbreviations = vec![ShellAbbreviation {
            keyword: "gs".to_string(),
            expansion: "git status".to_string(),
        }];
        panel.keybindings = vec![KeybindingRow {
            chord: "Cmd+K".to_string(),
            action: ShortcutAction::ClearBuffer.label().to_string(),
            conflict: false,
        }];

        let json =
            profile_json_from_panel(&panel, "Demo Profile").expect("profile should serialize");

        assert!(json.contains("\"version\""));
        assert!(json.contains("\"theia\""));
        assert!(json.contains("\"keybindings\""));
        assert!(json.contains("\"abbreviations\""));
        assert!(!json.contains("profile-hf-secret"));
        assert!(!json.contains("profile-gemini-secret"));

        let parsed = profile_config_from_json(
            &json,
            "Demo Profile",
            "runtime-hf-secret",
            "runtime-gemini-secret",
        )
        .expect("profile should parse");

        assert_eq!(parsed.theia.profile_name.as_deref(), Some("Demo Profile"));
        assert_eq!(parsed.theia.theme_id, 2);
        assert_eq!(parsed.theia.font_size, 18.0);
        assert_eq!(parsed.shell_path, "/bin/fish");
        assert_eq!(parsed.active_font_name, "JetBrainsMono Nerd Font (Active)");
        assert_eq!(parsed.abbreviations[0].keyword, "gs");
        assert_eq!(
            parsed.keybindings[0].action,
            ShortcutAction::ClearBuffer.label()
        );
        assert_eq!(parsed.theia.hf_token, "runtime-hf-secret");
        assert_eq!(parsed.theia.gemini_api_key, "runtime-gemini-secret");
    }

    #[test]
    fn legacy_theia_only_profile_json_still_loads() {
        let legacy = TheiaConfigPayload {
            theme_id: 3,
            font_size: 20.0,
            shell_path: "/bin/zsh".to_string(),
            font_name: "System Monospace".to_string(),
            hf_token: "legacy-hf-secret".to_string(),
            gemini_api_key: "legacy-gemini-secret".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&legacy).expect("legacy profile should serialize");

        let parsed = profile_config_from_json(
            &json,
            "Legacy",
            "runtime-hf-secret",
            "runtime-gemini-secret",
        )
        .expect("legacy profile should parse");

        assert_eq!(parsed.theia.profile_name.as_deref(), Some("Legacy"));
        assert_eq!(parsed.theia.theme_id, 3);
        assert_eq!(parsed.theia.font_size, 20.0);
        assert_eq!(parsed.shell_path, "/bin/zsh");
        assert_eq!(parsed.theia.hf_token, "runtime-hf-secret");
        assert_eq!(parsed.theia.gemini_api_key, "runtime-gemini-secret");
        assert!(parsed.keybindings.is_empty());
        assert!(parsed.abbreviations.is_empty());
    }

    #[test]
    fn starship_preset_names_map_to_supported_cli_names() {
        assert_eq!(starship_preset_cli_name("Jetpack"), Some("jetpack"));
        assert_eq!(starship_preset_cli_name("Tokyo Night"), Some("tokyo-night"));
        assert_eq!(
            starship_preset_cli_name("Pure Minimal"),
            Some("pure-preset")
        );
        assert_eq!(
            starship_preset_cli_name("Gruvbox Rainbow"),
            Some("gruvbox-rainbow")
        );
        assert_eq!(
            starship_preset_cli_name("Pastel Powerline"),
            Some("pastel-powerline")
        );
        assert_eq!(starship_preset_cli_name("unknown"), None);
    }

    #[test]
    fn embedded_jetpack_preset_writes_without_starship_cli() {
        let temp_dir = std::env::temp_dir().join(format!(
            "well-starship-embedded-test-{}",
            std::process::id()
        ));
        let config_path = temp_dir.join("nested/starship.toml");

        let message = apply_starship_preset_to_path("Jetpack", &config_path, None)
            .expect("embedded Jetpack preset should apply");
        let contents = std::fs::read_to_string(&config_path).expect("preset should be written");

        assert!(contents.contains("$schema"));
        assert!(contents.contains("[character]"));
        assert!(message.contains("Press Enter"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[cfg(unix)]
    #[test]
    fn starship_preset_uses_resolved_cli_and_requested_config_path() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir =
            std::env::temp_dir().join(format!("well-starship-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp directory should be created");
        let executable = temp_dir.join("starship");
        let config_path = temp_dir.join("custom/config.toml");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf '[character]\\nsuccess_symbol = \\\"[ok](green)\\\"\\n'\n",
        )
        .expect("fake Starship should be written");
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();

        let message = apply_starship_preset_to_path("Tokyo Night", &config_path, Some(&executable))
            .expect("CLI-backed preset should apply");
        let contents = std::fs::read_to_string(&config_path).expect("preset should be written");

        assert!(contents.contains("success_symbol"));
        assert!(message.contains("Tokyo Night"));
        assert!(message.contains(config_path.to_string_lossy().as_ref()));

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
