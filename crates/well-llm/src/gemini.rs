//! gemini.rs: Google Gemini LLM API Client for Terminal Translation
//!
//! Supports Gemini 2.0 Flash / 1.5 Flash for natural language command synthesis,
//! stderr diagnosis, and command explanation.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const GEMINI_IMAGE_MODEL: &str = "gemini-3.1-flash-image";
const GEMINI_INTERACTIONS_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/interactions";
const IMAGE_GENERATION_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone)]
pub struct GeminiClient {
    api_key: String,
    model: String,
    timeout: Duration,
}

#[derive(Serialize)]
struct GeminiRequest<'a> {
    contents: Vec<GeminiContent<'a>>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent<'a>>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent<'a> {
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiPart<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    #[serde(rename = "responseMimeType")]
    response_mime_type: &'static str,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Option<Vec<GeminiCandidatePart>>,
}

#[derive(Deserialize)]
struct GeminiCandidatePart {
    text: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct StructuredShellOutput {
    pub command: String,
    pub explanation: String,
    pub confidence: f32,
}

impl GeminiClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gemini-2.0-flash".to_string(),
            timeout: Duration::from_secs(8),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Translates a natural language query into an executable shell command.
    pub fn translate_query(
        &self,
        query: &str,
        shell_type: &str,
        cwd: &str,
    ) -> Result<StructuredShellOutput, String> {
        let system_prompt = format!(
            "You are Pythia, an expert terminal AI engine for Well Terminal on {}.\n\
            Target shell: {}.\n\
            Current directory: {}.\n\
            Given the user's natural language request, output ONLY a valid JSON object with fields:\n\
            {{\"command\": \"<exact executable shell command>\", \"explanation\": \"<short 1-line explanation>\", \"confidence\": <float 0.0 to 1.0>}}\n\
            Do NOT include markdown formatting or backticks.",
            std::env::consts::OS,
            shell_type,
            cwd
        );

        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: query }],
            }],
            system_instruction: Some(GeminiContent {
                parts: vec![GeminiPart {
                    text: &system_prompt,
                }],
            }),
            generation_config: GeminiGenerationConfig {
                temperature: 0.1,
                response_mime_type: "application/json",
            },
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let response = ureq::post(&url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("Gemini API request failed: {}", e))?;

        let gemini_resp: GeminiResponse = response
            .into_json()
            .map_err(|e| format!("Failed to parse Gemini response JSON: {}", e))?;

        let raw_text = gemini_resp
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|p| p.into_iter().next())
            .and_then(|p| p.text)
            .ok_or_else(|| "Empty response received from Gemini API".to_string())?;

        let structured: StructuredShellOutput = serde_json::from_str(&raw_text).map_err(|e| {
            format!(
                "Failed to parse structured output from Gemini: {} (raw: {})",
                e, raw_text
            )
        })?;

        Ok(structured)
    }

    /// Generates an image from a textual prompt using Gemini native image generation.
    /// Returns the raw PNG bytes and a generated filename.
    pub fn generate_image(
        &self,
        prompt: &str,
        width: u32,
        height: u32,
    ) -> Result<(Vec<u8>, String), String> {
        let payload = image_generation_payload(prompt, width, height);

        let response = ureq::post(GEMINI_INTERACTIONS_URL)
            .timeout(IMAGE_GENERATION_TIMEOUT)
            .set("x-goog-api-key", self.api_key.trim())
            .set("Content-Type", "application/json")
            .send_json(&payload)
            .map_err(|e| format!("Gemini API request failed: {}", e))?;

        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse Gemini API JSON response: {}", e))?;

        let b64_bytes = extract_generated_image_data(&json).ok_or_else(|| {
            format!(
                "Missing base64 image payload in Gemini API response: {}",
                json
            )
        })?;

        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64_bytes)
            .map_err(|e| format!("Failed to decode base64 image data: {}", e))?;

        if !looks_like_png(&bytes) {
            return Err("Gemini API returned image bytes that are not PNG data".to_string());
        }

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let filename = format!("image_{}.png", &hash[0..12]);

        Ok((bytes, filename))
    }

    /// Diagnoses a command failure and stderr output, recommending a fix.
    pub fn diagnose_stderr(
        &self,
        command: &str,
        exit_code: i32,
        stderr: &str,
    ) -> Result<StructuredShellOutput, String> {
        let prompt = format!(
            "Failed command: `{}`\nExit code: {}\nStderr:\n{}",
            command, exit_code, stderr
        );

        let system_prompt =
            "You are Pythia, an expert terminal diagnostic AI for Well Terminal.\n\
            Diagnose the command failure and propose the single best remediation command.\n\
            Output ONLY valid JSON with keys: 'command' (the fix command), 'explanation' (why it failed and how this fixes it), and 'confidence' (float 0.0 to 1.0).\n\
            Do NOT include markdown formatting.";

        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: &prompt }],
            }],
            system_instruction: Some(GeminiContent {
                parts: vec![GeminiPart {
                    text: system_prompt,
                }],
            }),
            generation_config: GeminiGenerationConfig {
                temperature: 0.1,
                response_mime_type: "application/json",
            },
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let response = ureq::post(&url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("Gemini API request failed: {}", e))?;

        let gemini_resp: GeminiResponse = response
            .into_json()
            .map_err(|e| format!("Failed to parse Gemini response JSON: {}", e))?;

        let raw_text = gemini_resp
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|p| p.into_iter().next())
            .and_then(|p| p.text)
            .ok_or_else(|| "Empty diagnostic response from Gemini API".to_string())?;

        let structured: StructuredShellOutput = serde_json::from_str(&raw_text)
            .map_err(|e| format!("Failed to parse diagnostic JSON: {} (raw: {})", e, raw_text))?;

        Ok(structured)
    }
}

