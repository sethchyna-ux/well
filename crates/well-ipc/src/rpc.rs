//! Local HTTP JSON-RPC and WebSocket transport with authenticated Unix peers.
//! Capabilities are registered by the application; unregistered UI operations
//! cannot report success or advertise features that the application does not run.
use axum::body::{to_bytes, Body};
use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_BATCH: usize = 64;
type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
            jsonrpc: "2.0".into(),
            result: Some(result),
            error: None,
            id,
        }
    }
    pub fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
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
fn error(id: Value, code: i32, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "error": {"code": code, "message": message.into()}, "id": id})
}

#[derive(Debug)]
pub enum HermesAppCommand {
    ReadSurface {
        surface_id: Option<u64>,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    WorkspaceState {
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    NewTab {
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    SendSurfaceInput {
        surface_id: u64,
        input_text: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    SwitchTab {
        tab_index: usize,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    SplitPane {
        horizontal: bool,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    SendShellInput {
        input_text: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    InsertEditorText {
        text: String,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
    PublishAgentLifecycle {
        event_name: String,
        payload: Value,
        respond_to: oneshot::Sender<Result<Value, String>>,
    },
}

pub struct HermesService {
    app_tx: mpsc::Sender<HermesAppCommand>,
    methods: BTreeSet<String>,
    dispatch_timeout: Duration,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl HermesService {
    /// Construct a service exposing only its own implemented system operations.
    pub fn new(app_tx: mpsc::Sender<HermesAppCommand>) -> Self {
        Self::with_methods(app_tx, &[], || {})
            .expect("System service registers no application methods")
    }
    pub fn with_methods(
        app_tx: mpsc::Sender<HermesAppCommand>,
        methods: &[&str],
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<Self> {
        for method in methods {
            if !matches!(
                *method,
                "system.ping"
                    | "system.capabilities"
                    | "workspace.state"
                    | "workspace.new_tab"
                    | "surface.read_text"
                    | "workspace.switch_tab"
                    | "surface.split"
                    | "surface.send_text"
                    | "editor.insert"
                    | "ai.*"
            ) && !method.starts_with("ai.")
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("No dispatcher implements {method}"),
                ));
            }
        }
        let mut methods: BTreeSet<String> = methods.iter().map(|m| (*m).to_owned()).collect();
        methods.insert("system.ping".into());
        methods.insert("system.capabilities".into());
        Ok(Self {
            app_tx,
            methods,
            dispatch_timeout: Duration::from_secs(5),
            wake: Arc::new(wake),
        })
    }
    pub async fn handle_request(self: Arc<Self>, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Ok(serde_json::to_vec(&error(
                Value::Null,
                -32600,
                "Request exceeds 64 KiB",
            ))?);
        }
        let value: Value = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(_) => {
                return Ok(serde_json::to_vec(&error(
                    Value::Null,
                    -32700,
                    "Parse error",
                ))?)
            }
        };
        let response = if let Value::Array(batch) = value {
            if batch.is_empty() || batch.len() > MAX_BATCH {
                Some(error(
                    Value::Null,
                    -32600,
                    "Batch must contain between 1 and 64 requests",
                ))
            } else {
                let responses = futures_util::future::join_all(
                    batch.into_iter().map(|request| self.handle_one(request)),
                )
                .await;
                let responses: Vec<Value> = responses.into_iter().flatten().collect();
                if responses.is_empty() {
                    None
                } else {
                    Some(Value::Array(responses))
                }
            }
        } else {
            self.handle_one(value).await
        };
        match response {
            Some(response) => Ok(serde_json::to_vec(&response)?),
            None => Ok(Vec::new()),
        }
    }
    async fn handle_one(&self, value: Value) -> Option<Value> {
        let Some(object) = value.as_object() else {
            return Some(error(Value::Null, -32600, "Invalid request"));
        };
        let id = object.get("id").cloned();
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || !object.get("method").is_some_and(Value::is_string)
            || id
                .as_ref()
                .is_some_and(|id| !matches!(id, Value::String(_) | Value::Number(_) | Value::Null))
        {
            return Some(error(Value::Null, -32600, "Invalid request"));
        }
        let method = object["method"].as_str().unwrap();
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        let result = if !params.is_null() && !params.is_object() && !params.is_array() {
            Err((-32602, "Params must be an object or array".into()))
        } else if !self.methods.contains(method)
            && !(method.starts_with("ai.") && self.methods.contains("ai.*"))
        {
            Err((-32601, format!("Method not found: {method}")))
        } else {
            match method {
                "system.ping" => Ok(json!({"status":"pong"})),
                "system.capabilities" => {
                    Ok(json!({"version": env!("CARGO_PKG_VERSION"), "methods": self.methods}))
                }
                _ => self.dispatch(method, params).await,
            }
        };
        let id = id?; // A valid notification never receives a response, including on error.
        Some(match result {
            Ok(result) => json!({"jsonrpc":"2.0", "result":result, "id":id}),
            Err((code, message)) => error(id, code, message),
        })
    }
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, (i32, String)> {
        let (tx, rx) = oneshot::channel();
        let invalid = || (-32602, "Missing or invalid method parameters".into());
        let command = match method {
            "surface.read_text" => HermesAppCommand::ReadSurface {
                surface_id: match params.get("surface_id") {
                    Some(value) => Some(value.as_u64().ok_or_else(invalid)?),
                    None => None,
                },
                respond_to: tx,
            },
            "workspace.state" => HermesAppCommand::WorkspaceState { respond_to: tx },
            "workspace.new_tab" => HermesAppCommand::NewTab { respond_to: tx },
            "workspace.switch_tab" => HermesAppCommand::SwitchTab {
                tab_index: params
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(invalid)?,
                respond_to: tx,
            },
            "surface.split" => HermesAppCommand::SplitPane {
                horizontal: params
                    .get("horizontal")
                    .and_then(Value::as_bool)
                    .ok_or_else(invalid)?,
                respond_to: tx,
            },
            "surface.send_text" if params.get("surface_id").is_some() => {
                HermesAppCommand::SendSurfaceInput {
                    surface_id: params
                        .get("surface_id")
                        .and_then(Value::as_u64)
                        .ok_or_else(invalid)?,
                    input_text: params
                        .get("text")
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?
                        .to_owned(),
                    respond_to: tx,
                }
            }
            "surface.send_text" => HermesAppCommand::SendShellInput {
                input_text: params
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?
                    .to_owned(),
                respond_to: tx,
            },
            "editor.insert" => HermesAppCommand::InsertEditorText {
                text: params
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?
                    .to_owned(),
                respond_to: tx,
            },
            method if method.starts_with("ai.") => HermesAppCommand::PublishAgentLifecycle {
                event_name: method.to_owned(),
                payload: params,
                respond_to: tx,
            },
            _ => return Err((-32601, format!("Method not found: {method}"))),
        };
        tokio::time::timeout(self.dispatch_timeout, async {
            self.app_tx
                .send(command)
                .await
                .map_err(|_| (-32000, "Application unavailable".into()))?;
            (self.wake)();
            rx.await
                .map_err(|_| (-32000, "Application response cancelled".into()))?
                .map_err(|message| (-32002, message))
        })
        .await
        .map_err(|_| (-32001, "Application request timed out".into()))?
    }
}

#[derive(Clone)]
struct TransportState {
    service: Arc<HermesService>,
    stop: CancellationToken,
}
async fn http_rpc(State(state): State<TransportState>, request: Request<Body>) -> Response {
    let body = match tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(request.into_body(), MAX_REQUEST_BYTES),
    )
    .await
    {
        Ok(Ok(body)) => body,
        Ok(Err(_)) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "Request body exceeds 64 KiB").into_response()
        }
        Err(_) => return (StatusCode::REQUEST_TIMEOUT, "Request body timed out").into_response(),
    };
    match state.service.handle_request(&body).await {
        Ok(response) if response.is_empty() => StatusCode::NO_CONTENT.into_response(),
        Ok(response) => ([(header::CONTENT_TYPE, "application/json")], response).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
async fn websocket_rpc(State(state): State<TransportState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .max_message_size(MAX_REQUEST_BYTES)
        .max_frame_size(MAX_REQUEST_BYTES)
        .on_upgrade(move |socket| websocket_session(socket, state))
}
async fn websocket_session(mut socket: WebSocket, state: TransportState) {
    loop {
        let message = tokio::select! { _ = state.stop.cancelled() => break, message = socket.recv() => message };
        match message {
            Some(Ok(Message::Text(text))) => {
                let response = tokio::select! { _ = state.stop.cancelled() => break, response = state.service.clone().handle_request(text.as_bytes()) => response };
                match response {
                    Ok(bytes) if !bytes.is_empty() => {
                        let Ok(text) = String::from_utf8(bytes) else {
                            break;
                        };
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            Some(Ok(Message::Binary(_))) => {
                let _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: close_code::UNSUPPORTED,
                        reason: "JSON-RPC requires text frames".into(),
                    })))
                    .await;
                break;
            }
            Some(Ok(Message::Ping(bytes))) => {
                if socket.send(Message::Pong(bytes)).await.is_err() {
                    break;
                }
            }
            Some(Ok(Message::Pong(_))) => {}
            _ => break,
        }
    }
    let _ = socket.close().await;
}

#[derive(Default)]
pub struct ServerStats {
    active_connections: AtomicUsize,
    rejected_connections: AtomicUsize,
}
impl ServerStats {
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }
    pub fn rejected_connections(&self) -> usize {
        self.rejected_connections.load(Ordering::Relaxed)
    }
}

pub struct HermesRpcServer {
    service: Arc<HermesService>,
}
impl HermesRpcServer {
    pub fn new(app_tx: mpsc::Sender<HermesAppCommand>) -> Self {
        Self {
            service: Arc::new(HermesService::new(app_tx)),
        }
    }
    pub fn with_service(service: Arc<HermesService>) -> Self {
        Self { service }
    }
    #[cfg(unix)]
    pub async fn bind(self, path: impl AsRef<Path>) -> Result<BoundRpcServer, Error> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Socket requires a private parent directory",
                )
            })?;
        ensure_private_directory(parent)?;
        match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() }
                {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "Socket path is occupied by an unrelated file",
                    )
                    .into());
                }
                match tokio::time::timeout(
                    Duration::from_millis(250),
                    tokio::net::UnixStream::connect(path),
                )
                .await
                {
                    Ok(Err(e))
                        if matches!(
                            e.kind(),
                            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                        ) =>
                    {
                        std::fs::remove_file(path)?
                    }
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            "Socket already has a listener",
                        )
                        .into())
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let listener = tokio::net::UnixListener::bind(path)?;
        let metadata = std::fs::symlink_metadata(path)?;
        let guard = SocketGuard {
            path: path.to_owned(),
            dev: metadata.dev(),
            ino: metadata.ino(),
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        let stop = CancellationToken::new();
        let stats = Arc::new(ServerStats::default());
        let listener = PrivateListener {
            listener,
            capacity: Arc::new(tokio::sync::Semaphore::new(32)),
            stats: stats.clone(),
            stop: stop.clone(),
        };
        let app = Router::new()
            .route("/", post(http_rpc))
            .route("/rpc", post(http_rpc).get(websocket_rpc))
            .with_state(TransportState {
                service: self.service,
                stop: stop.clone(),
            });
        Ok(BoundRpcServer {
            listener,
            app,
            stop,
            stats,
            guard,
        })
    }
    #[cfg(unix)]
    pub async fn run(self, path: &str) -> Result<(), Error> {
        self.bind(path)
            .await?
            .serve_until(std::future::pending())
            .await
    }
}

