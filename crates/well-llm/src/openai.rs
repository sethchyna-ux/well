//! openai.rs: Direct OpenAI API Provider for Well Terminal
//!
//! Synthesizes POSIX shell commands and diagnostics via OpenAI Chat Completions
//! (`https://api.openai.com/v1/chat/completions`, defaulting to `gpt-4o-mini`).

use crate::gemini::StructuredShellOutput;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";

#[derive(Debug, Clone)]
pub struct OpenAIClient {
    api_key: String,
    model: String,
    base_url: String,
    timeout: Duration,
}

#[derive(Serialize)]
struct OpenAIChatRequest<'a> {
    model: &'a str,
    messages: [OpenAIChatMessage<'a>; 2],
    temperature: f32,
    response_format: OpenAIResponseFormat,
}

#[derive(Serialize)]
struct OpenAIChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct OpenAIResponseFormat {
    #[serde(rename = "type")]
    format_type: &'static str,
}

#[derive(Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChatChoice>,
}

#[derive(Deserialize)]
struct OpenAIChatChoice {
    message: OpenAIChatChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAIChatChoiceMessage {
    content: String,
}

impl OpenAIClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: DEFAULT_OPENAI_MODEL.to_string(),
            base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
            timeout: Duration::from_secs(8),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// Translates a natural language query into a structured shell command via OpenAI.
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
             Output ONLY valid JSON with fields: 'command' (string), 'explanation' (string), 'confidence' (float 0.0 to 1.0).",
            std::env::consts::OS,
            shell_type,
            cwd
        );

        let body = OpenAIChatRequest {
            model: &self.model,
            messages: [
                OpenAIChatMessage {
                    role: "system",
                    content: &system_prompt,
                },
                OpenAIChatMessage {
                    role: "user",
                    content: query,
                },
            ],
            temperature: 0.1,
            response_format: OpenAIResponseFormat {
                format_type: "json_object",
            },
        };

        let url = format!("{}/chat/completions", self.base_url);
        let auth_header = format!("Bearer {}", self.api_key.trim());

        let response = ureq::post(&url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .set("Authorization", &auth_header)
            .send_json(&body)
            .map_err(|e| format!("OpenAI request failed: {e}"))?;

        let chat_resp: OpenAIChatResponse = response
            .into_json()
            .map_err(|e| format!("Failed to parse OpenAI response envelope: {e}"))?;

        let raw_content = chat_resp
            .choices
            .first()
            .map(|c| c.message.content.trim())
            .ok_or_else(|| "OpenAI returned empty choices array".to_string())?;

        serde_json::from_str::<StructuredShellOutput>(raw_content)
            .map_err(|e| format!("Failed to parse OpenAI JSON content: {e} (raw: {raw_content})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn test_openai_client_formats_request_and_parses_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut data = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let n = stream.read(&mut chunk).expect("read request");
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&chunk[..n]);
                let req_so_far = String::from_utf8_lossy(&data);
                if req_so_far.contains("\"model\":\"gpt-4o-mini\"") {
                    break;
                }
            }
            let req_str = String::from_utf8_lossy(&data);
            assert!(req_str.contains("POST /v1/chat/completions"));
            assert!(req_str.contains("Authorization: Bearer sk-test-openai-key"));
            assert!(req_str.contains("\"model\":\"gpt-4o-mini\""));

            let body = r#"{"choices":[{"message":{"content":"{\"command\":\"find . -name '*.rs'\",\"explanation\":\"Search Rust files recursively\",\"confidence\":0.96}"}}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).expect("write response");
            stream.flush().expect("flush response");
        });

        let client = OpenAIClient::new("sk-test-openai-key")
            .with_base_url(format!("http://{}/v1", addr));
        let out = client
            .translate_query("find all rust files", "zsh", "/tmp")
            .expect("OpenAI translate succeeds");

        assert_eq!(out.command, "find . -name '*.rs'");
        assert!(out.confidence > 0.9);
        server.join().expect("server thread completes");
    }
}