fn image_generation_payload(prompt: &str, width: u32, height: u32) -> serde_json::Value {
    let aspect_ratio = if width == height {
        "1:1"
    } else if width > height {
        "16:9"
    } else {
        "9:16"
    };

    let mut response_format = serde_json::json!({
        "type": "image",
        "mime_type": "image/png",
        "aspect_ratio": aspect_ratio,
    });
    if let Some(size) = requested_image_size(width, height) {
        response_format["image_size"] = serde_json::Value::String(size.to_string());
    }

    serde_json::json!({
        "model": GEMINI_IMAGE_MODEL,
        "input": [
            {
                "type": "text",
                "text": prompt,
            }
        ],
        "response_format": response_format,
    })
}

fn extract_generated_image_data(json: &serde_json::Value) -> Option<&str> {
    for key in ["output_image", "outputImage"] {
        if let Some(data) = json
            .get(key)
            .and_then(|image| image.get("data"))
            .and_then(|data| data.as_str())
        {
            return Some(data);
        }
    }

    extract_inline_image_data(json)
}

fn extract_inline_image_data(json: &serde_json::Value) -> Option<&str> {
    let parts = json
        .get("candidates")?
        .as_array()?
        .iter()
        .filter_map(|candidate| candidate.get("content"))
        .filter_map(|content| content.get("parts"))
        .filter_map(|parts| parts.as_array())
        .flat_map(|parts| parts.iter());

    for part in parts {
        let inline_data = part.get("inlineData").or_else(|| part.get("inline_data"));
        if let Some(data) = inline_data
            .and_then(|inline| inline.get("data"))
            .and_then(|data| data.as_str())
        {
            return Some(data);
        }
    }

    None
}

fn requested_image_size(width: u32, height: u32) -> Option<&'static str> {
    let longest_side = width.max(height);
    if longest_side <= 512 {
        Some("0.5K")
    } else if longest_side <= 1024 {
        Some("1K")
    } else if longest_side <= 2048 {
        Some("2K")
    } else if longest_side <= 4096 {
        Some("4K")
    } else {
        None
    }
}

fn looks_like_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1A\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn extracts_inline_image_data_from_camel_case_response() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1A\npayload");
        let response = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "done" },
                        { "inlineData": { "mimeType": "image/png", "data": encoded } }
                    ]
                }
            }]
        });

        assert_eq!(extract_inline_image_data(&response), Some(encoded.as_str()));
    }

    #[test]
    fn extracts_inline_image_data_from_snake_case_response() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1A\npayload");
        let response = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "inline_data": { "mime_type": "image/png", "data": encoded } }
                    ]
                }
            }]
        });

        assert_eq!(extract_inline_image_data(&response), Some(encoded.as_str()));
    }

    #[test]
    fn extracts_interaction_output_image_data() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1A\npayload");
        let response = serde_json::json!({
            "output_image": { "data": encoded }
        });

        assert_eq!(
            extract_generated_image_data(&response),
            Some(encoded.as_str())
        );
    }

    #[test]
    fn builds_interactions_image_request_with_supported_image_format() {
        let request = image_generation_payload("orbital terminal", 1024, 1024);

        assert_eq!(request["model"], GEMINI_IMAGE_MODEL);
        assert_eq!(request["input"][0]["type"], "text");
        assert_eq!(request["input"][0]["text"], "orbital terminal");
        assert_eq!(request["response_format"]["type"], "image");
        assert_eq!(request["response_format"]["mime_type"], "image/png");
        assert_eq!(request["response_format"]["aspect_ratio"], "1:1");
        assert_eq!(request["response_format"]["image_size"], "1K");
    }

    #[test]
    fn maps_requested_image_size_to_supported_values() {
        assert_eq!(requested_image_size(512, 512), Some("0.5K"));
        assert_eq!(requested_image_size(1024, 768), Some("1K"));
        assert_eq!(requested_image_size(2048, 1024), Some("2K"));
        assert_eq!(requested_image_size(4096, 2048), Some("4K"));
        assert_eq!(requested_image_size(4097, 1024), None);
    }

    #[test]
    fn detects_png_signature() {
        assert!(looks_like_png(b"\x89PNG\r\n\x1A\npayload"));
        assert!(!looks_like_png(br#"{"error":"not an image"}"#));
    }
}
