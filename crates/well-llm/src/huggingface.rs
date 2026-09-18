//! huggingface.rs: Hugging Face Inference Client for Uncensored / Abliterated Models
//!
//! Connects directly to Hugging Face Inference API or Hugging Face Router,
//! with first-class presets for Gemma Abliterated (`failspy/gemma-2-9b-it-abliterated`,
//! `mlabonne/gemma-2-2b-it-abliterated`) for unfiltered terminal command translation.

use crate::gemini::StructuredShellOutput;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
        let mut req = ureq::post(url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json");

        if let Some(ref token) = self.api_token {
            if !token.trim().is_empty() {
                req = req.set("Authorization", &format!("Bearer {}", token.trim()));
            }
        }

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

        let structured: StructuredShellOutput = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON from Gemma Abliterated: {} (raw: {})", e, raw_text))?;

        Ok(structured)
    }

    /// Generates an image using Hugging Face Inference API.
    pub fn generate_image(
        &self,
        prompt: &str,
        _width: u32,
        _height: u32,
    ) -> Result<(Vec<u8>, String), String> {
        let url = format!(
            "https://api-inference.huggingface.co/models/{}",
            self.model
        );
        let mut req = ureq::post(&url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json");

        if let Some(ref token) = self.api_token {
            if !token.trim().is_empty() {
                req = req.set("Authorization", &format!("Bearer {}", token.trim()));
            }
        }

        let body = serde_json::json!({ "inputs": prompt });
        let resp = req
            .send_json(&body)
            .map_err(|e| format!("Hugging Face image API request failed: {}", e))?;

        let mut bytes = Vec::new();
        use std::io::Read;
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read HF image bytes: {}", e))?;

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.model.as_bytes());
        hasher.update(prompt.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let filename = format!("image_hf_{}.png", &hash[0..12]);
        Ok((bytes, filename))
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
