//! Language Server Protocol (LSP) client channel.
//! Streams diagnostics, warnings, and formatting folding directly into the terminal viewport.

use crate::mneme::{DiagnosticSeverity, LspDiagnostic};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

pub struct LspChannel {
    server_name: String,
    to_server_tx: Option<Sender<Value>>,
    diagnostics_rx: Receiver<Vec<LspDiagnostic>>,
    running: Arc<AtomicBool>,
    child: Option<Child>,
    worker: Option<JoinHandle<()>>,
    doc_version: Arc<AtomicU64>,
}

impl LspChannel {
    /// Attempt to spawn a background language server process (e.g. "bash-language-server start").
    /// If unavailable, operates in passive mode where diagnostics come from the Safety AST Validator.
    pub fn spawn_or_offline(server_command: &str, args: &[&str]) -> Self {
        let (diag_tx, diag_rx) = channel::<Vec<LspDiagnostic>>();
        let (to_server_tx, to_server_rx) = channel::<Value>();
        let running = Arc::new(AtomicBool::new(true));
        let doc_version = Arc::new(AtomicU64::new(1));

        let mut child_proc = Command::new(server_command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok();

        let mut worker = None;

        if let Some(mut child) = child_proc.take() {
            let mut stdin = child.stdin.take().expect("Stdin was piped");
            let stdout = child.stdout.take().expect("Stdout was piped");
            let run_clone = running.clone();

            // Writer thread for sending JSON-RPC to LSP stdin
            let writer_run = run_clone.clone();
            thread::Builder::new()
                .name("lsp-jsonrpc-writer".into())
                .spawn(move || {
                    while writer_run.load(Ordering::Relaxed) {
                        if let Ok(msg) = to_server_rx.recv() {
                            if let Ok(body) = serde_json::to_string(&msg) {
                                let header = format!("Content-Length: {}\r\n\r\n", body.len());
                                if stdin.write_all(header.as_bytes()).is_err()
                                    || stdin.write_all(body.as_bytes()).is_err()
                                    || stdin.flush().is_err()
                                {
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                })
                .ok();

            // Reader thread parsing JSON-RPC messages from LSP stdout
            let reader_handle = thread::Builder::new()
                .name("lsp-jsonrpc-reader".into())
                .spawn(move || {
                    let mut reader = BufReader::new(stdout);
                    let mut line_buf = String::new();

                    while run_clone.load(Ordering::Relaxed) {
                        line_buf.clear();
                        let mut content_length: Option<usize> = None;

                        // Parse HTTP-like headers
                        loop {
                            line_buf.clear();
                            match reader.read_line(&mut line_buf) {
                                Ok(0) | Err(_) => return,
                                Ok(_) => {
                                    let trimmed = line_buf.trim();
                                    if trimmed.is_empty() {
                                        break;
                                    }
                                    if let Some(len_str) = trimmed.strip_prefix("Content-Length:") {
                                        if let Ok(len) = len_str.trim().parse::<usize>() {
                                            content_length = Some(len);
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(len) = content_length {
                            let mut body_buf = vec![0u8; len];
                            if reader.read_exact(&mut body_buf).is_ok() {
                                if let Ok(val) = serde_json::from_slice::<Value>(&body_buf) {
                                    if let Some(method) = val.get("method").and_then(Value::as_str)
                                    {
                                        if method == "textDocument/publishDiagnostics" {
                                            if let Some(params) = val.get("params") {
                                                let diags = parse_lsp_diagnostics(params);
                                                let _ = diag_tx.send(diags);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
                .ok();

            worker = reader_handle;
            child_proc = Some(child);
        }

        Self {
            server_name: server_command.to_string(),
            to_server_tx: if child_proc.is_some() {
                Some(to_server_tx)
            } else {
                None
            },
            diagnostics_rx: diag_rx,
            running,
            child: child_proc,
            worker,
            doc_version,
        }
    }

    /// True if an external language server process is active.
    pub fn is_connected(&self) -> bool {
        self.child.is_some()
    }

    /// Name of the language server binary or command.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Notify the language server of document changes.
    pub fn did_change(&self, uri: &str, text: &str) {
        let version = self.doc_version.fetch_add(1, Ordering::SeqCst);
        if let Some(tx) = &self.to_server_tx {
            let msg = json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "version": version,
                    },
                    "contentChanges": [
                        { "text": text }
                    ]
                }
            });
            let _ = tx.send(msg);
        }
    }

    /// Poll for any freshly arrived LSP diagnostics.
    pub fn poll_diagnostics(&self) -> Option<Vec<LspDiagnostic>> {
        let mut latest = None;
        while let Ok(diags) = self.diagnostics_rx.try_recv() {
            latest = Some(diags);
        }
        latest
    }
}

impl Drop for LspChannel {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn parse_lsp_diagnostics(params: &Value) -> Vec<LspDiagnostic> {
    let mut results = Vec::new();
    let Some(diags_arr) = params.get("diagnostics").and_then(Value::as_array) else {
        return results;
    };

    for item in diags_arr {
        let message = item
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let severity = match item.get("severity").and_then(Value::as_u64).unwrap_or(1) {
            1 => DiagnosticSeverity::Error,
            2 => DiagnosticSeverity::Warning,
            3 => DiagnosticSeverity::Information,
            _ => DiagnosticSeverity::Hint,
        };
        // Simple character range placeholder; caller maps line/col if needed
        results.push(LspDiagnostic {
            range: 0..0,
            severity,
            message,
        });
    }

    results
}
