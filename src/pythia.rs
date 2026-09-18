//! Pythia AI Assistant / Client for Well Terminal
//! 
//! Integrates Google Gemini API for natural language command generation,
//! error diagnosis, inline code suggestion, and multimodal tasks.

use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::StreamExt;

#[derive(Clone, Debug)]
pub struct PythiaClient {
    pub api_key: String,
    pub active_model: String,
    pub is_streaming: Arc<Mutex<bool>>,
}

impl Default for PythiaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PythiaClient {
    pub fn new() -> Self {
        let api_key = std::env::var("GEMINI_API_KEY")
            .or_else(|_| std::env::var("GOOGLE_API_KEY"))
            .unwrap_or_default();

        Self {
            api_key,
            active_model: "gemini-2.5-flash".to_string(),
            is_streaming: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_api_key(&mut self, key: String) {
        self.api_key = key;
    }

    pub async fn ask_stream<F>(&self, prompt: String, on_chunk: F) -> Result<String, String>
    where
        F: Fn(String) + Send + 'static,
    {
        if self.api_key.is_empty() {
            return Err("GEMINI_API_KEY is not set. Please set the GEMINI_API_KEY environment variable or enter it in Pythia settings.".to_string());
        }

        let mut lock = self.is_streaming.lock().await;
        *lock = true;
        drop(lock);

        // Streaming interface using Gemini REST streamGenerateContent:
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            if self.active_model.is_empty() { "gemini-2.5-flash" } else { &self.active_model },
            self.api_key
        );

        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "contents": [{
                "parts": [{
                    "text": prompt
                }]
            }]
        });

        let response = client.post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let err_text = response.text().await.unwrap_or_default();
            let mut lock = self.is_streaming.lock().await;
            *lock = false;
            return Err(format!("Gemini API Error: {}", err_text));
        }

        let mut stream = response.bytes_stream();
        let mut full_text = String::new();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(bytes) => {
                    let chunk_str = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&chunk_str);

                    for line in buffer.lines() {
                        if line.starts_with("data: ") {
                            let json_data = &line[6..];
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_data) {
                                if let Some(candidates) = val.get("candidates").and_then(|c| c.as_array()) {
                                    for candidate in candidates {
                                        if let Some(parts) = candidate.get("content").and_then(|c| c.get("parts")).and_then(|p| p.as_array()) {
                                            for part in parts {
                                                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                                    full_text.push_str(text);
                                                    on_chunk(text.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error streaming from Gemini API: {}", e);
                    break;
                }
            }
        }

        let mut lock = self.is_streaming.lock().await;
        *lock = false;

        Ok(full_text)
    }
}
