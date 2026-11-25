//! SSH Data Models & RPC Payloads for Well
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Individual SSH Host entry parsed from OpenSSH config.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHostConfig {
    pub name: String,
    pub hostname: String,
    pub user: Option<String>,
    pub port: u16,
    pub identity_file: Option<PathBuf>,
    pub proxy_jump: Option<String>,
    pub forward_agent: bool,
    pub options: HashMap<String, String>,
}

impl Default for SshHostConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            hostname: String::new(),
            user: None,
            port: 22,
            identity_file: None,
            proxy_jump: None,
            forward_agent: false,
            options: HashMap::new(),
        }
    }
}

/// Represents an SSH target connection specification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTarget {
    pub destination: String,
    pub user: Option<String>,
    pub port: u16,
    pub identity_file: Option<PathBuf>,
    pub proxy_jump: Option<String>,
    pub forward_agent: bool,
    pub password: Option<String>,
    pub extra_args: Vec<String>,
}

impl Default for SshTarget {
    fn default() -> Self {
        Self {
            destination: String::new(),
            user: None,
            port: 22,
            identity_file: None,
            proxy_jump: None,
            forward_agent: false,
            password: None,
            extra_args: Vec::new(),
        }
    }
}

impl SshTarget {
    /// Formats the command line arguments for the `ssh` binary.
    pub fn to_command_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.port != 22 {
            args.push("-p".to_string());
            args.push(self.port.to_string());
        }
        if let Some(key) = &self.identity_file {
            let expanded = expand_tilde_path(key);
            args.push("-i".to_string());
            args.push(expanded.display().to_string());
        }
        if let Some(jump) = &self.proxy_jump {
            args.push("-J".to_string());
            args.push(jump.clone());
        }
        if self.forward_agent {
            args.push("-A".to_string());
        }
        args.extend(self.extra_args.clone());

        let target_str = match &self.user {
            Some(u) => format!("{u}@{}", self.destination),
            None => self.destination.clone(),
        };
        args.push(target_str);
        args
    }
}

/// Expand a leading `~/` against `$HOME` so `/usr/bin/ssh` receives a real path
/// even when spawned directly without a shell.
fn expand_tilde_path(path: &std::path::Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            return home.join(rest);
        }
    } else if raw == "~" {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            return home;
        }
    }
    path.to_path_buf()
}

/// Live SSH session telemetry information.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSshSessionInfo {
    pub tab_id: u64,
    pub destination: String,
    pub user: Option<String>,
    pub port: u16,
    pub uptime_secs: u64,
}
