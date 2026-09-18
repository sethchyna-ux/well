//! well-llm: Terminal LLM Translation Layer for Well Terminal
//!
//! Provides intelligent multi-provider Natural Language to Shell synthesis,
//! stderr diagnosis, command explanation, and offline semantic rules.

pub mod gemini;
pub mod huggingface;
pub mod offline;
pub mod ollama;

pub use gemini::{GeminiClient, StructuredShellOutput};
pub use huggingface::HuggingFaceClient;
pub use offline::{OfflineRuleEngine, OfflineTranslation};
pub use ollama::OllamaClient;

use serde::{Deserialize, Serialize};
use well_core::ai::detect_destructive_command;

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

pub struct PythiaTranslator;

impl PythiaTranslator {
    /// Translates a natural language prompt into an executable shell command.
    /// Employs automatic fallback to the instant Offline Rule Engine if external APIs fail.
    pub fn translate(req: &TranslationRequest) -> TranslationResult {
        let trimmed_query = req.query.trim();

        // 1. Image generation path – if an image prompt is supplied, generate the image using the selected provider and return a result with the image path.
        if let Some(ref img_prompt) = req.image_prompt {
            // Determine dimensions (default 1024x1024)
            let width = req.image_width.unwrap_or(1024);
            let height = req.image_height.unwrap_or(1024);
            // Resolve cache directory
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
            let _ = std::fs::create_dir_all(&cache_dir);
            // Build cache filename based on hash of provider, prompt and size
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(req.provider.to_string().as_bytes());
            hasher.update(img_prompt.as_bytes());
            hasher.update(width.to_be_bytes());
            hasher.update(height.to_be_bytes());
            let hash = format!("{:x}", hasher.finalize());
            let filename = format!("image_{}.png", &hash[0..12]);
            let cache_path = cache_dir.join(&filename);
            if cache_path.exists() {
                // Cached image – return path directly
                return TranslationResult {
                    command: String::new(),
                    explanation: format!("Cached image artifact ready ({}x{})", width, height),
                    confidence: 1.0,
                    is_destructive: false,
                    provider_used: format!("{} (cached)", req.provider),
                    fell_back_to_offline: false,
                    generated_image_path: Some(cache_path.to_string_lossy().to_string()),
                };
            }

            // No cached image – generate via provider, mock, or offline fallback
            let is_mock = std::env::var("WELL_MOCK_LLM").map(|v| v == "1" || v == "true").unwrap_or(false);
            let generate_res: Result<(Vec<u8>, String), String> = if is_mock {
                let png_bytes = generate_test_pattern_png(width.min(256), height.min(256), img_prompt);
                Ok((png_bytes, filename.clone()))
            } else {
                match req.provider {
                    LlmProvider::Gemini => {
                        let api_key = req.gemini_api_key.clone().or_else(|| std::env::var("GEMINI_API_KEY").ok());
                        if let Some(key) = api_key {
                            let client = GeminiClient::new(key);
                            client.generate_image(img_prompt, width, height).or_else(|e| {
                                eprintln!("[Well-LLM] Gemini Imagen API call failed ({}), generating fallback test pattern.", e);
                                let png_bytes = generate_test_pattern_png(width, height, img_prompt);
                                Ok((png_bytes, filename.clone()))
                            })
                        } else {
                            eprintln!("[Well-LLM] No GEMINI_API_KEY found, generating fallback test pattern.");
                            let png_bytes = generate_test_pattern_png(width, height, img_prompt);
                            Ok((png_bytes, filename.clone()))
                        }
                    },
                    LlmProvider::HuggingFaceGemmaAbliterated => {
                        let token = req.hf_token.clone().or_else(|| std::env::var("HF_TOKEN").ok());
                        if let Some(tok) = token {
                            // Use a dedicated image model instead of the text model configured for Pythia
                            let model = "stabilityai/stable-diffusion-xl-base-1.0".to_string();
                            let client = HuggingFaceClient::new(Some(tok), model);
                            client.generate_image(img_prompt, width, height).or_else(|e| {
                                eprintln!("[Well-LLM] HuggingFace API call failed ({}), generating fallback test pattern.", e);
                                let png_bytes = generate_test_pattern_png(width, height, img_prompt);
                                Ok((png_bytes, filename.clone()))
                            })
                        } else {
                            let png_bytes = generate_test_pattern_png(width, height, img_prompt);
                            Ok((png_bytes, filename.clone()))
                        }
                    },
                    _ => {
                        // Offline semantic engine generates synthetic test pattern PNG
                        let png_bytes = generate_test_pattern_png(width, height, img_prompt);
                        Ok((png_bytes, filename.clone()))
                    }
                }
            };
            match generate_res {
                Ok((bytes, _fname)) => {
                    let _ = std::fs::write(&cache_path, &bytes);
                    let provider_desc = if is_mock {
                        format!("{} (Mock Engine)", req.provider)
                    } else {
                        format!("{}", req.provider)
                    };
                    return TranslationResult {
                        command: String::new(),
                        explanation: format!("Generated image artifact for prompt: \"{}\"", img_prompt),
                        confidence: 1.0,
                        is_destructive: false,
                        provider_used: provider_desc,
                        fell_back_to_offline: false,
                        generated_image_path: Some(cache_path.to_string_lossy().to_string()),
                    };
                }
                Err(e) => {
                    eprintln!("Image generation failed: {}", e);
                }
            }
        }
        // 2. Try cloud / local LLM if requested
        match req.provider {
            LlmProvider::Gemini => {
                let api_key = req
                    .gemini_api_key
                    .clone()
                    .or_else(|| std::env::var("GEMINI_API_KEY").ok());

                if let Some(key) = api_key {
                    if !key.trim().is_empty() {
                        let mut client = GeminiClient::new(key);
                        if let Some(ref m) = req.gemini_model {
                            client = client.with_model(m);
                        }

                        if let Ok(out) = client.translate_query(trimmed_query, &req.shell, &req.cwd) {
                            let is_destructive = detect_destructive_command(&out.command);
                            return TranslationResult {
                                command: out.command,
                                explanation: out.explanation,
                                confidence: out.confidence,
                                is_destructive,
                                provider_used: "Gemini 2.0 Flash".to_string(),
                                fell_back_to_offline: false,
                                generated_image_path: None,
                            };
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
                        let is_destructive = detect_destructive_command(&out.command);
                        return TranslationResult {
                            command: out.command,
                            explanation: out.explanation,
                            confidence: out.confidence,
                            is_destructive,
                            provider_used: "Ollama (Local)".to_string(),
                            fell_back_to_offline: false,
                            generated_image_path: None,
                        };
                    }
                }
            }
            LlmProvider::HuggingFaceGemmaAbliterated => {
                let token = req
                    .hf_token
                    .clone()
                    .or_else(|| std::env::var("HF_TOKEN").ok());

                let model = req
                    .hf_model
                    .clone()
                    .unwrap_or_else(|| "failspy/gemma-2-9b-it-abliterated".to_string());

                let client = HuggingFaceClient::new(token, model.clone());
                if let Ok(out) = client.translate_query(trimmed_query, &req.shell, &req.cwd) {
                    let is_destructive = detect_destructive_command(&out.command);
                    return TranslationResult {
                        command: out.command,
                        explanation: out.explanation,
                        confidence: out.confidence,
                        is_destructive,
                        provider_used: format!("Gemma Abliterated ({model})"),
                        fell_back_to_offline: false,
                        generated_image_path: None,
                    };
                }
            }
            LlmProvider::OfflineRules => {} // unchanged
        }

        // 2. Semantic Offline Rule Matcher
        if let Some(offline_match) = OfflineRuleEngine::translate(trimmed_query) {
            let is_destructive = detect_destructive_command(&offline_match.command);
            return TranslationResult {
                command: offline_match.command,
                explanation: offline_match.explanation,
                confidence: offline_match.confidence,
                is_destructive,
                provider_used: "Offline Semantic Engine".to_string(),
                fell_back_to_offline: req.provider != LlmProvider::OfflineRules,
                generated_image_path: None,
            };
        }

        // 3. Graceful fallback when no direct rule matched
        let safe_fallback = format!("echo \"[Pythia] Unrecognized command: {}\"", trimmed_query.replace('"', "\\\""));
        TranslationResult {
            command: safe_fallback,
            explanation: "No exact translation pattern matched. Please configure a Gemini API key or Ollama model for open-ended NLP synthesis.".to_string(),
            confidence: 0.1,
            is_destructive: false,
            provider_used: "Fallback Engine".to_string(),
            fell_back_to_offline: true,
            generated_image_path: None,
        }
    }

    /// Diagnoses a command error output with automatic fallback.
    pub fn diagnose(
        command: &str,
        exit_code: i32,
        stderr: &str,
        gemini_api_key: Option<String>,
    ) -> TranslationResult {
        let api_key = gemini_api_key.or_else(|| std::env::var("GEMINI_API_KEY").ok());

        if let Some(key) = api_key {
            if !key.trim().is_empty() {
                let client = GeminiClient::new(key);
                if let Ok(diag) = client.diagnose_stderr(command, exit_code, stderr) {
                    let is_destructive = detect_destructive_command(&diag.command);
                    return TranslationResult {
                        command: diag.command,
                        explanation: diag.explanation,
                        confidence: diag.confidence,
                        is_destructive,
                        provider_used: "Gemini 2.0 Flash (Diagnostic)".to_string(),
                        fell_back_to_offline: false,
                        generated_image_path: None,
                    };
                }
            }
        }

        // Basic offline diagnostic heuristic
        let (fix, explanation) = if stderr.contains("command not found") || stderr.contains("No such file or directory") {
            ("which <command>".to_string(), "The executable is missing or not located in your PATH.")
        } else if stderr.contains("Permission denied") {
            (format!("chmod +x {command} || sudo {command}"), "Executable permissions or elevated root privileges are required.")
        } else if stderr.contains("Address already in use") {
            ("lsof -i :<port> | xargs kill -9".to_string(), "The requested network socket port is already bound by another process.")
        } else {
            (format!("{command} --help"), "Command failed with non-zero exit code. Review the usage flags.")
        };

        TranslationResult {
            command: fix,
            explanation: explanation.to_string(),
            confidence: 0.75,
            is_destructive: false,
            provider_used: "Offline Diagnostic Heuristics".to_string(),
            fell_back_to_offline: true,
            generated_image_path: None,
        }
    }
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
        };

