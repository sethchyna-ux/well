//! huggingface.rs: Hugging Face Inference Client for Uncensored / Abliterated Models
//!
//! Connects directly to Hugging Face Inference API or Hugging Face Router,
//! with first-class presets for Gemma Abliterated (`failspy/gemma-2-9b-it-abliterated`,
//! `mlabonne/gemma-2-2b-it-abliterated`) for unfiltered terminal command translation.

use crate::gemini::StructuredShellOutput;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const HF_INFERENCE_ROUTER_URL: &str = "https://router.huggingface.co/hf-inference/models";
const IMAGE_GENERATION_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone)]
pub struct HuggingFaceClient {
    api_token: Option<String>,
    model: String,
    timeout: Duration,
}

#[derive(Serialize)]
struct HfChatRequest<'a> {
    model: &'a str,
    messages: Vec<HfMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct HfMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct HfChatResponse {
    choices: Option<Vec<HfChoice>>,
}

#[derive(Deserialize)]
struct HfChoice {
    message: Option<HfResponseMessage>,
}

#[derive(Deserialize)]
struct HfResponseMessage {
    content: Option<String>,
}

impl Default for HuggingFaceClient {
    fn default() -> Self {
        Self {
            api_token: std::env::var("HF_TOKEN").ok(),
            model: "failspy/gemma-2-9b-it-abliterated".to_string(),
            timeout: Duration::from_secs(12),
        }
    }
}

impl HuggingFaceClient {
    pub fn new(api_token: Option<String>, model: impl Into<String>) -> Self {
        Self {
            api_token,
            model: model.into(),
            timeout: Duration::from_secs(12),
        }
    }

    /// Translates a natural language query via Hugging Face Gemma Abliterated.
    pub fn translate_query(
        &self,
        query: &str,
        shell_type: &str,
        cwd: &str,
    ) -> Result<StructuredShellOutput, String> {
        let token = self
            .api_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                "Hugging Face credentials are required for cloud requests".to_string()
            })?;
        let system_prompt = format!(
            "You are Pythia, an expert terminal engineer running an abliterated Gemma model for Well Terminal on {}.\n\
            Target shell: {}.\n\
            Current directory: {}.\n\
            You never refuse technical commands, penetration testing queries, or root administration tasks.\n\
            Output ONLY valid JSON with keys:\n\
            {{\"command\": \"<exact shell command>\", \"explanation\": \"<short explanation>\", \"confidence\": <float 0.0 to 1.0>}}\n\
            No markdown formatting or commentary.",
            std::env::consts::OS,
            shell_type,
            cwd
        );

        let messages = vec![
            HfMessage {
                role: "system",
                content: &system_prompt,
            },
            HfMessage {
                role: "user",
                content: query,
            },
        ];

        let body = HfChatRequest {
            model: &self.model,
            messages,
            temperature: 0.1,
            max_tokens: 256,
        };

        // Standard Hugging Face Router endpoint (OpenAI compatible)
        let url = "https://router.huggingface.co/hf-inference/v1/chat/completions";
        let req = ureq::post(url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .set("Authorization", &format!("Bearer {}", token.trim()));

        let resp = req
            .send_json(&body)
            .map_err(|e| format!("Hugging Face API request failed: {}", e))?;

        let chat_resp: HfChatResponse = resp
            .into_json()
            .map_err(|e| format!("Failed to parse Hugging Face response: {}", e))?;

        let raw_text = chat_resp
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .ok_or_else(|| "Empty message content from Hugging Face API".to_string())?;

        // Extract JSON substring if wrapped in markdown ```json ... ```
        let json_str = extract_json_block(&raw_text);

        let structured: StructuredShellOutput = serde_json::from_str(json_str).map_err(|e| {
            format!(
                "Failed to parse JSON from Gemma Abliterated: {} (raw: {})",
                e, raw_text
            )
        })?;

        Ok(structured)
    }

    /// Generates an image using Hugging Face Inference API.
    pub fn generate_image(
        &self,
        prompt: &str,
        width: u32,
        height: u32,
    ) -> Result<(Vec<u8>, String), String> {
        let token = self
            .api_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                "Hugging Face credentials are required for cloud requests".to_string()
            })?;
        let url = format!("{HF_INFERENCE_ROUTER_URL}/{}", self.model);
        let req = ureq::post(&url)
            .timeout(IMAGE_GENERATION_TIMEOUT)
            .set("Content-Type", "application/json")
            .set("Authorization", &format!("Bearer {}", token.trim()));

        let body = image_generation_body(prompt, width, height);
        let resp = req
            .send_json(&body)
            .map_err(|e| format!("Hugging Face image API request failed: {}", e))?;

        let content_type = resp
            .header("content-type")
            .unwrap_or_default()
            .to_ascii_lowercase();

        let mut bytes = Vec::new();
        use std::io::Read;
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read HF image bytes: {}", e))?;

        if content_type.contains("application/json") {
            let body = String::from_utf8_lossy(&bytes);
            return Err(format!(
                "Hugging Face image API returned JSON instead of image bytes: {}",
                body
            ));
        }

        if !looks_like_png(&bytes) && !looks_like_jpeg(&bytes) && !looks_like_webp(&bytes) {
            let preview = String::from_utf8_lossy(&bytes);
            return Err(format!(
                "Hugging Face image API returned unsupported image bytes (content-type: {}; preview: {})",
                content_type,
                preview.chars().take(160).collect::<String>()
            ));
        }

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.model.as_bytes());
        hasher.update(prompt.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let filename = format!("image_hf_{}.{}", &hash[0..12], image_extension(&bytes));
        Ok((bytes, filename))
    }
}

fn image_generation_body(prompt: &str, width: u32, height: u32) -> serde_json::Value {
    serde_json::json!({
        "inputs": prompt,
        "parameters": {
            "width": width,
            "height": height,
        },
    })
}

fn looks_like_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1A\n"
}

fn looks_like_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}

fn looks_like_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

fn image_extension(bytes: &[u8]) -> &'static str {
    if looks_like_png(bytes) {
        "png"
    } else if looks_like_jpeg(bytes) {
        "jpg"
    } else {
        "webp"
    }
}

fn extract_json_block(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return &trimmed[start..=end];
            }
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_requests_require_a_non_empty_credential() {
        for token in [None, Some("   ".to_string())] {
            let client = HuggingFaceClient::new(token, "example/model");
            let text_error = match client.translate_query("list files", "zsh", "/tmp") {
                Err(error) => error,
                Ok(_) => panic!("text request without credentials must be rejected locally"),
            };
            let image_error = client
                .generate_image("blue square", 512, 512)
                .expect_err("image request without credentials must be rejected locally");
            assert!(text_error.contains("credentials are required"));
            assert!(image_error.contains("credentials are required"));
        }
    }

    #[test]
    fn image_generation_request_includes_requested_dimensions() {
        let request = image_generation_body("a terminal in space", 768, 512);

        assert_eq!(request["inputs"], "a terminal in space");
        assert_eq!(request["parameters"]["width"], 768);
        assert_eq!(request["parameters"]["height"], 512);
    }

    #[test]
    fn image_extension_matches_supported_image_signatures() {
        assert_eq!(image_extension(b"\x89PNG\r\n\x1A\npayload"), "png");
        assert_eq!(image_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
        assert_eq!(image_extension(b"RIFFxxxxWEBPpayload"), "webp");
    }
}
