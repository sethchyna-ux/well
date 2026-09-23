//! well-ipc: Hermes Zero-Copy IPC & Caduceus In-Process JSON-RPC 2.0 Server
//!
//! Subsystems:
//! - Hermes: Atomic lock-free Sequence Lock (Seqlock) coordinating state sync between
//!   Theia (GUI config) and concurrent worker threads (Metis / Orpheus).
//! - Caduceus: In-process asynchronous JSON-RPC 2.0 protocol for agent orchestration.

pub mod mcp;
pub mod ring_buffer;
pub mod rpc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::UnsafeCell;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

// =========================================================================
// 1. HERMES SEQLOCK & ZERO-COPY STATE SYNC
// =========================================================================

/// Configuration Vector payload for the Visual Config Tab (Theia).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TheiaConfigPayload {
    pub theme_id: u32,
    pub background_opacity: f32,
    pub glass_blur_radius: f32,
    pub scan_timeout_ms: u32,
    pub command_timeout_ms: u32,
    pub enable_transient_prompt: bool,
    pub enable_kitty_keyboard: bool,
    pub scrollback_limit: u32,
    pub font_size: f32,
    pub cursor_style: u32, // 0 = Block, 1 = Beam, 2 = Underline
    pub cursor_blink: bool,
    pub screen_curvature: f32,
    pub scanline_frequency: f32,
    pub glow_radius: f32,
    pub line_height: f32,
    // New fields
    pub profile_name: Option<String>,
    pub shader_preset: u32,
    #[serde(default = "default_prompt_preset")]
    pub prompt_preset: String,
    #[serde(default = "default_font_name")]
    pub font_name: String,
    #[serde(default = "default_shell_path")]
    pub shell_path: String,
    #[serde(default = "default_pythia_provider")]
    pub pythia_provider: String,
    #[serde(default = "default_hf_model")]
    pub hf_model: String,
    #[serde(default)]
    pub hf_token: String,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
}

fn default_pythia_provider() -> String {
    "Offline Semantic Rules".to_string()
}

fn default_hf_model() -> String {
    "failspy/gemma-2-9b-it-abliterated".to_string()
}

fn default_gemini_model() -> String {
    "gemini-2.0-flash".to_string()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_ollama_model() -> String {
    "qwen2.5-coder".to_string()
}

fn default_prompt_preset() -> String {
    "Jetpack".to_string()
}

fn default_font_name() -> String {
    "System Monospace".to_string()
}

fn default_shell_path() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
}

impl Default for TheiaConfigPayload {
    fn default() -> Self {
        Self {
            theme_id: 0, // Cyber-Neon
            background_opacity: 0.95,
            glass_blur_radius: 20.0,
            scan_timeout_ms: 30,
            command_timeout_ms: 500,
            enable_transient_prompt: true,
            enable_kitty_keyboard: true,
            scrollback_limit: 100_000,
            font_size: 14.0,
            cursor_style: 0, // Block
            cursor_blink: true,
            screen_curvature: 0.05,
            scanline_frequency: 0.5,
            glow_radius: 1.2,
            line_height: 1.2,
            profile_name: None,
            shader_preset: 0,
            prompt_preset: default_prompt_preset(),
            font_name: default_font_name(),
            shell_path: default_shell_path(),
            pythia_provider: default_pythia_provider(),
            hf_model: default_hf_model(),
            hf_token: String::new(),
            gemini_api_key: String::new(),
            gemini_model: default_gemini_model(),
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
        }
    }
}

impl TheiaConfigPayload {
    /// Apply a preset of shader parameters.
    pub fn apply_preset(&mut self, preset_id: u32) {
        match preset_id {
            1 => {
                self.screen_curvature = 0.3;
                self.scanline_frequency = 1.0;
                self.glow_radius = 2.0;
            }
            2 => {
                self.screen_curvature = 0.1;
                self.scanline_frequency = 0.3;
                self.glow_radius = 0.5;
            }
            _ => {}
        }
        self.shader_preset = preset_id;
    }

    /// Rename the current profile.
    pub fn rename_profile(&mut self, new_name: String) {
        self.profile_name = Some(new_name);
    }
}

