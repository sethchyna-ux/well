use std::path::Path;
use std::fs;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use crate::{HermesAppCommand, HermesService};
use std::os::unix::fs::PermissionsExt;

pub struct CaduceusServer;

impl CaduceusServer {
    pub async fn start(socket_path: &Path, app_tx: mpsc::Sender<HermesAppCommand>) {
        if socket_path.exists() {
            let _ = fs::remove_file(socket_path);
        }
        
        let listener = UnixListener::bind(socket_path).expect("Failed to bind to Unix socket");
        
        // Set strict 0600 permissions
        let mut perms = fs::metadata(socket_path).expect("Failed to get socket metadata").permissions();
        perms.set_mode(0o600);
        fs::set_permissions(socket_path, perms).expect("Failed to set socket permissions");
        
        let my_uid = unsafe { libc::geteuid() };
        
        let service = Arc::new(HermesService::new(app_tx));
        
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    // Verify peer credentials (SO_PEERCRED on Linux/macOS)
                    if let Ok(cred) = stream.peer_cred() {
                        if cred.uid() != my_uid {
                            eprintln!("Rejected connection from unauthorized UID: {}", cred.uid());
                            continue;
                        }
                    } else {
                        eprintln!("Rejected connection: could not get peer credentials");
                        continue;
                    }
                    
                    let svc = Arc::clone(&service);
                    tokio::spawn(async move {
                        let mut buf = vec![0; 32768]; // Use a large buffer for big blocklist payloads
                        loop {
                            match stream.read(&mut buf).await {
                                Ok(0) => break, // Connection closed
                                Ok(n) => {
                                    // Try parsing the payload as a complete message. 
                                    // In a production system we should handle stream framing.
                                    match svc.clone().handle_request(&buf[..n]).await {
                                        Ok(response_bytes) => {
                                            if let Err(e) = stream.write_all(&response_bytes).await {
                                                eprintln!("Failed to write response: {}", e);
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Error handling request: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Socket read error: {}", e);
                                    break;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}
