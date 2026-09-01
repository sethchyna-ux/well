//! Hermes RPC Server: In-process asynchronous JSON-RPC 2.0 socket server for "Well".
//! Powered by Hyper, Tokio, and Serde. Coordinates local client and agent actions (like Claude Code)
//! by bridging them directly into Well's shared memory state engines (Metis, Mneme, and Orpheus).
//!
//! Mythological context:
//! This module acts as the "Caduceus" of Hermes (the swift messenger). It exposes a secure,
//! local IPC interface that allows external coding agents to query workspace parameters,
//! spawn split-panes, insert text into Mneme buffers, and stream terminal blocks with sub-millisecond latency.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// =========================================================================
// 1. Structural Schema for Well's JSON-RPC 2.0 Protocol
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

// =========================================================================
// 2. Hermes State-Dispatch Message Protocol (Inter-Thread Communication)
// =========================================================================

/// Actions dispatched to the Main UI Thread (Atlas) where mutable views (egui/Theia)
/// and text editors (Mneme) reside. The socket thread cannot mutate them directly.
#[derive(Debug)]
pub enum HermesAppCommand {
    /// Navigate to a specific tab index (e.g. index 0 = active shell, index 1 = Theia's Prism)
    SwitchTab { tab_index: usize, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Split the current active surface horizontally or vertically
    SplitPane { horizontal: bool, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Send text/keystrokes directly to the input execution queue of Metis (Fish Shell Core)
    SendShellInput { input_text: String, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Insert text at the current cursor coordinates inside Mneme (Rope Editor)
    InsertEditorText { text: String, respond_to: oneshot::Sender<Result<Value, String>> },
    /// Signal agent state changes (e.g. ai.session_start, ai.tool_use) to update the visual sidebar
    PublishAgentLifecycle { event_name: String, payload: Value, respond_to: oneshot::Sender<Result<Value, String>> },
}

// =========================================================================
// 3. Hyper Service Implementation & Request Router
// =========================================================================

/// Core handler for incoming HTTP socket connections from agents or CLI scripts.
pub struct HermesService {
    app_tx: mpsc::Sender<HermesAppCommand>,
}

impl HermesService {
    pub fn new(app_tx: mpsc::Sender<HermesAppCommand>) -> Self {
        Self { app_tx }
    }

    /// Primary router translating incoming HTTP JSON payloads into strongly typed JSON-RPC responses.
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

        // Routing table for stateful methods requiring Main-Thread execution
        let response = match request.method.as_str() {
            "system.ping" => {
                JsonRpcResponse::success(request_id, serde_json::json!({ "status": "pong" }))
            }
            "system.capabilities" => {
                JsonRpcResponse::success(request_id, serde_json::json!({
                    "version": "1.0.0",
                    "features": ["pty_bypass", "rope_buffer", "progressive_input", "agent_sidebar"],
                    "subsystems": ["Metis", "Mneme", "Astraea", "Orpheus", "Theia"]
                }))
            }
            "workspace.switch_tab" => {
                let tab_index = request.params
                    .and_then(|p| p.get("index").and_then(|i| i.as_u64()))
                    .unwrap_or(0) as usize;

                let (tx, rx) = oneshot::channel();
                self.app_tx.send(HermesAppCommand::SwitchTab { tab_index, respond_to: tx }).await?;

                match rx.await? {
                    Ok(val) => JsonRpcResponse::success(request_id, val),
                    Err(e) => JsonRpcResponse::error(request_id, -32002, e),
                }
            }
            "surface.split" => {
                let horizontal = request.params
                    .and_then(|p| p.get("horizontal").and_then(|h| h.as_bool()))
                    .unwrap_or(true);

                let (tx, rx) = oneshot::channel();
                self.app_tx.send(HermesAppCommand::SplitPane { horizontal, respond_to: tx }).await?;

                match rx.await? {
                    Ok(val) => JsonRpcResponse::success(request_id, val),
                    Err(e) => JsonRpcResponse::error(request_id, -32003, e),
                }
            }
            "surface.send_text" => {
                let input_text = match request.params.and_then(|p| p.get("text").and_then(|t| t.as_str().map(String::from))) {
                    Some(t) => t,
                    None => return Ok(serde_json::to_vec(&JsonRpcResponse::error(request_id, -32602, "Invalid params: 'text' field required".to_string()))?),
                };

                let (tx, rx) = oneshot::channel();
                self.app_tx.send(HermesAppCommand::SendShellInput { input_text, respond_to: tx }).await?;

                match rx.await? {
                    Ok(val) => JsonRpcResponse::success(request_id, val),
                    Err(e) => JsonRpcResponse::error(request_id, -32004, e),
                }
            }
            "editor.insert" => {
                let text = match request.params.and_then(|p| p.get("text").and_then(|t| t.as_str().map(String::from))) {
                    Some(t) => t,
                    None => return Ok(serde_json::to_vec(&JsonRpcResponse::error(request_id, -32602, "Invalid params: 'text' field required".to_string()))?),
                };

                let (tx, rx) = oneshot::channel();
                self.app_tx.send(HermesAppCommand::InsertEditorText { text, respond_to: tx }).await?;

                match rx.await? {
                    Ok(val) => JsonRpcResponse::success(request_id, val),
                    Err(e) => JsonRpcResponse::error(request_id, -32005, e),
                }
            }
            // Catch-all route for AI Agent Lifecycle events (e.g. ai.session_start, ai.tool_use)
            method if method.starts_with("ai.") => {
                let payload = request.params.unwrap_or(Value::Null);
                let (tx, rx) = oneshot::channel();
                self.app_tx.send(HermesAppCommand::PublishAgentLifecycle {
                    event_name: method.to_string(),
                    payload,
                    respond_to: tx,
                }).await?;

                match rx.await? {
                    Ok(val) => JsonRpcResponse::success(request_id, val),
                    Err(e) => JsonRpcResponse::error(request_id, -32006, e),
                }
            }
            _ => {
                JsonRpcResponse::error(request_id, -32601, format!("Method not found: '{}'", request.method))
            }
        };

        Ok(serde_json::to_vec(&response)?)
    }
}

// =========================================================================
// 4. Secure Native Transport Binding (Unix Domain Sockets & named Pipes)
// =========================================================================

/// Core server struct running on a background thread pool, accepting local connections.
pub struct HermesRpcServer {
    app_tx: mpsc::Sender<HermesAppCommand>,
}

impl HermesRpcServer {
    pub fn new(app_tx: mpsc::Sender<HermesAppCommand>) -> Self {
        Self { app_tx }
    }

    /// Spawns the local socket server on a background thread.
    /// On Unix systems, binds to a safe Unix Domain Socket (mode 0600).
    /// On Windows, binds to a local Named Pipe.
    pub async fn run(self, socket_path: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let service_handler = Arc::new(HermesService::new(self.app_tx));

        #[cfg(unix)]
        {
            use tokio::net::UnixListener;
            use std::fs;
            use std::os::unix::fs::PermissionsExt;

            // Ensure parent directory exists and clean up leftover socket descriptors
            if let Some(parent) = std::path::Path::new(socket_path).parent() {
                fs::create_dir_all(parent)?;
            }
            let _ = fs::remove_file(socket_path);

            let listener = UnixListener::bind(socket_path)?;

            // Restrict access strictly to the launching user (mode 0600 / owner read+write only)
            let mut permissions = fs::metadata(socket_path)?.permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(socket_path, permissions)?;

            println!("[Hermes] Secure Unix socket listening at: {}", socket_path);

            loop {
                let (mut stream, peer_addr) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("[Hermes] Error accepting connection: {}", e);
                        continue;
                    }
                };

                // Security check: verify peer UID matches current process UID to prevent cross-user hijacking
                #[cfg(target_os = "linux")]
                {
                    use std::os::unix::io::AsRawFd;
                    let fd = stream.as_raw_fd();
                    let mut cred = unsafe { std::mem::zeroed::<libc::ucred>() };
                    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
                    
                    let res = unsafe {
                        libc::getsockopt(
                            fd,
                            libc::SOL_SOCKET,
                            libc::SO_PEERCRED,
                            &mut cred as *mut _ as *mut libc::c_void,
                            &mut len,
                        )
                    };

                    if res == 0 {
                        let current_uid = unsafe { libc::getuid() };
                        if cred.uid != current_uid {
                            eprintln!("[Hermes] Connection rejected: peer UID {} does not match server UID {}", cred.uid, current_uid);
                            continue; // Drop client connection instantly
                        }
                    }
                }

                let service_cloned = service_handler.clone();

                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 8192];
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};

                    loop {
                        match stream.read(&mut buffer).await {
                            Ok(0) => break, // Connection closed
                            Ok(bytes_read) => {
                                // Delegate parsing and routing
                                match service_cloned.handle_request(&buffer[..bytes_read]).await {
                                    Ok(response_bytes) => {
                                        if let Err(e) = stream.write_all(&response_bytes).await {
                                            eprintln!("[Hermes] Fail to write socket response: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[Hermes] Error processing RPC command: {}", e);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[Hermes] Socket read error: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
        }

        #[cfg(windows)]
        {
            // Windows-specificNamed Pipe implementation
            use tokio::net::windows::named_pipe::ServerOptions;

            // Formatted as standard Win32 named pipe path (e.g. \\.\pipe\well-hermes)
            let pipe_name = if socket_path.starts_with(r"\\.\pipe\") {
                socket_path.to_string()
            } else {
                format!(r"\\.\pipe\{}", socket_path.replace('/', "-").replace('\\', "-"))
            };

            println!("[Hermes] Named Pipe listening at: {}", pipe_name);

            loop {
                let mut server = ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(&pipe_name)?;

                // Wait for the client connection asynchronously
                server.connect().await?;

                let service_cloned = service_handler.clone();

                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 8192];
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};

                    loop {
                        match server.read(&mut buffer).await {
                            Ok(0) => break,
                            Ok(bytes_read) => {
                                match service_cloned.handle_request(&buffer[..bytes_read]).await {
                                    Ok(response_bytes) => {
                                        if let Err(e) = server.write_all(&response_bytes).await {
                                            eprintln!("[Hermes] Named Pipe write error: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[Hermes] Named Pipe processing error: {}", e);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[Hermes] Named Pipe read error: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
        }

        #[allow(unreachable_code)]
        Ok(())
    }
}
