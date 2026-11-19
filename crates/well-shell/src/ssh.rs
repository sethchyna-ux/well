//! Deep SSH Engine & Config Discovery
//! Parses OpenSSH configuration (~/.ssh/config), resolves host inheritance,
//! manages remote targets, and orchestrates PTY-backed SSH sessions.

pub use well_ipc::ssh::{ActiveSshSessionInfo, SshHostConfig, SshTarget};

use serde::{Deserialize, Serialize};
pub use shell_words::quote as quote_argument;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Discovered OpenSSH configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SshConfigFile {
    pub hosts: Vec<SshHostConfig>,
    pub global_defaults: SshHostConfig,
}

impl SshConfigFile {
    /// Load standard SSH configuration files from user and system paths.
    pub fn load_default() -> Self {
        let mut config = Self::default();
        if let Some(home) = dirs_home() {
            let user_config = home.join(".ssh").join("config");
            if user_config.exists() {
                if let Ok(content) = std::fs::read_to_string(&user_config) {
                    config.merge(Self::parse(&content));
                }
            }
        }
        let sys_config = Path::new("/etc/ssh/ssh_config");
        if sys_config.exists() {
            if let Ok(content) = std::fs::read_to_string(sys_config) {
                config.merge(Self::parse(&content));
            }
        }
        config
    }

    /// Parse raw OpenSSH config string.
    pub fn parse(content: &str) -> Self {
        let mut hosts = Vec::new();
        let mut global_defaults = SshHostConfig::default();
        let mut current_hosts: Vec<String> = Vec::new();
        let mut current_config = SshHostConfig::default();

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.splitn(2, |c: char| c.is_whitespace() || c == '=');
            let key = match parts.next() {
                Some(k) => k.trim().to_ascii_lowercase(),
                None => continue,
            };
            let val = match parts.next() {
                Some(v) => v.trim().trim_matches('"').to_string(),
                None => continue,
            };

            if key == "host" {
                // Flush previous section
                if !current_hosts.is_empty() {
                    for h in &current_hosts {
                        let mut entry = current_config.clone();
                        entry.name = h.clone();
                        if entry.hostname.is_empty() {
                            entry.hostname = h.clone();
                        }
                        if h == "*" {
                            global_defaults = entry;
                        } else {
                            hosts.push(entry);
                        }
                    }
                }
                current_hosts = val.split_whitespace().map(String::from).collect();
                current_config = SshHostConfig::default();
                continue;
            }

            match key.as_str() {
                "hostname" => current_config.hostname = val,
                "user" => current_config.user = Some(val),
                "port" => {
                    if let Ok(p) = val.parse::<u16>() {
                        current_config.port = p;
                    }
                }
                "identityfile" => {
                    current_config.identity_file = Some(expand_tilde(&val));
                }
                "proxyjump" => current_config.proxy_jump = Some(val),
                "forwardagent" => {
                    current_config.forward_agent = val.eq_ignore_ascii_case("yes");
                }
                _ => {
                    current_config.options.insert(key, val);
                }
            }
        }

        // Flush final section
        if !current_hosts.is_empty() {
            for h in &current_hosts {
                let mut entry = current_config.clone();
                entry.name = h.clone();
                if entry.hostname.is_empty() {
                    entry.hostname = h.clone();
                }
                if h == "*" {
                    global_defaults = entry;
                } else {
                    hosts.push(entry);
                }
            }
        }

        Self {
            hosts,
            global_defaults,
        }
    }

    /// Merge another config, preferring existing host definitions.
    pub fn merge(&mut self, other: SshConfigFile) {
        for other_host in other.hosts {
            if !self.hosts.iter().any(|h| h.name == other_host.name) {
                self.hosts.push(other_host);
            }
        }
    }

    /// Resolve effective config for a given host name or pattern match.
    pub fn resolve(&self, target: &str) -> SshHostConfig {
        let mut resolved = self.global_defaults.clone();
        resolved.name = target.to_string();
        resolved.hostname = target.to_string();

        for host in &self.hosts {
            if wildcard_match(&host.name, target) {
                if !host.hostname.is_empty() && host.hostname != host.name {
                    resolved.hostname = host.hostname.clone();
                }
                if host.user.is_some() {
                    resolved.user = host.user.clone();
                }
                if host.port != 22 {
                    resolved.port = host.port;
                }
                if host.identity_file.is_some() {
                    resolved.identity_file = host.identity_file.clone();
                }
                if host.proxy_jump.is_some() {
                    resolved.proxy_jump = host.proxy_jump.clone();
                }
                if host.forward_agent {
                    resolved.forward_agent = true;
                }
                for (k, v) in &host.options {
                    resolved.options.insert(k.clone(), v.clone());
                }
            }
        }

        resolved
    }
}

