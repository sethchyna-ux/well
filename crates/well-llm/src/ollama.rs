//! ollama.rs: Local Ollama LLM Client for Private Terminal Translation
//!
//! Connects to local Ollama instance (default: http://localhost:11434) for zero-latency,
//! 100% offline private LLM inference (e.g. qwen2.5-coder, gemma2, llama3).

use crate::gemini::StructuredShellOutput;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    timeout: Duration,
}

#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
    format: &'static str,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "qwen2.5-coder".to_string(),
            timeout: Duration::from_secs(5),
        }
    }
}

impl OllamaClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            timeout: Duration::from_secs(6),
        }
    }

    /// Checks if the local Ollama server is running and reachable.
    pub fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        ureq::get(&url)
            .timeout(Duration::from_millis(600))
            .call()
            .map(|r| r.status() == 200)
            .unwrap_or(false)
    }

    /// Translates a natural language query via local Ollama inference.
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
            Output ONLY valid JSON with fields: 'command' (string), 'explanation' (string), 'confidence' (float 0.0 to 1.0).\n\
            Do NOT include markdown backticks or commentary.",
            std::env::consts::OS,
            shell_type,
            cwd
        );

        let body = OllamaGenerateRequest {
            model: &self.model,
            prompt: query,
            system: &system_prompt,
            stream: false,
            format: "json",
        };

        let url = format!("{}/api/generate", self.base_url);
        let response = ureq::post(&url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("Ollama request failed: {}", e))?;

        let ollama_resp: OllamaGenerateResponse = response
            .into_json()
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let structured: StructuredShellOutput = serde_json::from_str(&ollama_resp.response)
            .map_err(|e| {
                format!(
                    "Failed to parse JSON from Ollama output: {} (raw: {})",
                    e, ollama_resp.response
                )
            })?;

        Ok(structured)
    }
}
