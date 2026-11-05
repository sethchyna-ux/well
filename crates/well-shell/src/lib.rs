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
pub use history::{MetisHistory, MetisRecipe, MetisTrieNode};
pub mod graphics;
pub mod hypershell;
pub mod keyboard;
pub mod pty;
pub mod service;
pub mod ssh;
pub mod theia_ssh;
pub use service::{init_shell, ShellHandle};
pub use ssh::{parse_target, ActiveSshSession, SshConfigFile, SshEngine, SshHostConfig, SshTarget};
pub use theia_ssh::{SshHostTier, TheiaSshProfile};

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
        let mut history = MetisHistory::new();
        history.load_all_shell_histories();

        Self {
            history,
            current_dir,
        }
    }

    pub fn with_history(current_dir: PathBuf, history: MetisHistory) -> Self {
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

        let parts = match shell_words::split(trimmed) {
            Ok(parts) if !parts.is_empty() => parts,
            Ok(_) => {
                return ShellOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            Err(e) => {
                return ShellOutput {
                    stdout: String::new(),
                    stderr: format!("shell syntax error: {e}\n"),
                    exit_code: 2,
                }
            }
        };
        let cmd = parts[0].as_str();
        let args = &parts[1..];

        match cmd {
            "cd" => {
                let target = if args.is_empty() {
                    dirs_fallback()
                } else {
                    PathBuf::from(&args[0])
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
            "clear" => ShellOutput {
                stdout: "\x1b[2J\x1b[H".to_string(),
                stderr: String::new(),
                exit_code: 0,
            },
            "history" => {
                let limit = args
                    .first()
                    .and_then(|a| a.parse::<usize>().ok())
                    .unwrap_or(20);
                let recent = self.history.recent_commands(limit);
                let mut out = String::new();
                for (idx, cmd) in recent.iter().enumerate() {
                    out.push_str(&format!("{:5}  {}\n", idx + 1, cmd));
                }
                ShellOutput {
                    stdout: out,
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            "export" => {
                for arg in args {
                    if let Some((k, v)) = arg.split_once('=') {
                        env::set_var(k, v);
                    }
                }
                ShellOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            "env" => {
                let mut out = String::new();
                for (k, v) in env::vars() {
                    out.push_str(&format!("{k}={v}\n"));
                }
                ShellOutput {
                    stdout: out,
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            "which" | "type" => {
                if args.is_empty() {
                    return ShellOutput {
                        stdout: String::new(),
                        stderr: "which: missing argument\n".to_string(),
                        exit_code: 1,
                    };
                }
                let target = &args[0];
                match target.as_str() {
                    "cd" | "pwd" | "echo" | "clear" | "history" | "export" | "env" | "which"
                    | "type" => ShellOutput {
                        stdout: format!("{target}: metis in-process shell builtin\n"),
                        stderr: String::new(),
                        exit_code: 0,
                    },
                    _ => {
                        let path_var = env::var("PATH").unwrap_or_default();
                        let found = env::split_paths(&path_var).find_map(|dir| {
                            let candidate = dir.join(target);
                            if candidate.is_file() {
                                Some(candidate)
                            } else {
                                None
                            }
                        });
                        match found {
                            Some(path) => ShellOutput {
                                stdout: format!("{}\n", path.display()),
                                stderr: String::new(),
                                exit_code: 0,
                            },
                            None => ShellOutput {
                                stdout: String::new(),
                                stderr: format!("{target} not found\n"),
                                exit_code: 1,
                            },
                        }
                    }
                }
            }
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

        let quote_res = exec.execute("echo 'hello   world' \"nested arg\"");
        assert_eq!(quote_res.exit_code, 0);
        assert_eq!(quote_res.stdout.trim(), "hello   world nested arg");

        let which_res = exec.execute("which cd");
        assert_eq!(which_res.exit_code, 0);
        assert!(which_res.stdout.contains("builtin"));

        let clear_res = exec.execute("clear");
        assert_eq!(clear_res.exit_code, 0);
        assert!(clear_res.stdout.contains("\x1b[2J"));

        let export_res = exec.execute("export WELL_TEST_VAR=sovereign");
        assert_eq!(export_res.exit_code, 0);
        assert_eq!(
            std::env::var("WELL_TEST_VAR").ok(),
            Some("sovereign".to_string())
        );

        let hist_res = exec.execute("history 10");
        assert_eq!(hist_res.exit_code, 0);
        assert!(hist_res.stdout.contains("export WELL_TEST_VAR=sovereign"));
    }
}
pub mod subsystems_core;