/// Parse an input string into an SshTarget resolved against an SshConfigFile.
pub fn parse_target(input: &str, config: &SshConfigFile) -> SshTarget {
    let mut trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("ssh ") {
        trimmed = rest.trim();
    }

    let mut password = None;
    let mut port_override = None;

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let mut target_str = "";
    let mut i = 0;
    while i < parts.len() {
        let part = parts[i];
        if part.eq_ignore_ascii_case("password") || part == "--password" || part == "-pass" {
            if i + 1 < parts.len() {
                password = Some(parts[i + 1].to_string());
                i += 2;
                continue;
            }
        } else if part == "-p" {
            if i + 1 < parts.len() {
                if let Ok(p) = parts[i + 1].parse::<u16>() {
                    port_override = Some(p);
                }
                i += 2;
                continue;
            }
        } else if target_str.is_empty() {
            target_str = part;
        }
        i += 1;
    }

    if target_str.is_empty() {
        target_str = trimmed;
    }

    let (user_part, rest) = if let Some((u, h)) = target_str.split_once('@') {
        (Some(u.to_string()), h)
    } else {
        (None, target_str)
    };

    let (host, inline_port) = if let Some((h, p)) = rest.split_once(':') {
        (h, p.parse::<u16>().ok())
    } else {
        (rest, None)
    };

    let resolved = config.resolve(host);
    SshTarget {
        destination: resolved.hostname,
        user: user_part.or(resolved.user),
        port: port_override.or(inline_port).unwrap_or(resolved.port),
        identity_file: resolved.identity_file,
        proxy_jump: resolved.proxy_jump,
        forward_agent: resolved.forward_agent,
        password,
        extra_args: Vec::new(),
    }
}

/// Metadata for an active SSH session within Well.
#[derive(Clone, Debug)]
pub struct ActiveSshSession {
    pub tab_id: u64,
    pub target: SshTarget,
    pub connected_at: Instant,
}

/// Central SSH Engine tracking discovered hosts and active sessions.
#[derive(Clone, Debug)]
pub struct SshEngine {
    pub config: Arc<Mutex<SshConfigFile>>,
    pub active_sessions: Arc<Mutex<Vec<ActiveSshSession>>>,
}

impl Default for SshEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SshEngine {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(SshConfigFile::load_default())),
            active_sessions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn reload(&self) {
        if let Ok(mut cfg) = self.config.lock() {
            *cfg = SshConfigFile::load_default();
        }
    }

    pub fn list_hosts(&self) -> Vec<SshHostConfig> {
        self.config
            .lock()
            .map(|c| c.hosts.clone())
            .unwrap_or_default()
    }

    pub fn register_session(&self, tab_id: u64, target: SshTarget) {
        if let Ok(mut sessions) = self.active_sessions.lock() {
            sessions.retain(|s| s.tab_id != tab_id);
            sessions.push(ActiveSshSession {
                tab_id,
                target,
                connected_at: Instant::now(),
            });
        }
    }

    pub fn remove_session(&self, tab_id: u64) {
        if let Ok(mut sessions) = self.active_sessions.lock() {
            sessions.retain(|s| s.tab_id != tab_id);
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_sessions.lock().map(|s| s.len()).unwrap_or(0)
    }

    pub fn sessions_snapshot(&self) -> Vec<ActiveSshSessionInfo> {
        self.active_sessions
            .lock()
            .map(|sessions| {
                sessions
                    .iter()
                    .map(|s| ActiveSshSessionInfo {
                        tab_id: s.tab_id,
                        destination: s.target.destination.clone(),
                        user: s.target.user.clone(),
                        port: s.target.port,
                        uptime_secs: s.connected_at.elapsed().as_secs(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn resolve_target(&self, input: &str) -> SshTarget {
        let config = self.config.lock().unwrap_or_else(|p| p.into_inner());
        parse_target(input, &config)
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs_home() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

fn wildcard_match(pattern: &str, target: &str) -> bool {
    if pattern == "*" || pattern == target {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return target.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return target.ends_with(suffix);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SSH_CONFIG: &str = r#"
# Global configuration
Host *
    ForwardAgent yes
    Port 22
    ServerAliveInterval 60

Host dev-box
    HostName 192.168.1.100
    User yocan
    Port 2222
    IdentityFile ~/.ssh/id_ed25519

Host *.internal
    User deploy
    ProxyJump dev-box
"#;

    #[test]
    fn test_parse_ssh_config() {
        let config = SshConfigFile::parse(SAMPLE_SSH_CONFIG);
        assert_eq!(config.hosts.len(), 2);

        let dev_box = config.resolve("dev-box");
        assert_eq!(dev_box.hostname, "192.168.1.100");
        assert_eq!(dev_box.user.as_deref(), Some("yocan"));
        assert_eq!(dev_box.port, 2222);
        assert!(dev_box.forward_agent);
        assert!(dev_box.identity_file.is_some());
    }

    #[test]
    fn test_wildcard_resolution() {
        let config = SshConfigFile::parse(SAMPLE_SSH_CONFIG);
        let resolved = config.resolve("api.internal");
        assert_eq!(resolved.hostname, "api.internal");
        assert_eq!(resolved.user.as_deref(), Some("deploy"));
        assert_eq!(resolved.proxy_jump.as_deref(), Some("dev-box"));
        assert!(resolved.forward_agent);
    }

    #[test]
    fn test_ssh_target_args() {
        let config = SshConfigFile::parse(SAMPLE_SSH_CONFIG);
        let target = parse_target("dev-box", &config);
        let args = target.to_command_args();
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "2222");
        assert_eq!(args[2], "-i");
        assert_eq!(args[4], "-A");
        assert_eq!(args.last().unwrap(), "yocan@192.168.1.100");
    }

    #[test]
    fn test_parse_target_ssh_prefix_and_password() {
        let config = SshConfigFile::default();
        let target = parse_target("ssh chyna@192.168.1.12 password yocan", &config);
        assert_eq!(target.destination, "192.168.1.12");
        assert_eq!(target.user.as_deref(), Some("chyna"));
        assert_eq!(target.port, 22);
        assert_eq!(target.password.as_deref(), Some("yocan"));
    }
}
