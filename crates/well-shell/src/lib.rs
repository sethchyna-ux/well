//! well-shell: Metis Shell Logic Core & Logarithmic Suggestion Engine
//!
//! Subsystems:
//! - MetisHistory: High-performance prefix-trie indexing command history with O(k) queries.
//! - MetisExecutor: In-process shell parser and execution loop.

pub mod pty;
pub use pty::PtySession;

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Default, Debug)]
pub struct MetisTrieNode {
    pub children: HashMap<char, MetisTrieNode>,
    pub is_terminal: bool,
    pub frequency: u64,
}

/// MetisHistory: In-memory prefix-trie search engine.
/// Provides O(k) predictive suggestions on raw keystroke streams inside the shell thread.
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
}

/// Execution outcome returned by MetisExecutor
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// MetisExecutor: Command tokenizer and executor
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
        // Seed standard initial commands
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
            _ => {
                // Execute child process
                match Command::new(cmd)
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
                }
            }
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
}
