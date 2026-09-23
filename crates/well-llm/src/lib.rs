//! well-llm: Terminal LLM Translation Layer for Well Terminal
//!
//! Provides intelligent multi-provider Natural Language to Shell synthesis,
//! stderr diagnosis, command explanation, and offline semantic rules.

pub mod gemini;
pub mod huggingface;
pub mod keychain;
pub mod offline;
pub mod ollama;

pub use gemini::{GeminiClient, StructuredShellOutput};
pub use huggingface::HuggingFaceClient;
pub use keychain::{
    delete_secret, get_secret, resolve_gemini_api_key, resolve_hf_token, set_secret,
};
pub use offline::{OfflineRuleEngine, OfflineTranslation};
pub use ollama::OllamaClient;

use serde::{Deserialize, Serialize};
use well_core::ai::detect_destructive_command;

const IMAGE_CACHE_VERSION: &str = "v2";
const HF_IMAGE_MODEL: &str = "stabilityai/stable-diffusion-3-medium-diffusers";
const IMAGE_CACHE_EXTENSIONS: [&str; 3] = ["png", "jpg", "webp"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LlmProvider {
    Gemini,
    Ollama,
    HuggingFaceGemmaAbliterated,
    #[default]
    OfflineRules,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gemini => write!(f, "Gemini"),
            Self::Ollama => write!(f, "Ollama"),
            Self::HuggingFaceGemmaAbliterated => write!(f, "Hugging Face (Gemma Abliterated)"),
            Self::OfflineRules => write!(f, "Offline Rules"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub query: String,
    pub shell: String,
    pub cwd: String,
    pub provider: LlmProvider,
    pub gemini_api_key: Option<String>,
    pub gemini_model: Option<String>,
    pub ollama_url: Option<String>,
    pub ollama_model: Option<String>,
    pub hf_token: Option<String>,
    pub hf_model: Option<String>,
    // New optional fields for image generation
    pub image_prompt: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    #[serde(default)]
    pub force_image_regeneration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub command: String,
    pub explanation: String,
    pub confidence: f32,
    pub is_destructive: bool,
    pub provider_used: String,
    pub fell_back_to_offline: bool,
    // New field for image generation
    pub generated_image_path: Option<String>,
}

impl TranslationResult {
    /// Build a command-bearing result from the exact command that will be shown or executed.
    /// Keeping detection here prevents individual provider and fallback paths from bypassing it.
    fn for_command(
        command: String,
        explanation: String,
        confidence: f32,
        provider_used: String,
        fell_back_to_offline: bool,
    ) -> Self {
        let is_destructive = detect_destructive_command(&command);
        Self {
            command,
            explanation,
            confidence,
            is_destructive,
            provider_used,
            fell_back_to_offline,
            generated_image_path: None,
        }
    }

    fn for_image_artifact(
        path: std::path::PathBuf,
        provider_used: String,
        explanation: String,
    ) -> Self {
        Self {
            command: String::new(),
            explanation,
            confidence: 1.0,
            is_destructive: false,
            provider_used,
            fell_back_to_offline: false,
            generated_image_path: Some(path.to_string_lossy().to_string()),
        }
    }

    fn for_image_failure(provider_used: String, explanation: String) -> Self {
        Self {
            command: String::new(),
            explanation,
            confidence: 0.0,
            is_destructive: false,
            provider_used,
            fell_back_to_offline: false,
            generated_image_path: None,
        }
    }
}

struct GeneratedImage {
    bytes: Vec<u8>,
    extension: &'static str,
    provider_used: String,
}

fn image_cache_directory() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
    std::fs::create_dir_all(&cache_dir).map_err(|error| {
        format!(
            "Unable to create the generated-image cache {}: {error}",
            cache_dir.display()
        )
    })?;
    Ok(cache_dir)
}

fn image_cache_key(
    provider: &LlmProvider,
    prompt: &str,
    width: u32,
    height: u32,
    is_mock: bool,
) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(IMAGE_CACHE_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(provider.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(if is_mock {
        b"mock".as_slice()
    } else {
        b"provider".as_slice()
    });
    hasher.update(b"\0");
    hasher.update(prompt.as_bytes());
    hasher.update(width.to_be_bytes());
    hasher.update(height.to_be_bytes());
    format!("{:x}", hasher.finalize())
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 8 && &bytes[..8] == b"\x89PNG\r\n\x1A\n" {
        Some("png")
    } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some("jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn cached_image_path(cache_dir: &std::path::Path, cache_key: &str) -> Option<std::path::PathBuf> {
    IMAGE_CACHE_EXTENSIONS.iter().find_map(|extension| {
        let path = cache_dir.join(format!(
            "image_{IMAGE_CACHE_VERSION}_{cache_key}.{extension}"
        ));
        let bytes = std::fs::read(&path).ok()?;
        image_extension(&bytes).map(|_| path)
    })
}

fn write_image_artifact(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Invalid image artifact path {}", path.display()))?;
    let temporary_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));

    let result = (|| {
        std::fs::write(&temporary_path, bytes).map_err(|error| {
            format!(
                "Unable to write temporary image artifact {}: {error}",
                temporary_path.display()
            )
        })?;
        std::fs::rename(&temporary_path, path).map_err(|error| {
            format!(
                "Unable to finalize image artifact {}: {error}",
                path.display()
            )
        })
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result
}

fn generate_image(
    req: &TranslationRequest,
    prompt: &str,
    width: u32,
    height: u32,
    is_mock: bool,
) -> Result<GeneratedImage, String> {
    if is_mock {
        return Ok(GeneratedImage {
            bytes: generate_test_pattern_png(width.min(256), height.min(256), prompt),
            extension: "png",
            provider_used: format!("{} (mock image)", req.provider),
        });
    }

    match &req.provider {
        LlmProvider::Gemini => {
            let api_key = resolve_gemini_api_key(req.gemini_api_key.as_deref())
                .ok_or_else(|| {
                    "Gemini image generation requires a Gemini API key. Add one in Pythia settings, macOS Keychain, or set GEMINI_API_KEY."
                        .to_string()
                })?;
            let (bytes, _) = GeminiClient::new(api_key).generate_image(prompt, width, height)?;
            let extension = image_extension(&bytes).ok_or_else(|| {
                "Gemini returned an unsupported image format; no artifact was saved.".to_string()
            })?;
            Ok(GeneratedImage {
                bytes,
                extension,
                provider_used: "Gemini 3.1 Flash Image".to_string(),
            })
        }
        LlmProvider::HuggingFaceGemmaAbliterated => {
            let token = resolve_hf_token(req.hf_token.as_deref())
                .ok_or_else(|| {
                    "Hugging Face image generation requires an HF token with Inference Providers access. Add one in settings, macOS Keychain, or set HF_TOKEN."
                        .to_string()
                })?;
            let (bytes, _) =
                HuggingFaceClient::new(Some(token), HF_IMAGE_MODEL).generate_image(prompt, width, height)?;
            let extension = image_extension(&bytes).ok_or_else(|| {
                "Hugging Face returned an unsupported image format; no artifact was saved.".to_string()
            })?;
            Ok(GeneratedImage {
                bytes,
                extension,
                provider_used: "Hugging Face Inference (Stable Diffusion 3 Medium)".to_string(),
            })
        }
        LlmProvider::Ollama => Err(
            "Ollama command translation is configured, but this Well build has no local image-generation backend. Select Gemini or Hugging Face for images."
                .to_string(),
        ),
        LlmProvider::OfflineRules => Err(
            "Offline Rules does not synthesize AI images. Select Gemini or Hugging Face and configure its credential."
                .to_string(),
        ),
    }
}

pub struct PythiaTranslator;

impl PythiaTranslator {
    /// Translates a natural language prompt into an executable shell command.
    /// Employs automatic fallback to the instant Offline Rule Engine if external APIs fail.
    pub fn translate(req: &TranslationRequest) -> TranslationResult {
        let trimmed_query = req.query.trim();

        // 1. Image generation path. A real provider must return real image bytes;
        // production requests never masquerade a local test pattern as generated art.
        if let Some(img_prompt) = req.image_prompt.as_deref() {
            let img_prompt = img_prompt.trim();
            if img_prompt.is_empty() {
                return TranslationResult::for_image_failure(
                    format!("{} (image request rejected)", req.provider),
                    "Enter a prompt before requesting an image.".to_string(),
                );
            }

            let width = req.image_width.unwrap_or(1024);
            let height = req.image_height.unwrap_or(1024);
            if width == 0 || height == 0 {
                return TranslationResult::for_image_failure(
                    format!("{} (image request rejected)", req.provider),
                    "Image dimensions must both be greater than zero.".to_string(),
                );
            }

            let is_mock = std::env::var("WELL_MOCK_LLM")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let cache_dir = match image_cache_directory() {
                Ok(cache_dir) => cache_dir,
                Err(error) => {
                    return TranslationResult::for_image_failure(
                        format!("{} (image cache unavailable)", req.provider),
                        error,
                    );
                }
            };
            let cache_key = image_cache_key(&req.provider, img_prompt, width, height, is_mock);
            if !req.force_image_regeneration {
                if let Some(path) = cached_image_path(&cache_dir, &cache_key) {
                    let provider_used = if is_mock {
                        format!("{} (mock image, cached)", req.provider)
                    } else {
                        format!("{} (cached)", req.provider)
                    };
                    return TranslationResult::for_image_artifact(
                        path,
                        provider_used,
                        format!("Cached image artifact ready ({}x{}).", width, height),
                    );
                }
            }

            let image = match generate_image(req, img_prompt, width, height, is_mock) {
                Ok(image) => image,
                Err(error) => {
                    eprintln!("[Well-LLM] Image generation failed: {error}");
                    return TranslationResult::for_image_failure(
                        format!("{} (image generation failed)", req.provider),
                        error,
                    );
                }
            };
            let path = cache_dir.join(format!(
                "image_{IMAGE_CACHE_VERSION}_{cache_key}.{}",
                image.extension
            ));
            if let Err(error) = write_image_artifact(&path, &image.bytes) {
                return TranslationResult::for_image_failure(
                    format!("{} (image artifact failed)", image.provider_used),
                    error,
                );
            }

            let explanation = if is_mock {
                format!(
                    "Created a test-only mock PNG for \"{}\" (WELL_MOCK_LLM is enabled).",
                    img_prompt
                )
            } else {
                format!("Generated image artifact for prompt: \"{}\"", img_prompt)
            };
            return TranslationResult::for_image_artifact(path, image.provider_used, explanation);
        }
        // 2. Try cloud / local LLM if requested
        match req.provider {
            LlmProvider::Gemini => {
                let api_key = resolve_gemini_api_key(req.gemini_api_key.as_deref());

                if let Some(key) = api_key {
                    if !key.trim().is_empty() {
                        let mut client = GeminiClient::new(key);
                        if let Some(ref m) = req.gemini_model {
                            client = client.with_model(m);
                        }

                        if let Ok(out) = client.translate_query(trimmed_query, &req.shell, &req.cwd)
                        {
                            return TranslationResult::for_command(
                                out.command,
                                out.explanation,
                                out.confidence,
                                "Gemini 2.0 Flash".to_string(),
                                false,
                            );
                        }
                    }
                }
            }
            LlmProvider::Ollama => {
                let base_url = req
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string());
                let model = req
                    .ollama_model
                    .clone()
                    .unwrap_or_else(|| "qwen2.5-coder".to_string());

                let client = OllamaClient::new(base_url, model);
                if client.is_available() {
                    if let Ok(out) = client.translate_query(trimmed_query, &req.shell, &req.cwd) {
                        return TranslationResult::for_command(
                            out.command,
                            out.explanation,
                            out.confidence,
                            "Ollama (Local)".to_string(),
                            false,
                        );
                    }
                }
            }
            LlmProvider::HuggingFaceGemmaAbliterated => {
                let token = resolve_hf_token(req.hf_token.as_deref());

                let model = req
                    .hf_model
                    .clone()
                    .unwrap_or_else(|| "failspy/gemma-2-9b-it-abliterated".to_string());

                if let Some(token) = token.filter(|token| !token.trim().is_empty()) {
                    let client = HuggingFaceClient::new(Some(token), model.clone());
                    if let Ok(out) = client.translate_query(trimmed_query, &req.shell, &req.cwd) {
                        return TranslationResult::for_command(
                            out.command,
                            out.explanation,
                            out.confidence,
                            format!("Gemma Abliterated ({model})"),
                            false,
                        );
                    }
                }
            }
            LlmProvider::OfflineRules => {} // unchanged
        }

        // 2. Semantic Offline Rule Matcher
        if let Some(offline_match) = OfflineRuleEngine::translate(trimmed_query) {
            return TranslationResult::for_command(
                offline_match.command,
                offline_match.explanation,
                offline_match.confidence,
                "Offline Semantic Engine".to_string(),
                req.provider != LlmProvider::OfflineRules,
            );
        }

        // 3. Graceful fallback when no direct rule matched
        let safe_fallback = format!(
            "printf '%s\\n' {}",
            shell_single_quote(&format!("[Pythia] Unrecognized command: {trimmed_query}"))
        );
        TranslationResult::for_command(
            safe_fallback,
            "No exact translation pattern matched. Please configure a Gemini API key or Ollama model for open-ended NLP synthesis.".to_string(),
            0.1,
            "Fallback Engine".to_string(),
            true,
        )
    }

    /// Diagnoses a command error output with automatic fallback.
    pub fn diagnose(
        command: &str,
        exit_code: i32,
        stderr: &str,
        gemini_api_key: Option<String>,
    ) -> TranslationResult {
        let api_key = resolve_gemini_api_key(gemini_api_key.as_deref());

        if let Some(key) = api_key {
            if !key.trim().is_empty() {
                let client = GeminiClient::new(key);
                if let Ok(diag) = client.diagnose_stderr(command, exit_code, stderr) {
                    return TranslationResult::for_command(
                        diag.command,
                        diag.explanation,
                        diag.confidence,
                        "Gemini 2.0 Flash (Diagnostic)".to_string(),
                        false,
                    );
                }
            }
        }

        // Basic offline diagnostic heuristic
        let (fix, explanation) = if stderr.contains("command not found")
            || stderr.contains("No such file or directory")
        {
            (
                "which <command>".to_string(),
                "The executable is missing or not located in your PATH.",
            )
        } else if stderr.contains("Permission denied") {
            (
                format!("chmod +x {command} || sudo {command}"),
                "Executable permissions or elevated root privileges are required.",
            )
        } else if stderr.contains("Address already in use") {
            (
                "lsof -i :<port> | xargs kill -9".to_string(),
                "The requested network socket port is already bound by another process.",
            )
        } else {
            (
                format!("{command} --help"),
                "Command failed with non-zero exit code. Review the usage flags.",
            )
        };

        TranslationResult::for_command(
            fix,
            explanation.to_string(),
            0.75,
            "Offline Diagnostic Heuristics".to_string(),
            true,
        )
    }
}

/// Quote arbitrary text as one POSIX-shell word. Embedded single quotes are represented by
/// ending the quoted region, inserting an escaped quote, and reopening it.
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Generates a valid, pure-Rust standalone PNG test pattern without external graphics dependencies.
/// Uses standard Deflate uncompressed blocks (RFC 1951) and zlib Adler32/CRC-32 checksums.
pub fn generate_test_pattern_png(width: u32, height: u32, prompt: &str) -> Vec<u8> {
    let w = width.clamp(16, 512) as usize;
    let h = height.clamp(16, 512) as usize;

    // Scanline: 1 byte filter (0 = None) + 4 bytes per pixel (RGBA)
    let row_len = 1 + w * 4;
    let mut raw_data = Vec::with_capacity(h * row_len);

    // Hash prompt for deterministic color palette accents
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    let hash_bytes = hasher.finalize();
    let accent_r = hash_bytes[0].max(64);
    let accent_g = hash_bytes[1].max(64);
    let accent_b = hash_bytes[2].max(64);

    for y in 0..h {
        raw_data.push(0); // Filter type 0 (None)
        let is_grid_y = y % 16 == 0;
        let is_border_y = y == 0 || y == h - 1;
        for x in 0..w {
            let is_grid_x = x % 16 == 0;
            let is_border_x = x == 0 || x == w - 1;

            if is_border_x || is_border_y {
                // Neon border
                raw_data.extend_from_slice(&[accent_r, accent_g, accent_b, 255]);
            } else if is_grid_x || is_grid_y {
                // Subtle grid
                raw_data.extend_from_slice(&[30, 41, 59, 255]); // #1E293B
            } else {
                // Dark cyber canvas gradient
                let gradient = (((x + y) * 20) / (w + h)) as u8;
                raw_data.extend_from_slice(&[11 + gradient, 15 + gradient, 25 + gradient, 255]);
            }
        }
    }

    // Wrap raw_data in zlib stream (RFC 1950) with uncompressed Deflate blocks (RFC 1951)
    let mut zlib = Vec::new();
    zlib.push(0x78); // CMF: Deflate, 32K window
    zlib.push(0x01); // FLG: FCHECK=1 (0x7801 % 31 == 0)

    let mut offset = 0;
    while offset < raw_data.len() {
        let chunk_size = (raw_data.len() - offset).min(65535);
        let is_final = (offset + chunk_size) == raw_data.len();
        zlib.push(if is_final { 0x01 } else { 0x00 }); // BFINAL and BTYPE=00 (uncompressed)
        let len_u16 = chunk_size as u16;
        let nlen_u16 = !len_u16;
        zlib.extend_from_slice(&len_u16.to_le_bytes());
        zlib.extend_from_slice(&nlen_u16.to_le_bytes());
        zlib.extend_from_slice(&raw_data[offset..offset + chunk_size]);
        offset += chunk_size;
    }

    // Adler32 checksum over uncompressed raw_data
    let adler = adler32(&raw_data);
    zlib.extend_from_slice(&adler.to_be_bytes());

    // Build PNG file
    let mut png = Vec::new();
    // Signature
    png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR chunk (13 bytes)
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr_data.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr_data.push(8); // Bit depth
    ihdr_data.push(6); // Color type: RGBA (6)
    ihdr_data.push(0); // Compression: deflate (0)
    ihdr_data.push(0); // Filter: standard (0)
    ihdr_data.push(0); // Interlace: none (0)
    write_png_chunk(&mut png, b"IHDR", &ihdr_data);

    // IDAT chunk
    write_png_chunk(&mut png, b"IDAT", &zlib);

    // IEND chunk
    write_png_chunk(&mut png, b"IEND", &[]);

    png
}

fn adler32(data: &[u8]) -> u32 {
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    let start_crc = out.len();
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let crc = crc32(&out[start_crc..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static MOCK_IMAGE_ENV_LOCK: Mutex<()> = Mutex::new(());

    struct RestoreMockImageEnvironment(Option<OsString>);

    impl Drop for RestoreMockImageEnvironment {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                std::env::set_var("WELL_MOCK_LLM", previous);
            } else {
                std::env::remove_var("WELL_MOCK_LLM");
            }
        }
    }

    fn enable_mock_image_generation() -> RestoreMockImageEnvironment {
        let previous = std::env::var_os("WELL_MOCK_LLM");
        std::env::set_var("WELL_MOCK_LLM", "1");
        RestoreMockImageEnvironment(previous)
    }

    fn disable_mock_image_generation() -> RestoreMockImageEnvironment {
        let previous = std::env::var_os("WELL_MOCK_LLM");
        std::env::remove_var("WELL_MOCK_LLM");
        RestoreMockImageEnvironment(previous)
    }

    #[test]
    fn test_offline_translation_dispatch() {
        let req = TranslationRequest {
            query: "kill process on port 3000".to_string(),
            shell: "zsh".to_string(),
            cwd: "/Users/yocan/Desktop/well".to_string(),
            provider: LlmProvider::OfflineRules,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: None,
            image_width: None,
            image_height: None,
            force_image_regeneration: false,
        };

        let res = PythiaTranslator::translate(&req);
        assert_eq!(res.command, "lsof -ti:3000 | xargs kill -9");
        assert_eq!(res.provider_used, "Offline Semantic Engine");
        assert!(res.is_destructive);
        assert!(!res.fell_back_to_offline);
    }

    #[test]
    fn test_destructive_command_flagging() {
        let req = TranslationRequest {
            query: "undo last commit completely".to_string(),
            shell: "bash".to_string(),
            cwd: "/".to_string(),
            provider: LlmProvider::OfflineRules,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: None,
            image_width: None,
            image_height: None,
            force_image_regeneration: false,
        };

        let res = PythiaTranslator::translate(&req);
        assert_eq!(res.command, "git reset --hard HEAD~1");
        assert!(res.is_destructive);
    }

    #[test]
    fn test_offline_diagnostics_derive_safety_from_the_final_command() {
        let permission = PythiaTranslator::diagnose("./tool", 126, "Permission denied", None);
        assert_eq!(permission.command, "chmod +x ./tool || sudo ./tool");
        assert!(permission.is_destructive);
        assert_eq!(
            permission.is_destructive,
            detect_destructive_command(&permission.command)
        );

        let occupied =
            PythiaTranslator::diagnose("server --port 8080", 1, "Address already in use", None);
        assert_eq!(occupied.command, "lsof -i :<port> | xargs kill -9");
        assert!(occupied.is_destructive);
        assert_eq!(
            occupied.is_destructive,
            detect_destructive_command(&occupied.command)
        );

        let missing = PythiaTranslator::diagnose("missing", 127, "command not found", None);
        assert_eq!(missing.command, "which <command>");
        assert!(!missing.is_destructive);
        assert_eq!(
            missing.is_destructive,
            detect_destructive_command(&missing.command)
        );
    }

    #[test]
    fn test_unknown_query_fallback_shell_quotes_untrusted_input() {
        let query = "opaque $(printf SUBSTITUTED >&2) `printf BACKTICK >&2` ' \" newline\ntext";
        let req = TranslationRequest {
            query: query.to_string(),
            shell: "sh".to_string(),
            cwd: "/tmp".to_string(),
            provider: LlmProvider::OfflineRules,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: None,
            image_width: None,
            image_height: None,
            force_image_regeneration: false,
        };

        let result = PythiaTranslator::translate(&req);
        assert_eq!(result.provider_used, "Fallback Engine");
        assert!(!result.is_destructive);
        assert_eq!(
            result.is_destructive,
            detect_destructive_command(&result.command)
        );

        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&result.command)
            .output()
            .expect("fallback command should execute in the system shell");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("[Pythia] Unrecognized command: {query}\n")
        );
    }

    #[test]
    fn test_huggingface_gemma_fallback_to_offline() {
        let req = TranslationRequest {
            query: "kill process on port 8080".to_string(),
            shell: "zsh".to_string(),
            cwd: "/Users/yocan/Desktop/well".to_string(),
            provider: LlmProvider::HuggingFaceGemmaAbliterated,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: Some("invalid_dummy_token".to_string()),
            hf_model: Some("failspy/gemma-2-9b-it-abliterated".to_string()),
            image_prompt: None,
            image_width: None,
            image_height: None,
            force_image_regeneration: false,
        };

        let res = PythiaTranslator::translate(&req);
        // Because the dummy token will fail to connect or auth, it gracefully falls back to offline engine!
        assert_eq!(res.command, "lsof -ti:8080 | xargs kill -9");
        assert!(res.fell_back_to_offline);
    }

    #[test]
    fn test_png_test_pattern_generation() {
        let png = generate_test_pattern_png(64, 64, "retro terminal wallpaper");
        // Verify valid PNG signature
        assert!(png.len() > 8);
        assert_eq!(
            &png[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        // Verify IHDR chunk presence
        assert_eq!(&png[12..16], b"IHDR");
        // Verify IEND chunk presence
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    #[test]
    fn test_image_generation_offline_pipeline() {
        let _mock_env_lock = MOCK_IMAGE_ENV_LOCK.lock().expect("mock env lock");
        let _mock_env = disable_mock_image_generation();
        let req = TranslationRequest {
            query: String::new(),
            shell: "zsh".to_string(),
            cwd: "/Users/yocan/Desktop/well".to_string(),
            provider: LlmProvider::OfflineRules,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: Some("synthwave terminal grid".to_string()),
            image_width: Some(128),
            image_height: Some(128),
            force_image_regeneration: false,
        };

        let res = PythiaTranslator::translate(&req);
        assert!(res.generated_image_path.is_none());
        assert_eq!(res.confidence, 0.0);
        assert!(res.provider_used.contains("image generation failed"));
        assert!(res.explanation.contains("does not synthesize AI images"));
    }

    #[test]
    fn test_image_generation_caching() {
        let _mock_env_lock = MOCK_IMAGE_ENV_LOCK.lock().expect("mock env lock");
        let _mock_env = enable_mock_image_generation();
        let req = TranslationRequest {
            query: String::new(),
            shell: "zsh".to_string(),
            cwd: "/Users/yocan/Desktop/well".to_string(),
            provider: LlmProvider::Gemini,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: Some(format!("cached_test_icon_{}", std::process::id())),
            image_width: Some(64),
            image_height: Some(64),
            force_image_regeneration: false,
        };

        let res1 = PythiaTranslator::translate(&req);
        let res2 = PythiaTranslator::translate(&req);
        assert_eq!(res1.generated_image_path, res2.generated_image_path);
        assert!(res2.provider_used.contains("cached"));
    }

    #[test]
    fn test_forced_image_generation_bypasses_the_cache() {
        let _mock_env_lock = MOCK_IMAGE_ENV_LOCK.lock().expect("mock env lock");
        let _mock_env = enable_mock_image_generation();
        let mut req = TranslationRequest {
            query: String::new(),
            shell: "zsh".to_string(),
            cwd: "/Users/yocan/Desktop/well".to_string(),
            provider: LlmProvider::Gemini,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: Some(format!("forced_refresh_{}", std::process::id())),
            image_width: Some(64),
            image_height: Some(64),
            force_image_regeneration: false,
        };

        let cached_or_created = PythiaTranslator::translate(&req);
        req.force_image_regeneration = true;
        let refreshed = PythiaTranslator::translate(&req);

        assert_eq!(
            cached_or_created.generated_image_path,
            refreshed.generated_image_path
        );
        assert!(!refreshed.provider_used.contains("cached"));
        assert!(refreshed.provider_used.contains("mock image"));
    }

    #[test]
    fn test_image_generation_mock_llm_mode() {
        let _mock_env_lock = MOCK_IMAGE_ENV_LOCK.lock().expect("mock env lock");
        let _mock_env = enable_mock_image_generation();
        let req = TranslationRequest {
            query: String::new(),
            shell: "zsh".to_string(),
            cwd: "/Users/yocan/Desktop/well".to_string(),
            provider: LlmProvider::Gemini,
            gemini_api_key: None,
            gemini_model: None,
            ollama_url: None,
            ollama_model: None,
            hf_token: None,
            hf_model: None,
            image_prompt: Some("mock_matrix_flow".to_string()),
            image_width: Some(32),
            image_height: Some(32),
            force_image_regeneration: false,
        };
        let res = PythiaTranslator::translate(&req);
        assert!(res.generated_image_path.is_some());
        let path = std::path::Path::new(res.generated_image_path.as_ref().unwrap());
        assert!(path.exists());
        let bytes = std::fs::read(path).expect("Read mock image");
        assert_eq!(
            &bytes[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        assert!(res.provider_used.contains("mock image"));
    }
}
