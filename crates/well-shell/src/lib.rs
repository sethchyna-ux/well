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
pub mod history;
pub use history::{MetisHistory, MetisTrieNode};
pub mod graphics;
pub mod hypershell;
pub mod keyboard;
pub mod pty;
pub mod service;
pub use service::{init_shell, ShellHandle};

pub use engine::ExecutionRouter;
pub use graphics::{GraphicEvent, Interceptor};
pub use hypershell::{HyperShellEngine, PipelineStage, TaskResult};
pub use keyboard::KeyEncoder;
pub use pty::{handle_terminal_queries, PtySession};

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
        let history = MetisHistory::new();

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
pub mod subsystems_core;
