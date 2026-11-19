/// Integration tests for CaduceusServer length-prefix wire framing.
///
/// Each message is prefixed with a little-endian u32 body length so that
/// the server can reliably delimit JSON payloads even when TCP/Unix socket
/// reads arrive in fragments.
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use well_ipc::rpc::CaduceusServer;
use well_ipc::HermesAppCommand;

fn tmp_socket_path(name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::Builder::new()
        .prefix("well-framing-")
        .tempdir()
        .unwrap()
        .keep();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    root.join(format!("{name}.sock"))
}

/// Send a JSON-RPC request using the 4-byte LE length-prefix framing and
/// return the parsed response body (also length-prefixed on the wire).
async fn framed_rpc(stream: &mut UnixStream, request: serde_json::Value) -> serde_json::Value {
    let body = request.to_string();
    let len = body.len() as u32;
    stream.write_all(&len.to_le_bytes()).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();

    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await.unwrap();
    let resp_len = u32::from_le_bytes(header) as usize;

    let mut resp_body = vec![0u8; resp_len];
    stream.read_exact(&mut resp_body).await.unwrap();

    serde_json::from_slice(&resp_body).expect("response should be valid JSON")
}

/// Starts a CaduceusServer bound to `socket_path` in a background task.
async fn spawn_server(socket_path: PathBuf) -> mpsc::Receiver<HermesAppCommand> {
    let (app_tx, app_rx) = mpsc::channel::<HermesAppCommand>(16);
    let bound = CaduceusServer::new(app_tx)
        .bind(&socket_path)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = bound.serve_until(std::future::pending()).await;
    });

    app_rx
}

#[tokio::test]
async fn framed_ping_round_trip() {
    let socket_path = tmp_socket_path("ping");
    let _app_rx = spawn_server(socket_path.clone()).await;
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();

    let resp = framed_rpc(
        &mut stream,
        serde_json::json!({ "jsonrpc": "2.0", "method": "system.ping", "id": 1 }),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["status"], "pong");
    assert_eq!(resp["result"]["protocol"], "caduceus/1.0");
}

#[tokio::test]
async fn framed_capabilities_round_trip() {
    let socket_path = tmp_socket_path("caps");
    let _app_rx = spawn_server(socket_path.clone()).await;
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();

    let resp = framed_rpc(
        &mut stream,
        serde_json::json!({ "jsonrpc": "2.0", "method": "system.capabilities", "id": "caps" }),
    )
    .await;

    assert_eq!(resp["result"]["protocol"], "caduceus/1.0");
    assert_eq!(resp["result"]["gpu"]["remote_gpu_control"], false);
    assert!(!resp["result"]["active_methods"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!resp["result"]["unsupported_methods"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn framed_unknown_method_returns_method_not_found() {
    let socket_path = tmp_socket_path("unknown");
    let _app_rx = spawn_server(socket_path.clone()).await;
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();

    let resp = framed_rpc(
        &mut stream,
        serde_json::json!({ "jsonrpc": "2.0", "method": "does.not.exist", "id": 42 }),
    )
    .await;

    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32601);
    assert!(resp["error"]["message"]
        .as_str()
        .unwrap_or("")
        .contains("Method not found"));
}

#[tokio::test]
async fn framed_unsupported_method_returns_explicit_boundary_error() {
    let socket_path = tmp_socket_path("unsupported");
    let _app_rx = spawn_server(socket_path.clone()).await;
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();

    let resp = framed_rpc(
        &mut stream,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "editor.insert",
            "params": { "text": "hello" },
            "id": "ei"
        }),
    )
    .await;

    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32004);
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("editor.insert is unsupported"),
        "unexpected message: {}",
        resp["error"]["message"]
    );
}

#[tokio::test]
async fn framed_multiple_requests_on_same_connection() {
    let socket_path = tmp_socket_path("multi");
    let _app_rx = spawn_server(socket_path.clone()).await;
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();

    // Send 5 pings on the same connection to exercise the framing loop.
    for i in 0..5u32 {
        let resp = framed_rpc(
            &mut stream,
            serde_json::json!({ "jsonrpc": "2.0", "method": "system.ping", "id": i }),
        )
        .await;
        assert_eq!(resp["id"], i);
        assert_eq!(resp["result"]["status"], "pong");
    }
}
