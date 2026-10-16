//! well-shell: Metis Shell Logic Core & Logarithmic Suggestion Engine
//!
//! Subsystems:
//! - MetisHistory: High-performance prefix-trie indexing command history with O(k) queries.
//! - MetisExecutor: In-process shell parser and execution loop.
//! - PtySession: PTY session with automated VT DA1/DA2/DSR query answers.
//! - HyperShellEngine: High-throughput asynchronous multi-stage pipeline runner.
//! - ExecutionRouter: Dual-path execution router (interactive PTY shim vs lock-free Native IPC).
//! - KeyEncoder: Kitty Keyboard Protocol legacy transcoder.

pub mod engine;
pub mod graphics;
pub mod hypershell;
pub mod keyboard;
pub mod pty;

pub use engine::ExecutionRouter;
pub use graphics::{GraphicEvent, Interceptor};
pub use hypershell::{HyperShellEngine, PipelineStage, TaskResult};
pub use keyboard::KeyEncoder;
pub use pty::{handle_terminal_queries, PtySession};

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;

#[derive(Default, Debug, Clone)]
pub struct MetisTrieNode {
    pub children: HashMap<char, MetisTrieNode>,
    pub is_terminal: bool,
    pub frequency: u64,
}

/// MetisHistory: In-memory prefix-trie search engine.
/// Provides O(k) predictive suggestions on raw keystroke streams inside the shell thread.
#[derive(Debug, Clone)]
pub struct MetisHistory {
    root: MetisTrieNode,
    total_commands: u64,
}

impl Default for MetisHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl MetisHistory {
    pub fn new() -> Self {
        Self {
            root: MetisTrieNode::default(),
            total_commands: 0,
        }
    }

    /// Inserts a historically executed command into the trie.
    pub fn learn_command(&mut self, command: &str) {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut current = &mut self.root;
        for ch in trimmed.chars() {
            current = current.children.entry(ch).or_default();
        }
        current.is_terminal = true;
        current.frequency += 1;
        self.total_commands += 1;
    }

    /// O(k) retrieval of the most frequent historical auto-suggestion matching prefix.
    pub fn get_autosuggestion(&self, prefix: &str) -> Option<String> {
        if prefix.is_empty() {
            return None;
        }

        let mut current = &self.root;
        for ch in prefix.chars() {
            if let Some(next) = current.children.get(&ch) {
                current = next;
            } else {
                return None;
            }
        }

        let mut suffix = String::new();
        if Self::find_dominant_path(current, &mut suffix) {
            Some(format!("{}{}", prefix, suffix))
        } else {
            None
        }
    }

    /// Collect up to `limit` suggestions matching a given prefix.
    pub fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut current = &self.root;
        for ch in prefix.chars() {
            if let Some(next) = current.children.get(&ch) {
                current = next;
            } else {
                return Vec::new();
            }
        }

        let mut results = Vec::new();
        Self::collect_all(current, prefix.to_string(), &mut results, limit);
        results
    }

    fn collect_all(node: &MetisTrieNode, current: String, out: &mut Vec<String>, limit: usize) {
        if out.len() >= limit {
            return;
        }
        if node.is_terminal {
            out.push(current.clone());
        }
        for (ch, child) in &node.children {
            let mut next = current.clone();
            next.push(*ch);
            Self::collect_all(child, next, out, limit);
            if out.len() >= limit {
                break;
            }
        }
    }

    fn find_dominant_path(node: &MetisTrieNode, path: &mut String) -> bool {
        if node.children.is_empty() {
            return node.is_terminal;
        }

        let mut best_char = None;
        let mut max_freq = 0;
        let mut best_node = None;

        for (ch, child) in &node.children {
            let child_freq = if child.is_terminal { child.frequency } else { 1 };
            if child_freq > max_freq {
                max_freq = child_freq;
                best_char = Some(*ch);
                best_node = Some(child);
            }
        }

        if let (Some(ch), Some(next)) = (best_char, best_node) {
            path.push(ch);
            Self::find_dominant_path(next, path);
            true
        } else {
            node.is_terminal
        }
    }

    pub fn total_commands(&self) -> u64 {
        self.total_commands
    }
}