/// HermesSeqlock: Lock-free atomic synchronization guard for zero-copy state reads.
pub struct HermesSeqlock {
    counter: AtomicU64,
}

impl Default for HermesSeqlock {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesSeqlock {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Executed by the writer before modifying state.
    pub fn write_begin(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Executed by the writer after modifying state.
    pub fn write_end(&self, seq: u64) {
        self.counter.store(seq + 2, Ordering::SeqCst);
    }

    /// Executed by the reader.
    pub fn read_begin(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Returns true if the state remained unmodified during the read transaction.
    pub fn read_validate(&self, seq: u64) -> bool {
        let current = self.counter.load(Ordering::SeqCst);
        seq == current && seq.is_multiple_of(2)
    }

    /// Returns current atomic sequence counter (even = idle, odd = writing).
    pub fn sequence(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }
}

/// HermesChannel: Dual-slot shared-memory synchronization context.
pub struct HermesChannel<T = TheiaConfigPayload> {
    seqlock: HermesSeqlock,
    payload: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for HermesChannel<T> {}
unsafe impl<T: Sync> Sync for HermesChannel<T> {}

impl<T: Clone> HermesChannel<T> {
    pub fn new(initial: T) -> Self {
        Self {
            seqlock: HermesSeqlock::new(),
            payload: UnsafeCell::new(initial),
        }
    }

    /// Safely synchronizes/writes a new configuration state vector.
    pub fn sync_state(&self, new_state: T) {
        let seq = self.seqlock.write_begin();
        unsafe {
            *self.payload.get() = new_state;
        }
        self.seqlock.write_end(seq);
    }

    /// Safely reads current configuration state vector with lock-free seqlock validation.
    pub fn read_state(&self) -> T {
        loop {
            let seq = self.seqlock.read_begin();
            // Clone the value to avoid requiring Copy
            let val = unsafe { (*self.payload.get()).clone() };
            if self.seqlock.read_validate(seq) {
                return val;
            }
            std::hint::spin_loop();
        }
    }

    /// Returns current atomic sequence counter for IPC telemetry.
    pub fn sequence(&self) -> u64 {
        self.seqlock.sequence()
    }
}

// =========================================================================
// 2. CADUCEUS: JSON-RPC 2.0 PROTOCOL & AGENT ORCHESTRATION
// =========================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub fn caduceus_capabilities() -> Value {
    serde_json::json!({
        "protocol": "caduceus/1.0",
        "scope": "local desktop runtime",
        "transport": {
            "kind": "unix_socket",
            "peer_uid_required": true,
            "socket_permissions": "0600"
        },
        "active_methods": [
            {
                "method": "system.ping",
                "description": "Health check for the local Caduceus RPC service.",
                "dispatches_to_host": false
            },
            {
                "method": "system.capabilities",
                "description": "Machine-readable capability and boundary manifest.",
                "dispatches_to_host": false
            },
            {
                "method": "shell.input",
                "description": "Send text to the currently active desktop PTY.",
                "dispatches_to_host": true
            },
            {
                "method": "surface.switch_tab",
                "description": "Switch the active desktop tab by zero-based index.",
                "dispatches_to_host": true
            },
            {
                "method": "surface.split",
                "description": "Split the active surface and spawn a new local PTY.",
                "dispatches_to_host": true
            },
            {
                "method": "agent.lifecycle",
                "description": "Record an agent lifecycle event in the desktop runtime.",
                "dispatches_to_host": true
            }
        ],
        "unsupported_methods": [
            {
                "method": "editor.insert",
                "reason": "Mneme inline composer is GUI-local; remote cursor insertion is not exposed."
            },
            {
                "method": "editor.diff",
                "reason": "File diff application is not exposed by the current desktop runtime."
            },
            {
                "method": "profile.save",
                "reason": "Profile persistence is GUI/config-local and not exposed over RPC."
            },
            {
                "method": "profile.load",
                "reason": "Profile loading is GUI/config-local and not exposed over RPC."
            },
            {
                "method": "agent.workspace.spawn",
                "reason": "Sandboxed agent workspaces are not implemented in the desktop runtime."
            }
        ],
        "gpu": {
            "renderer": "local wgpu/Metal Orpheus renderer",
            "remote_gpu_control": false,
            "maturity": "local rendering path only; surface lost/outdated/timeout/out-of-memory handling is tested in the desktop host"
        },
        "notes": [
            "Capabilities describe the currently exposed automation surface, not long-term roadmap intent.",
            "Unsupported methods should fail explicitly instead of pretending to succeed."
        ]
    })
}

fn unsupported_rpc_reason(method: &str) -> Option<&'static str> {
    match method {
        "editor.insert" => {
            Some("Mneme inline composer is GUI-local; remote cursor insertion is not exposed.")
        }
        "editor.diff" => {
            Some("File diff application is not exposed by the current desktop runtime.")
        }
        "profile.save" => Some("Profile persistence is GUI/config-local and not exposed over RPC."),
        "profile.load" => Some("Profile loading is GUI/config-local and not exposed over RPC."),
        "agent.workspace.spawn" => {
            Some("Sandboxed agent workspaces are not implemented in the desktop runtime.")
        }
        _ => None,
    }
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

/// Actions dispatched to the Main UI Thread (Atlas) where mutable views reside.
#[derive(Debug)]
pub enum HermesAppCommand {
    /// Navigate to a specific tab index
    SwitchTab {
        tab_index: usize,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Split the current active surface horizontally or vertically
    SplitPane {
        horizontal: bool,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Send text/keystrokes directly to Metis shell
    SendShellInput {
        input_text: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Insert text at cursor inside Mneme editor
    InsertEditorText {
        text: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Signal agent lifecycle events
    PublishAgentLifecycle {
        event_name: String,
        payload: Value,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Save a configuration profile
    SaveProfile {
        name: String,
        payload: Box<TheiaConfigPayload>,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Load a configuration profile
    LoadProfile {
        name: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Apply an incoming editor diff proposal from an MCP agent
    ProposeEditorDiff {
        file_uri: String,
        diff: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    /// Spawn an isolated agent workspace using distrobox/chroot sandboxing
    SpawnAgentWorkspace {
        command: String,
        args: Vec<String>,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
}

/// Core service translating incoming JSON-RPC payloads into strongly typed responses.
pub struct HermesService {
    app_tx: mpsc::Sender<HermesAppCommand>,
}

impl HermesService {
    pub fn new(app_tx: mpsc::Sender<HermesAppCommand>) -> Self {
        Self { app_tx }
    }

    pub async fn handle_request(
        self: Arc<Self>,
        req_body_bytes: &[u8],
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let request: JsonRpcRequest = match serde_json::from_slice(req_body_bytes) {
            Ok(req) => req,
            Err(_) => {
                let err_resp = JsonRpcResponse::error(
                    Value::Null,
                    -32700,
                    "Parse error: Invalid JSON".to_string(),
                );
                return Ok(serde_json::to_vec(&err_resp)?);
            }
        };

        let request_id = request.id.unwrap_or(Value::Null);

        let response = match request.method.as_str() {
            "system.ping" => JsonRpcResponse::success(
                request_id,
                serde_json::json!({ "status": "pong", "protocol": "caduceus/1.0" }),
            ),
            "system.capabilities" => {
                JsonRpcResponse::success(request_id, caduceus_capabilities())
            }
            "surface.switch_tab" => {
                let tab_index = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("tab_index"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self
                    .app_tx
                    .send(HermesAppCommand::SwitchTab {
                        tab_index,
                        respond_to: tx,
                    })
                    .await
                {
                    JsonRpcResponse::error(
                        request_id,
                        -32603,
                        format!("Internal channel send error: {}", e),
                    )
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(
                            request_id,
                            -32603,
                            "Response dropped by host loop".to_string(),
                        ),
                    }
                }
            }
            "surface.split" => {
                let horizontal = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("horizontal"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self
                    .app_tx
                    .send(HermesAppCommand::SplitPane {
                        horizontal,
                        respond_to: tx,
                    })
                    .await
                {
                    JsonRpcResponse::error(
                        request_id,
                        -32603,
                        format!("Internal channel send error: {}", e),
                    )
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(
                            request_id,
                            -32603,
                            "Response dropped by host loop".to_string(),
                        ),
                    }
                }
            }
            "shell.input" => {
                let input = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self
                    .app_tx
                    .send(HermesAppCommand::SendShellInput {
                        input_text: input,
                        respond_to: tx,
                    })
                    .await
                {
                    JsonRpcResponse::error(
                        request_id,
                        -32603,
                        format!("Internal channel send error: {}", e),
                    )
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(
                            request_id,
                            -32603,
                            "Response dropped by host loop".to_string(),
                        ),
                    }
                }
            }
            "agent.lifecycle" => {
                let event_name = request
                    .params
                    .as_ref()
                    .and_then(|p| {
                        p.get("event_name")
                            .or_else(|| p.get("event"))
                            .or_else(|| p.get("type"))
                    })
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent.event")
                    .to_string();
                let payload = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("payload"))
                    .cloned()
                    .unwrap_or(Value::Null);

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self
                    .app_tx
                    .send(HermesAppCommand::PublishAgentLifecycle {
                        event_name,
                        payload,
                        respond_to: tx,
                    })
                    .await
                {
                    JsonRpcResponse::error(
                        request_id,
                        -32603,
                        format!("Internal channel send error: {}", e),
                    )
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(
                            request_id,
                            -32603,
                            "Response dropped by host loop".to_string(),
                        ),
                    }
                }
            }
            method if unsupported_rpc_reason(method).is_some() => JsonRpcResponse::error(
                request_id,
                -32004,
                format!(
                    "{method} is unsupported: {} Call system.capabilities for the active RPC surface.",
                    unsupported_rpc_reason(method).expect("reason checked above")
                ),
            ),
            _ => JsonRpcResponse::error(
                request_id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        };

        Ok(serde_json::to_vec(&response)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermes_seqlock_consistency() {
        let default_payload = TheiaConfigPayload::default();
        let channel = HermesChannel::new(default_payload.clone());
        let read1 = channel.read_state();
        assert_eq!(read1.theme_id, default_payload.theme_id);
        assert_eq!(read1.pythia_provider, "Offline Semantic Rules");

        let mut updated = read1;
        updated.theme_id = 42;
        channel.sync_state(updated);

        let read2 = channel.read_state();
        assert_eq!(read2.theme_id, 42);
    }

    #[tokio::test]
    async fn unsupported_manifest_methods_return_explicit_errors_without_host_dispatch() {
        for method in [
            "editor.insert",
            "editor.diff",
            "profile.save",
            "profile.load",
            "agent.workspace.spawn",
        ] {
            let (app_tx, mut app_rx) = mpsc::channel(1);
            let service = Arc::new(HermesService::new(app_tx));
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": {
                    "text": "hello",
                    "file_uri": "file:///tmp/example.rs",
                    "diff": "@@",
                    "name": "default",
                    "command": "true"
                },
                "id": method
            });

            let response = service
                .handle_request(request.to_string().as_bytes())
                .await
                .expect("request should be handled");
            let response: JsonRpcResponse =
                serde_json::from_slice(&response).expect("response should parse");

            assert_eq!(response.id, serde_json::json!(method));
            let error = response.error.expect("unsupported method should error");
            assert_eq!(error.code, -32004);
            assert!(
                error.message.contains(&format!("{method} is unsupported")),
                "unexpected error for {method}: {}",
                error.message
            );
            assert!(error.message.contains("system.capabilities"));
            assert!(app_rx.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn capabilities_rpc_reports_active_and_unsupported_surfaces() {
        let (app_tx, mut app_rx) = mpsc::channel(1);
        let service = Arc::new(HermesService::new(app_tx));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "system.capabilities",
            "id": "caps"
        });

        let response = service
            .handle_request(request.to_string().as_bytes())
            .await
            .expect("request should be handled");
        let response: JsonRpcResponse =
            serde_json::from_slice(&response).expect("response should parse");

        assert_eq!(response.id, serde_json::json!("caps"));
        let result = response.result.expect("capabilities should be returned");
        assert_eq!(result["protocol"], serde_json::json!("caduceus/1.0"));
        assert_eq!(
            result["transport"]["peer_uid_required"],
            serde_json::json!(true)
        );
        assert_eq!(
            result["gpu"]["remote_gpu_control"],
            serde_json::json!(false)
        );

        let active_methods = result["active_methods"]
            .as_array()
            .expect("active methods should be an array");
        assert!(active_methods
            .iter()
            .any(|method| method["method"] == serde_json::json!("shell.input")));
        assert!(active_methods
            .iter()
            .any(|method| method["method"] == serde_json::json!("surface.split")));

        let unsupported_methods = result["unsupported_methods"]
            .as_array()
            .expect("unsupported methods should be an array");
        assert!(unsupported_methods
            .iter()
            .any(|method| method["method"] == serde_json::json!("editor.insert")));
        assert!(unsupported_methods
            .iter()
            .any(|method| method["method"] == serde_json::json!("agent.workspace.spawn")));
        assert!(app_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn surface_split_rpc_dispatches_to_host_loop_and_returns_response() {
        let (app_tx, mut app_rx) = mpsc::channel(1);
        let service = Arc::new(HermesService::new(app_tx));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "surface.split",
            "params": { "horizontal": true },
            "id": 8
        });

        let response_task = tokio::spawn(async move {
            service
                .handle_request(request.to_string().as_bytes())
                .await
                .expect("request should be handled")
        });

        let HermesAppCommand::SplitPane {
            horizontal,
            respond_to,
        } = app_rx.recv().await.expect("split pane command expected")
        else {
            panic!("unexpected Hermes command");
        };
        assert!(horizontal);
        let _ = respond_to.send(Ok(serde_json::json!({ "new_pane_id": 1 })));

        let response_bytes = response_task.await.expect("task join");
        let response: JsonRpcResponse =
            serde_json::from_slice(&response_bytes).expect("response should parse");
        assert_eq!(response.id, serde_json::json!(8));
        assert_eq!(
            response.result.unwrap(),
            serde_json::json!({ "new_pane_id": 1 })
        );
    }

    #[tokio::test]
    async fn surface_switch_tab_rpc_dispatches_to_host_loop_and_returns_response() {
        let (app_tx, mut app_rx) = mpsc::channel(1);
        let service = Arc::new(HermesService::new(app_tx));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "surface.switch_tab",
            "params": { "tab_index": 2 },
            "id": "tab"
        });

        let response_task = tokio::spawn(async move {
            service
                .handle_request(request.to_string().as_bytes())
                .await
                .expect("request should be handled")
        });

        let HermesAppCommand::SwitchTab {
            tab_index,
            respond_to,
        } = app_rx.recv().await.expect("switch tab command expected")
        else {
            panic!("unexpected Hermes command");
        };
        assert_eq!(tab_index, 2);
        let _ = respond_to.send(Ok(serde_json::json!({
            "accepted": true,
            "active_tab": 2
        })));

        let response_bytes = response_task.await.expect("task join");
        let response: JsonRpcResponse =
            serde_json::from_slice(&response_bytes).expect("response should parse");
        assert_eq!(response.id, serde_json::json!("tab"));
        assert_eq!(
            response.result.unwrap(),
            serde_json::json!({ "accepted": true, "active_tab": 2 })
        );
    }

    #[tokio::test]
    async fn shell_input_rpc_dispatches_to_host_loop_and_returns_response() {
        let (app_tx, mut app_rx) = mpsc::channel(1);
        let service = Arc::new(HermesService::new(app_tx));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "shell.input",
            "params": { "text": "printf hi\n" },
            "id": "shell"
        });

        let response_task = tokio::spawn(async move {
            service
                .handle_request(request.to_string().as_bytes())
                .await
                .expect("request should be handled")
        });

        let HermesAppCommand::SendShellInput {
            input_text,
            respond_to,
        } = app_rx.recv().await.expect("shell input command expected")
        else {
            panic!("unexpected Hermes command");
        };
        assert_eq!(input_text, "printf hi\n");
        respond_to
            .send(Ok(serde_json::json!({ "accepted": true })))
            .expect("response channel should be open");

        let response = response_task.await.expect("response task should finish");
        let response: JsonRpcResponse =
            serde_json::from_slice(&response).expect("response should parse");
        assert_eq!(response.id, serde_json::json!("shell"));
        assert!(response.error.is_none());
        assert_eq!(
            response.result.expect("success result expected")["accepted"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn agent_lifecycle_rpc_dispatches_to_host_loop_and_returns_response() {
        let (app_tx, mut app_rx) = mpsc::channel(1);
        let service = Arc::new(HermesService::new(app_tx));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.lifecycle",
            "params": {
                "event_name": "agent.started",
                "payload": { "agent": "qa" }
            },
            "id": "life"
        });

        let response_task = tokio::spawn(async move {
            service
                .handle_request(request.to_string().as_bytes())
                .await
                .expect("request should be handled")
        });

        let HermesAppCommand::PublishAgentLifecycle {
            event_name,
            payload,
            respond_to,
        } = app_rx
            .recv()
            .await
            .expect("agent lifecycle command expected")
        else {
            panic!("unexpected Hermes command");
        };
        assert_eq!(event_name, "agent.started");
        assert_eq!(payload, serde_json::json!({ "agent": "qa" }));
        let _ = respond_to.send(Ok(serde_json::json!({ "accepted": true })));

        let response_bytes = response_task.await.expect("task join");
        let response: JsonRpcResponse =
            serde_json::from_slice(&response_bytes).expect("response should parse");
        assert_eq!(response.id, serde_json::json!("life"));
        assert_eq!(
            response.result.unwrap(),
            serde_json::json!({ "accepted": true })
        );
    }
}

// =========================================================================
// 3. NATIVE PATH: KITTY KEYBOARD PROTOCOL & TYPED BLOCKS
// =========================================================================

/// Kitty Keyboard Protocol Input Event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KittyKeyboardInput {
    pub key_code: u32,
    pub modifiers: u32, // Shift, Alt, Ctrl, Super bitmask
    pub event_type: u8, // Press(1), Release(2), Repeat(3)
}

/// Structured typed block for output transport over Hermes IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypedBlock {
    StdoutChunk(Vec<u8>),
    StderrChunk(Vec<u8>),
    ExitStatus(i32),
}

/// A zero-copy lock-free ring buffer using HermesSeqlock for synchronization.
/// This acts as the high-throughput transport mechanism for Native Path block output.
pub struct HermesRingBuffer<const CAPACITY: usize> {
    seqlock: HermesSeqlock,
    head: std::sync::atomic::AtomicUsize,
    tail: std::sync::atomic::AtomicUsize,
    buffer: UnsafeCell<[Option<TypedBlock>; CAPACITY]>,
}

unsafe impl<const N: usize> Send for HermesRingBuffer<N> {}
unsafe impl<const N: usize> Sync for HermesRingBuffer<N> {}

impl<const CAPACITY: usize> HermesRingBuffer<CAPACITY> {
    pub fn new() -> Self {
        // Initialize an array of `None`
        let buffer = std::array::from_fn(|_| None);
        Self {
            seqlock: HermesSeqlock::new(),
            head: std::sync::atomic::AtomicUsize::new(0),
            tail: std::sync::atomic::AtomicUsize::new(0),
            buffer: UnsafeCell::new(buffer),
        }
    }

    /// Safely push a TypedBlock onto the ring buffer
    pub fn push(&self, block: TypedBlock) -> Result<(), &'static str> {
        let seq = self.seqlock.write_begin();

        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);

        let next_tail = (tail + 1) % CAPACITY;
        if next_tail == head {
            self.seqlock.write_end(seq);
            return Err("Ring buffer full");
        }

        unsafe {
            (*self.buffer.get())[tail] = Some(block);
        }

        self.tail.store(next_tail, Ordering::Release);
        self.seqlock.write_end(seq);
        Ok(())
    }

    /// Lock-free pop operation from the ring buffer
    pub fn pop(&self) -> Option<TypedBlock> {
        loop {
            let seq = self.seqlock.read_begin();

            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Acquire);

            if head == tail {
                if self.seqlock.read_validate(seq) {
                    return None; // Buffer empty
                }
                continue;
            }

            if self
                .head
                .compare_exchange(
                    head,
                    (head + 1) % CAPACITY,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                let val = unsafe { (*self.buffer.get())[head].take() };
                if self.seqlock.read_validate(seq) {
                    return val;
                }
                return val;
            }
        }
    }
}

impl<const N: usize> Default for HermesRingBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}