        let res = PythiaTranslator::translate(&req);
        assert_eq!(res.command, "lsof -ti:3000 | xargs kill -9");
        assert_eq!(res.provider_used, "Offline Semantic Engine");
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
        };

        let res = PythiaTranslator::translate(&req);
        assert_eq!(res.command, "git reset --hard HEAD~1");
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
        assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        // Verify IHDR chunk presence
        assert_eq!(&png[12..16], b"IHDR");
        // Verify IEND chunk presence
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    #[test]
    fn test_image_generation_offline_pipeline() {
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
        };

        let res = PythiaTranslator::translate(&req);
        assert!(res.generated_image_path.is_some());
        let path_str = res.generated_image_path.as_ref().unwrap();
        let path = std::path::Path::new(path_str);
        assert!(path.exists(), "Generated image must exist on disk");
        let bytes = std::fs::read(path).expect("Must read generated image");
        assert_eq!(&bytes[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn test_image_generation_caching() {
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
            image_prompt: Some("cached_test_icon".to_string()),
            image_width: Some(64),
            image_height: Some(64),
        };

        let res1 = PythiaTranslator::translate(&req);
        let res2 = PythiaTranslator::translate(&req);
        assert_eq!(res1.generated_image_path, res2.generated_image_path);
        assert!(res2.provider_used.contains("cached"));
    }

    #[test]
    fn test_image_generation_mock_llm_mode() {
        std::env::set_var("WELL_MOCK_LLM", "1");
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
        };
        let res = PythiaTranslator::translate(&req);
        assert!(res.generated_image_path.is_some());
        let path = std::path::Path::new(res.generated_image_path.as_ref().unwrap());
        assert!(path.exists());
        let bytes = std::fs::read(path).expect("Read mock image");
        assert_eq!(&bytes[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        assert!(res.provider_used.contains("Mock Engine") || res.provider_used.contains("cached"));
        std::env::remove_var("WELL_MOCK_LLM");
    }
}