/// Execution outcome returned by MetisExecutor
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// MetisExecutor: In-process command tokenizer and executor
pub struct MetisExecutor {
    pub history: MetisHistory,
    pub current_dir: PathBuf,
}

impl Default for MetisExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MetisExecutor {
    pub fn new() -> Self {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut history = MetisHistory::new();
        history.learn_command("git status");
        history.learn_command("cargo build");
        history.learn_command("cargo check");
        history.learn_command("cargo test");
        history.learn_command("ls -la");

        Self {
            history,
            current_dir,
        }
    }

    pub fn execute(&mut self, input: &str) -> ShellOutput {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return ShellOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            };
        }

        self.history.learn_command(trimmed);

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "cd" => {
                let target = if args.is_empty() {
                    dirs_fallback()
                } else {
                    PathBuf::from(args[0])
                };

                let new_dir = if target.is_absolute() {
                    target
                } else {
                    self.current_dir.join(target)
                };

                if new_dir.is_dir() {
                    self.current_dir = new_dir;
                    ShellOutput {
                        stdout: String::new(),
                        stderr: String::new(),
                        exit_code: 0,
                    }
                } else {
                    ShellOutput {
                        stdout: String::new(),
                        stderr: format!("cd: no such directory: {}\n", args[0]),
                        exit_code: 1,
                    }
                }
            }
            "pwd" => ShellOutput {
                stdout: format!("{}\n", self.current_dir.display()),
                stderr: String::new(),
                exit_code: 0,
            },
            "echo" => ShellOutput {
                stdout: format!("{}\n", args.join(" ")),
                stderr: String::new(),
                exit_code: 0,
            },
            _ => match Command::new(cmd)
                .args(args)
                .current_dir(&self.current_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
            {
                Ok(out) => ShellOutput {
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    exit_code: out.status.code().unwrap_or(0),
                },
                Err(e) => ShellOutput {
                    stdout: String::new(),
                    stderr: format!("{}: command not found ({})\n", cmd, e),
                    exit_code: 127,
                },
            },
        }
    }
}

fn dirs_fallback() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

/// Public handle for asynchronous interactions with the shell subsystem.
#[derive(Debug)]
pub struct ShellHandle {
    pub history: Arc<Mutex<MetisHistory>>,
    pub tx: Sender<String>,
    pub rx: Receiver<String>,
}

impl ShellHandle {
    pub async fn send_command(&self, cmd: String) -> Result<(), String> {
        self.tx.send(cmd).await.map_err(|e| e.to_string())
    }

    pub async fn suggest(&self, prefix: &str) -> Vec<String> {
        let hist = self.history.lock().await;
        hist.suggestions(prefix, 5)
    }
}

/// Initialise the shell subsystem handle
pub fn init_shell() -> ShellHandle {
    let (tx, mut rx_cmd) = channel::<String>(32);
    let (tx_out, rx) = channel::<String>(32);
    let history = Arc::new(Mutex::new(MetisHistory::new()));
    let history_clone = Arc::clone(&history);

    tokio::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            {
                let mut hist = history_clone.lock().await;
                hist.learn_command(&cmd);
            }
            let _ = tx_out.send(format!("Executed: {}", cmd)).await;
        }
    });

    ShellHandle {
        history,
        tx,
        rx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metis_trie_suggestions() {
        let mut history = MetisHistory::new();
        history.learn_command("cargo build --release");
        history.learn_command("cargo test");

        assert_eq!(
            history.get_autosuggestion("cargo b"),
            Some("cargo build --release".to_string())
        );
        assert_eq!(
            history.get_autosuggestion("cargo t"),
            Some("cargo test".to_string())
        );
        assert_eq!(history.get_autosuggestion("unknown"), None);
    }

    #[test]
    fn test_metis_executor_builtins() {
        let mut exec = MetisExecutor::new();
        let echo_res = exec.execute("echo sovereign kernel");
        assert_eq!(echo_res.exit_code, 0);
        assert_eq!(echo_res.stdout.trim(), "sovereign kernel");

        let pwd_res = exec.execute("pwd");
        assert_eq!(pwd_res.exit_code, 0);
        assert!(!pwd_res.stdout.is_empty());
    }
}