#[cfg(unix)]
fn ensure_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    if !path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Socket directory must be owned by this user with mode 0700",
        ));
    }
    Ok(())
}
#[cfg(unix)]
struct SocketGuard {
    path: PathBuf,
    dev: u64,
    ino: u64,
}
#[cfg(unix)]
impl Drop for SocketGuard {
    fn drop(&mut self) {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        if let Ok(meta) = std::fs::symlink_metadata(&self.path) {
            if meta.file_type().is_socket() && meta.dev() == self.dev && meta.ino() == self.ino {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}
#[cfg(unix)]
pub struct BoundRpcServer {
    listener: PrivateListener,
    app: Router,
    stop: CancellationToken,
    pub stats: Arc<ServerStats>,
    guard: SocketGuard,
}
#[cfg(unix)]
impl BoundRpcServer {
    pub fn path(&self) -> &Path {
        &self.guard.path
    }
    pub async fn serve_until(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Error> {
        let Self {
            listener,
            app,
            stop,
            stats: _,
            guard: _guard,
        } = self;
        let _cancel_on_drop = CancelOnDrop(stop.clone());
        let signal = stop.clone();
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown.await;
                signal.cancel();
            })
            .await?;
        Ok(())
    }
}
struct CancelOnDrop(CancellationToken);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
#[cfg(unix)]
struct PrivateListener {
    listener: tokio::net::UnixListener,
    capacity: Arc<tokio::sync::Semaphore>,
    stats: Arc<ServerStats>,
    stop: CancellationToken,
}
#[cfg(unix)]
impl axum::serve::Listener for PrivateListener {
    type Io = PrivateStream;
    type Addr = tokio::net::unix::SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let permit = self
                .capacity
                .clone()
                .acquire_owned()
                .await
                .expect("Private connection semaphore is never closed");
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    if !stream
                        .peer_cred()
                        .is_ok_and(|credentials| credentials.uid() == unsafe { libc::geteuid() })
                    {
                        self.stats
                            .rejected_connections
                            .fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    self.stats
                        .active_connections
                        .fetch_add(1, Ordering::Relaxed);
                    return (
                        PrivateStream {
                            stream,
                            _permit: permit,
                            stats: self.stats.clone(),
                            cancelled: Box::pin(self.stop.clone().cancelled_owned()),
                        },
                        addr,
                    );
                }
                Err(e) => {
                    eprintln!("Well RPC accept: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}
#[cfg(unix)]
struct PrivateStream {
    stream: tokio::net::UnixStream,
    _permit: tokio::sync::OwnedSemaphorePermit,
    stats: Arc<ServerStats>,
    cancelled: std::pin::Pin<Box<tokio_util::sync::WaitForCancellationFutureOwned>>,
}
#[cfg(unix)]
impl Drop for PrivateStream {
    fn drop(&mut self) {
        self.stats
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);
    }
}
#[cfg(unix)]
impl tokio::io::AsyncRead for PrivateStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.cancelled.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(Ok(()));
        }
        std::pin::Pin::new(&mut this.stream).poll_read(cx, buf)
    }
}
#[cfg(unix)]
impl tokio::io::AsyncWrite for PrivateStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.cancelled.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Server shutting down",
            )));
        }
        std::pin::Pin::new(&mut this.stream).poll_write(cx, buf)
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn service() -> Arc<HermesService> {
        let (tx, _rx) = mpsc::channel(8);
        Arc::new(HermesService::new(tx))
    }
    async fn request(service: Arc<HermesService>, value: Value) -> Option<Value> {
        let bytes = service
            .handle_request(&serde_json::to_vec(&value).unwrap())
            .await
            .unwrap();
        if bytes.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(&bytes).unwrap())
        }
    }
    #[tokio::test]
    async fn validates_protocol_and_handles_batches_notifications_and_ids() {
        let service = service();
        let response = request(
            service.clone(),
            json!({"jsonrpc":"2.0", "method":"system.ping", "id":null}),
        )
        .await
        .unwrap();
        assert_eq!(response["result"]["status"], "pong");
        assert!(response["id"].is_null());
        assert!(request(
            service.clone(),
            json!({"jsonrpc":"2.0", "method":"system.ping"})
        )
        .await
        .is_none());
        let response = request(
            service.clone(),
            json!([
                {"jsonrpc":"2.0", "method":"system.ping"},
                {"jsonrpc":"2.0", "method":"system.ping", "id":"a"},
                {"jsonrpc":"1.0", "method":"system.ping", "id":true},
                {"jsonrpc":"2.0", "method":"surface.send_text", "id":4}
            ]),
        )
        .await
        .unwrap();
        assert_eq!(response.as_array().unwrap().len(), 3);
        assert_eq!(response[0]["id"], "a");
        assert_eq!(response[1]["error"]["code"], -32600);
        assert_eq!(response[2]["error"]["code"], -32601);
        let response = request(service.clone(), json!([])).await.unwrap();
        assert_eq!(response["error"]["code"], -32600);
        let response: Value =
            serde_json::from_slice(&service.handle_request(b"invalid").await.unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32700);
    }
    #[tokio::test]
    async fn capabilities_include_only_registered_implemented_application_methods() {
        let response = request(
            service(),
            json!({"jsonrpc":"2.0", "method":"system.capabilities", "id":1}),
        )
        .await
        .unwrap();
        assert_eq!(response["result"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            response["result"]["methods"],
            json!(["system.capabilities", "system.ping"])
        );
    }
    #[tokio::test]
    async fn dispatch_validates_parameters_and_times_out_without_an_application() {
        let (tx, mut rx) = mpsc::channel(8);
        let wakes = Arc::new(AtomicUsize::new(0));
        let signalled = wakes.clone();
        let mut handler = HermesService::with_methods(tx, &["surface.send_text"], move || {
            signalled.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();
        handler.dispatch_timeout = Duration::from_millis(50);
        let handler = Arc::new(handler);
        let invalid = request(
            handler.clone(),
            json!({"jsonrpc":"2.0", "method":"surface.send_text", "params":{}, "id":1}),
        )
        .await
        .unwrap();
        assert_eq!(invalid["error"]["code"], -32602);
        assert_eq!(wakes.load(Ordering::Relaxed), 0);
        let task = tokio::spawn(request(
            handler.clone(),
            json!({"jsonrpc":"2.0", "method":"surface.send_text", "params":{"text":"abc"}, "id":2}),
        ));
        match rx.recv().await.unwrap() {
            HermesAppCommand::SendShellInput {
                input_text,
                respond_to,
            } => {
                assert_eq!(input_text, "abc");
                respond_to.send(Ok(json!({"accepted":3}))).unwrap();
            }
            _ => panic!("wrong dispatch"),
        }
        assert_eq!(task.await.unwrap().unwrap()["result"]["accepted"], 3);
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
        let timed = request(
            handler,
            json!({"jsonrpc":"2.0", "method":"surface.send_text", "params":{"text":"abc"}, "id":3}),
        )
        .await
        .unwrap();
        assert_eq!(timed["error"]["code"], -32001);
    }
    #[cfg(unix)]
    fn temp() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .prefix("well-rpc-")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }
    #[test]
    fn notification_serialization_and_capability_registration_are_explicit() {
        let notification = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "system.ping".into(),
            params: None,
            id: None,
        };
        let value = serde_json::to_value(notification).unwrap();
        assert!(value.get("id").is_none());
        let (tx, _rx) = mpsc::channel(1);
        assert!(HermesService::with_methods(tx, &["imaginary.feature"], || {}).is_err());
    }
    #[cfg(unix)]
    async fn start(
        path: &Path,
    ) -> (
        oneshot::Sender<()>,
        tokio::task::JoinHandle<Result<(), Error>>,
        Arc<ServerStats>,
    ) {
        let server = HermesRpcServer::with_service(service())
            .bind(path)
            .await
            .unwrap();
        let stats = server.stats.clone();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(server.serve_until(async {
            let _ = stopped.await;
        }));
        (stop, task, stats)
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn fragmented_http_is_framed_and_socket_is_private_and_cleaned_up() {
        use std::os::unix::fs::PermissionsExt;
        let temp = temp();
        let path = temp.path().join("rpc.sock");
        let (stop, task, stats) = start(&path).await;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let mut socket = tokio::net::UnixStream::connect(&path).await.unwrap();
        let body = br#"{"jsonrpc":"2.0","method":"system.ping","id":42}"#;
        let headers = format!("POST /rpc HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len());
        for chunk in headers.as_bytes().chunks(3) {
            socket.write_all(chunk).await.unwrap();
        }
        for byte in body {
            socket.write_all(&[*byte]).await.unwrap();
            tokio::task::yield_now().await;
        }
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        let json: Value = serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(json["id"], 42);
        assert_eq!(json["result"]["status"], "pong");
        drop(socket);
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
        assert!(!path.exists());
        assert_eq!(stats.active_connections(), 0);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn websocket_jsonrpc_and_shutdown_close_the_connection() {
        let temp = temp();
        let path = temp.path().join("rpc.sock");
        let (stop, task, _) = start(&path).await;
        let socket = tokio::net::UnixStream::connect(&path).await.unwrap();
        let (mut ws, response) = tokio_tungstenite::client_async("ws://localhost/rpc", socket)
            .await
            .unwrap();
        assert_eq!(response.status(), 101);
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"jsonrpc":"2.0","method":"system.ping","id":"ws"}"#.into(),
        ))
        .await
        .unwrap();
        let message = ws.next().await.unwrap().unwrap().into_text().unwrap();
        let response: Value = serde_json::from_str(&message).unwrap();
        assert_eq!(response["id"], "ws");
        assert_eq!(response["result"]["status"], "pong");
        stop.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let message = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .unwrap();
        assert!(!matches!(
            message,
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(_)))
        ));
        assert!(!path.exists());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn refuses_regular_files_symlinks_and_live_sockets() {
        let temp = temp();
        let path = temp.path().join("rpc.sock");
        std::fs::write(&path, b"keep").unwrap();
        assert!(HermesRpcServer::with_service(service())
            .bind(&path)
            .await
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
        let link = temp.path().join("link.sock");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(HermesRpcServer::with_service(service())
            .bind(&link)
            .await
            .is_err());
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let live = temp.path().join("live.sock");
        let server = HermesRpcServer::with_service(service())
            .bind(&live)
            .await
            .unwrap();
        assert!(HermesRpcServer::with_service(service())
            .bind(&live)
            .await
            .is_err());
        assert!(live.exists());
        drop(server);
        assert!(!live.exists());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cleanup_preserves_replacement_files_and_refuses_public_directories() {
        use std::os::unix::fs::PermissionsExt;
        let temp = temp();
        let path = temp.path().join("rpc.sock");
        let server = HermesRpcServer::with_service(service())
            .bind(&path)
            .await
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"replacement").unwrap();
        drop(server);
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(HermesRpcServer::with_service(service())
            .bind(temp.path().join("other.sock"))
            .await
            .is_err());
    }
}
