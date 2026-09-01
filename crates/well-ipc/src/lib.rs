//! well-ipc: Hermes Zero-Copy IPC & Caduceus In-Process JSON-RPC 2.0 Server
//!
//! Subsystems:
//! - Hermes: Atomic lock-free Sequence Lock (Seqlock) coordinating state sync between
//!   Theia (GUI config) and concurrent worker threads (Metis / Orpheus).
//! - Caduceus: In-process asynchronous JSON-RPC 2.0 protocol for agent orchestration.

use std::cell::UnsafeCell;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// =========================================================================
// 1. HERMES SEQLOCK & ZERO-COPY STATE SYNC
// =========================================================================

/// Configuration Vector payload for the Visual Config Tab (Theia).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Self { counter: AtomicU64::new(0) }
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
        seq == current && seq % 2 == 0
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
            error: Some(JsonRpcError { code, message, data: None }),
            id,
        }
    }
}

/// Actions dispatched to the Main UI Thread (Atlas) where mutable views reside.
#[derive(Debug)]
pub enum HermesAppCommand {
    /// Navigate to a specific tab index
    SwitchTab { tab_index: usize, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Split the current active surface horizontally or vertically
    SplitPane { horizontal: bool, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Send text/keystrokes directly to Metis shell
    SendShellInput { input_text: String, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Insert text at cursor inside Mneme editor
    InsertEditorText { text: String, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Signal agent lifecycle events
    PublishAgentLifecycle { event_name: String, payload: Value, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Save a configuration profile
    SaveProfile { name: String, payload: TheiaConfigPayload, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Load a configuration profile
    LoadProfile { name: String, respond_to: oneshot::Sender<Result<Value, String>> },
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
                let err_resp = JsonRpcResponse::error(Value::Null, -32700, "Parse error: Invalid JSON".to_string());
                return Ok(serde_json::to_vec(&err_resp)?);
            }
        };

        let request_id = request.id.unwrap_or(Value::Null);

        let response = match request.method.as_str() {
            "system.ping" => {
                JsonRpcResponse::success(request_id, serde_json::json!({ "status": "pong", "protocol": "caduceus/1.0" }))
            }
            "surface.switch_tab" => {
                let tab_index = request.params
                    .as_ref()
                    .and_then(|p| p.get("index"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self.app_tx.send(HermesAppCommand::SwitchTab { tab_index, respond_to: tx }).await {
                    JsonRpcResponse::error(request_id, -32603, format!("Internal channel send error: {}", e))
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(request_id, -32603, "Response dropped by host loop".to_string()),
                    }
                }
            }
            "surface.split" => {
                let horizontal = request.params
                    .as_ref()
                    .and_then(|p| p.get("horizontal"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self.app_tx.send(HermesAppCommand::SplitPane { horizontal, respond_to: tx }).await {
                    JsonRpcResponse::error(request_id, -32603, format!("Internal channel send error: {}", e))
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(request_id, -32603, "Response dropped by host loop".to_string()),
                    }
                }
            }
            "shell.input" => {
                let input = request.params
                    .as_ref()
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self.app_tx.send(HermesAppCommand::SendShellInput { input_text: input, respond_to: tx }).await {
                    JsonRpcResponse::error(request_id, -32603, format!("Internal channel send error: {}", e))
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(request_id, -32603, "Response dropped by host loop".to_string()),
                    }
                }
            }
            "editor.insert" => {
                let text = request.params
                    .as_ref()
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let (tx, rx) = oneshot::channel();
                if let Err(e) = self.app_tx.send(HermesAppCommand::InsertEditorText { text, respond_to: tx }).await {
                    JsonRpcResponse::error(request_id, -32603, format!("Internal channel send error: {}", e))
                } else {
                    match rx.await {
                        Ok(Ok(val)) => JsonRpcResponse::success(request_id, val),
                        Ok(Err(err)) => JsonRpcResponse::error(request_id, -32000, err),
                        Err(_) => JsonRpcResponse::error(request_id, -32603, "Response dropped by host loop".to_string()),
                    }
                }
            }
            _ => {
                JsonRpcResponse::error(request_id, -32601, format!("Method not found: {}", request.method))
            }
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

        let mut updated = read1;
        updated.theme_id = 42;
        channel.sync_state(updated);

        let read2 = channel.read_state();
        assert_eq!(read2.theme_id, 42);
    }
}
