//! well-shell: Metis Shell Logic Core & Logarithmic Suggestion Engine
//!
//! Subsystems:
//! - MetisHistory: High-performance prefix-trie indexing command history with O(k) queries.
//! - MetisExecutor: Stateful command execution and history.
//! - PtySession: PTY session with automated VT DA1/DA2/DSR query answers.
//! - HyperShellEngine: High-throughput asynchronous multi-stage pipeline runner.
//! - ExecutionRouter: Dual-path execution router (interactive PTY shim vs bounded IPC).
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
            let child_freq = if child.is_terminal {
                child.frequency
            } else {
                1
            };
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

/// Executes commands while retaining the working directory and history between requests.
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
        Self {
            history: MetisHistory::new(),
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

        let command_name = trimmed.split_whitespace().next().unwrap_or_default();
        match command_name {
            "cd" => {
                let target_text = trimmed[command_name.len()..].trim();
                let target = if target_text.is_empty() {
                    dirs_fallback()
                } else {
                    match parse_single_path_argument(target_text) {
                        Ok(path) => path,
                        Err(message) => {
                            return ShellOutput {
                                stdout: String::new(),
                                stderr: format!("cd: {message}\n"),
                                exit_code: 2,
                            };
                        }
                    }
                };

                let new_dir = if target.is_absolute() {
                    target.clone()
                } else {
                    self.current_dir.join(&target)
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
                        stderr: format!("cd: no such directory: {}\n", target.display()),
                        exit_code: 1,
                    }
                }
            }
            _ => match run_shell_command(trimmed, &self.current_dir) {
                Ok(out) => ShellOutput {
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    exit_code: out.status.code().unwrap_or(1),
                },
                Err(e) => ShellOutput {
                    stdout: String::new(),
                    stderr: format!("{command_name}: {e}\n"),
                    exit_code: 127,
                },
            },
        }
    }
}

fn parse_single_path_argument(input: &str) -> Result<PathBuf, &'static str> {
    let mut chars = input.chars().peekable();
    let quote = match chars.peek() {
        Some('\'' | '"') => chars.next(),
        _ => None,
    };
    let mut path = String::new();
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            path.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if Some(ch) == quote {
            if chars.any(|remaining| !remaining.is_whitespace()) {
                return Err("expected one directory argument");
            }
            return Ok(PathBuf::from(path));
        }
        if quote.is_none() && ch.is_whitespace() {
            return Err("expected one directory argument; quote paths containing spaces");
        }
        path.push(ch);
    }

    if escaped || quote.is_some() {
        return Err("unterminated escape or quote");
    }
    Ok(PathBuf::from(path))
}

fn run_shell_command(
    command: &str,
    current_dir: &std::path::Path,
) -> std::io::Result<std::process::Output> {
    #[cfg(windows)]
    let mut process = {
        let mut process = Command::new(env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()));
        process.arg("/C").arg(command);
        process
    };

    #[cfg(not(windows))]
    let mut process = {
        let mut process = Command::new("/bin/sh");
        process.arg("-c").arg(command);
        process
    };

    process
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
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
    pub rx: Receiver<ShellOutput>,
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

/// Initialize a shell command worker and its input/output channels.
pub fn init_shell() -> ShellHandle {
    let (tx, mut rx_cmd) = channel::<String>(32);
    let (tx_out, rx) = channel::<ShellOutput>(32);
    let history = Arc::new(Mutex::new(MetisHistory::new()));
    let history_clone = Arc::clone(&history);

    tokio::task::spawn_blocking(move || {
        let mut executor = MetisExecutor::new();
        while let Some(cmd) = rx_cmd.blocking_recv() {
            let output = executor.execute(&cmd);
            history_clone.blocking_lock().learn_command(&cmd);
            if tx_out.blocking_send(output).is_err() {
                break;
            }
        }
    });

    ShellHandle { history, tx, rx }
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
        let echo_res = exec.execute("echo 'sovereign kernel'");
        assert_eq!(echo_res.exit_code, 0);
        assert_eq!(echo_res.stdout.trim(), "sovereign kernel");

        let pwd_res = exec.execute("pwd");
        assert_eq!(pwd_res.exit_code, 0);
        assert!(!pwd_res.stdout.is_empty());
    }

    #[test]
    fn executor_runs_shell_syntax_and_preserves_exit_status() {
        let mut exec = MetisExecutor::new();
        let result = exec.execute("printf 'one two'; exit 7");
        assert_eq!(result.stdout, "one two");
        assert_eq!(result.exit_code, 7);
    }

    #[test]
    fn executor_changes_directory_with_quoted_path() {
        let directory =
            std::env::temp_dir().join(format!("well shell test {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let expected_directory = std::fs::canonicalize(&directory).unwrap();
        let command = format!("cd \"{}\"", directory.display());
        let mut exec = MetisExecutor::new();

        assert_eq!(exec.execute(&command).exit_code, 0);
        assert_eq!(
            exec.execute("pwd").stdout.trim(),
            expected_directory.display().to_string()
        );
        std::fs::remove_dir(&directory).unwrap();
    }

    #[tokio::test]
    async fn shell_handle_executes_commands_and_returns_full_result() {
        let mut shell = init_shell();
        assert!(shell.suggest("git").await.is_empty());
        shell
            .send_command("printf 'hello from shell'; exit 9".into())
            .await
            .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), shell.rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result.stdout, "hello from shell");
        assert_eq!(result.exit_code, 9);
        assert_eq!(
            shell.suggest("printf").await,
            vec!["printf 'hello from shell'; exit 9"]
        );
    }
}
