//! Theia SSH & NetOps Console Models, Protocol Engines & Discovery (`ssh_console`)
//! Supports:
//! 1. Full SSH Host Profile CRUD & Editing (`HostEditor`)
//! 2. Visual Port Forwarding & Tunnels (`PortForwarding`: `LOCAL -L`, `DYNAMIC -D` SOCKS5, `REMOTE -R`)
//! 3. Local SSH Key Vault & Embedded HTTP Authorized Key Server (`KeyVault`: `:8787/authorized_keys`)
//! 4. SSH Server Hosting, `$SSH_AUTH_SOCK` Agent & NetOps Autonomous Agent (`ServerAndAgentHub`)
//! 5. AgentOS x86_64 EFI build/export/QEMU and the real PXE netboot companion (`PxeAndBootloader`)
//! 6. SMB / CIFS File Shares & Netboot Payload Mounts (`SmbShares`)
//! 7. Networking Tools: Fast TCP Port Scanner, Live ARP Table, Wake-on-LAN (WoL) UDP Magic Packet, Ping/Packet Sniffer (`NetworkTools`)

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use well_shell::ssh::SshHostConfig;
use well_shell::theia_ssh::TheiaSshProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SshConsoleSection {
    #[default]
    HostEditor,
    PortForwarding,
    KeyVault,
    ServerAndAgentHub,
    PxeAndBootloader,
    SmbShares,
    NetworkTools,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ServerHubSubtab {
    #[default]
    ServerHosting,
    AgentHub,
    OpsAgent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SshConsoleInputField {
    #[default]
    SidebarSearch,
    HostName,
    HostHostname,
    HostUser,
    HostPort,
    HostIdentityFile,
    HostProxyJump,
    TunnelLocalListen,
    TunnelRemoteTarget,
    TunnelSshHost,
    ServerPort,
    ServerListenAddr,
    AgentKeyPath,
    KeyServerPort,
    PxeServerIp,
    PxeHttpPort,
    PxeBootFile,
    BootSource,
    BootLeaseRange,
    SmbServerHost,
    SmbShareName,
    SmbUsername,
    NetTargetHost,
    NetTargetMac,
}

/// Editable draft for creating or modifying an SSH Host profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshHostEditorDraft {
    pub editing_index: Option<usize>,
    pub name: String,
    pub hostname: String,
    pub user: String,
    pub port: String,
    pub identity_file: String,
    pub proxy_jump: String,
    pub forward_agent: bool,
    pub use_control_master: bool,
    pub forward_caduceus_ipc: bool,
}

impl Default for SshHostEditorDraft {
    fn default() -> Self {
        Self {
            editing_index: None,
            name: String::new(),
            hostname: String::new(),
            user: String::new(),
            port: "22".into(),
            identity_file: "~/.ssh/id_ed25519".into(),
            proxy_jump: String::new(),
            forward_agent: true,
            use_control_master: true,
            forward_caduceus_ipc: false,
        }
    }
}

impl SshHostEditorDraft {
    pub fn new_blank() -> Self {
        Self {
            editing_index: None,
            name: "new-server".into(),
            hostname: String::new(),
            user: std::env::var("USER").unwrap_or_else(|_| "ops".into()),
            port: "22".into(),
            identity_file: "~/.ssh/id_ed25519".into(),
            proxy_jump: String::new(),
            forward_agent: true,
            use_control_master: true,
            forward_caduceus_ipc: false,
        }
    }

    pub fn from_profile(editing_index: Option<usize>, prof: &TheiaSshProfile) -> Self {
        Self {
            editing_index,
            name: prof.host.name.clone(),
            hostname: prof.host.hostname.clone(),
            user: prof.host.user.clone().unwrap_or_else(|| "ops".into()),
            port: prof.host.port.to_string(),
            identity_file: prof
                .host
                .identity_file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.ssh/id_ed25519".into()),
            proxy_jump: prof.host.proxy_jump.clone().unwrap_or_default(),
            forward_agent: prof.host.forward_agent,
            use_control_master: prof.use_control_master,
            forward_caduceus_ipc: prof.forward_caduceus_ipc,
        }
    }

    pub fn to_profile(&self) -> Result<TheiaSshProfile, String> {
        let name = self.name.trim();
        let hostname = self.hostname.trim();
        if name.is_empty() {
            return Err("Host alias/name cannot be empty".into());
        }
        let resolved_hostname = if hostname.is_empty() {
            name.to_string()
        } else {
            hostname.to_string()
        };
        let port = self
            .port
            .trim()
            .parse::<u16>()
            .map_err(|_| "SSH port must be between 1 and 65535")?;
        if port == 0 {
            return Err("SSH port must be between 1 and 65535".into());
        }
        let user = {
            let u = self.user.trim();
            if u.is_empty() {
                None
            } else {
                Some(u.to_string())
            }
        };
        let identity_file = {
            let id = self.identity_file.trim();
            if id.is_empty() {
                None
            } else {
                Some(PathBuf::from(id))
            }
        };
        let proxy_jump = {
            let pj = self.proxy_jump.trim();
            if pj.is_empty() {
                None
            } else {
                Some(pj.to_string())
            }
        };

        let host_cfg = SshHostConfig {
            name: name.to_string(),
            hostname: resolved_hostname,
            user,
            port,
            identity_file,
            proxy_jump,
            forward_agent: self.forward_agent,
            options: Default::default(),
        };
        let mut prof = TheiaSshProfile::from_host(host_cfg);
        prof.use_control_master = self.use_control_master;
        prof.forward_caduceus_ipc = self.forward_caduceus_ipc;
        Ok(prof)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshTunnelKind {
    Local,
    Dynamic,
    Remote,
}

impl SshTunnelKind {
    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::Local => "LOCAL",
            Self::Dynamic => "DYNAMIC",
            Self::Remote => "REMOTE",
        }
    }

    pub const fn ssh_flag(self) -> &'static str {
        match self {
            Self::Local => "-L",
            Self::Dynamic => "-D",
            Self::Remote => "-R",
        }
    }

    pub const fn badge_colors(self) -> (u32, u32) {
        match self {
            Self::Local => (0x0d2833, 0x2ac3de),
            Self::Dynamic => (0x2e2412, 0xe0af68),
            Self::Remote => (0x251b38, 0xbb9af7),
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Local => Self::Dynamic,
            Self::Dynamic => Self::Remote,
            Self::Remote => Self::Local,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTunnelEntry {
    pub kind: SshTunnelKind,
    pub title: String,
    pub local_listen: String,
    pub remote_target: String,
    pub active: bool,
    pub traffic_bytes: u64,
}

impl SshTunnelEntry {
    #[cfg(test)]
    pub fn default_catalog() -> Vec<Self> {
        vec![
            Self {
                kind: SshTunnelKind::Local,
                title: "Port 5433 (Postgres)".into(),
                local_listen: "127.0.0.1:5433".into(),
                remote_target: "127.0.0.1:5432".into(),
                active: false,
                traffic_bytes: 0,
            },
            Self {
                kind: SshTunnelKind::Local,
                title: "Port 6380 (Redis)".into(),
                local_listen: "127.0.0.1:6380".into(),
                remote_target: "127.0.0.1:6379".into(),
                active: false,
                traffic_bytes: 0,
            },
            Self {
                kind: SshTunnelKind::Dynamic,
                title: "Port 1080 (SOCKS5)".into(),
                local_listen: "127.0.0.1:1080".into(),
                remote_target: "SOCKS5 Proxy".into(),
                active: false,
                traffic_bytes: 0,
            },
            Self {
                kind: SshTunnelKind::Dynamic,
                title: "Port 8080 (SOCKS5)".into(),
                local_listen: "127.0.0.1:8080".into(),
                remote_target: "SOCKS5 Proxy".into(),
                active: false,
                traffic_bytes: 0,
            },
            Self {
                kind: SshTunnelKind::Remote,
                title: "Port 8080 (Ingress)".into(),
                local_listen: "127.0.0.1:8080".into(),
                remote_target: "127.0.0.1:80".into(),
                active: false,
                traffic_bytes: 0,
            },
        ]
    }

    /// Build the exact OpenSSH CLI invocation for this tunnel.
    pub fn build_ssh_command(&self, ssh_destination: &str) -> String {
        let dest = ssh_destination.trim();
        if dest.is_empty() {
            return String::new();
        }
        let forward = if self.kind == SshTunnelKind::Dynamic {
            self.local_listen.clone()
        } else {
            format!("{}:{}", self.local_listen, self.remote_target)
        };
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        format!(
            "ssh -o ExitOnForwardFailure=yes -N {} {} -- {}",
            self.kind.ssh_flag(),
            quote(&forward),
            quote(dest)
        )
    }

    /// Check if the tunnel's local port is currently accepting TCP connections.
    pub fn probe_local_port(&self) -> bool {
        if let Ok(addr) = self.local_listen.parse::<SocketAddr>() {
            TcpStream::connect_timeout(&addr, Duration::from_millis(35)).is_ok()
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredSshKey {
    pub name: String,
    pub algorithm: String,
    pub path_display: String,
    pub fingerprint: String,
    pub public_key_line: String,
    pub is_passkey: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownHostEntry {
    pub line_number: usize,
    pub host_pattern: String,
    pub key_type: String,
    pub fingerprint: String,
    pub raw_line: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalNetworkInterface {
    pub name: String,
    pub ip: String,
    pub netmask: String,
    pub mac: String,
    pub is_up: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListeningSocket {
    pub port: u16,
    pub protocol: String,
    pub process_name: String,
    pub pid: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PingProbeResult {
    pub host: String,
    pub transmitted: usize,
    pub received: usize,
    pub loss_percent: f32,
    pub min_ms: f32,
    pub avg_ms: f32,
    pub max_ms: f32,
}

impl DiscoveredSshKey {
    pub fn discover_local_keys() -> Vec<Self> {
        let mut keys = Vec::new();
        if let Ok(home) = std::env::var("HOME") {
            let ssh_dir = PathBuf::from(&home).join(".ssh");
            if let Ok(entries) = std::fs::read_dir(&ssh_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("pub") {
                        if let Ok(raw) = std::fs::read_to_string(&path) {
                            let line = raw.lines().next().unwrap_or("").trim().to_string();
                            if line.is_empty() {
                                continue;
                            }
                            let stem = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("id_key")
                                .to_string();
                            let priv_path = ssh_dir.join(&stem);
                            let is_pk = line.starts_with("sk-") || stem.contains("_sk");
                            let algo = if line.starts_with("sk-ssh-ed25519") {
                                "FIDO2 Ed25519 Passkey".to_string()
                            } else if line.starts_with("sk-ecdsa") {
                                "FIDO2 ECDSA Passkey".to_string()
                            } else if line.starts_with("ssh-ed25519") {
                                "Ed25519".to_string()
                            } else if line.starts_with("ssh-rsa") {
                                "RSA".to_string()
                            } else if line.starts_with("ecdsa") {
                                "ECDSA".to_string()
                            } else if is_pk {
                                "FIDO2 Passkey".to_string()
                            } else {
                                "SSH".to_string()
                            };
                            let fp = Self::fingerprint(&line);
                            keys.push(Self {
                                name: stem,
                                algorithm: algo,
                                path_display: priv_path.display().to_string(),
                                fingerprint: fp,
                                public_key_line: line,
                                is_passkey: is_pk,
                            });
                        }
                    }
                }
            }
        }
        keys.sort_by(|a, b| a.name.cmp(&b.name));
        keys
    }

    /// Query `ssh-add -l` to list currently loaded SSH agent identities in `$SSH_AUTH_SOCK`.
    pub fn query_loaded_agent_identities() -> Vec<String> {
        let output = std::process::Command::new("ssh-add").arg("-l").output();
        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && !s.contains("The agent has no identities"))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn fingerprint(pub_line: &str) -> String {
        well_ssh::public_key_fingerprint(pub_line).unwrap_or_else(|_| "Invalid public key".into())
    }
    pub fn authorized() -> Vec<Self> {
        let Some(home) = std::env::var_os("HOME") else {
            return Vec::new();
        };
        std::fs::read_to_string(PathBuf::from(home).join(".ssh/authorized_keys"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let fingerprint = well_ssh::public_key_fingerprint(line).ok()?;
                Some(Self {
                    name: line
                        .split_whitespace()
                        .nth(2)
                        .unwrap_or("Authorized client")
                        .into(),
                    algorithm: line.split_whitespace().next().unwrap_or("SSH").into(),
                    path_display: "~/.ssh/authorized_keys".into(),
                    fingerprint,
                    public_key_line: line.into(),
                    is_passkey: line.starts_with("sk-"),
                })
            })
            .collect()
    }
}

/// Discover and parse `~/.ssh/known_hosts` into structured security entries
pub fn discover_known_hosts() -> Vec<KnownHostEntry> {
    let mut entries = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let kh_path = PathBuf::from(home).join(".ssh").join("known_hosts");
        if let Ok(content) = std::fs::read_to_string(&kh_path) {
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let mut parts = trimmed.split_whitespace();
                let first = parts.next().unwrap_or("?");
                let host = if first.starts_with('@') {
                    parts.next().unwrap_or("?")
                } else {
                    first
                }
                .to_string();
                let key_type = parts.next().unwrap_or("unknown").to_string();
                let key_data = parts.next().unwrap_or("");
                let fp = DiscoveredSshKey::fingerprint(&format!("{key_type} {key_data}"));
                entries.push(KnownHostEntry {
                    line_number: idx + 1,
                    host_pattern: host,
                    key_type,
                    fingerprint: fp,
                    raw_line: trimmed.to_string(),
                });
            }
        }
    }
    entries
}

/// Query local network interfaces via macOS `ifconfig`
pub fn discover_network_interfaces() -> Vec<LocalNetworkInterface> {
    let mut ifaces = Vec::new();
    if let Ok(out) = std::process::Command::new("ifconfig").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut cur_name = String::new();
        let mut cur_ip = String::new();
        let mut cur_mask = String::new();
        let mut cur_mac = String::new();
        let mut is_up = false;

        for line in text.lines() {
            if !line.starts_with('\t') && !line.starts_with(' ') {
                if !cur_name.is_empty() && (!cur_ip.is_empty() || !cur_mac.is_empty()) {
                    ifaces.push(LocalNetworkInterface {
                        name: cur_name,
                        ip: if cur_ip.is_empty() {
                            "Unassigned".into()
                        } else {
                            cur_ip
                        },
                        netmask: if cur_mask.is_empty() {
                            "255.255.255.0".into()
                        } else {
                            cur_mask
                        },
                        mac: if cur_mac.is_empty() {
                            "--".into()
                        } else {
                            cur_mac
                        },
                        is_up,
                    });
                }
                cur_name = line.split(':').next().unwrap_or("").trim().to_string();
                cur_ip = String::new();
                cur_mask = String::new();
                cur_mac = String::new();
                is_up = line.contains("UP") && line.contains("RUNNING");
            } else {
                let trimmed = line.trim();
                if trimmed.starts_with("inet ") {
                    let mut parts = trimmed.split_whitespace();
                    let _ = parts.next(); // "inet"
                    if let Some(ip) = parts.next() {
                        cur_ip = ip.to_string();
                    }
                    if let Some(m) = parts.nth(1) {
                        cur_mask = m.to_string();
                    }
                } else if trimmed.starts_with("ether ") {
                    if let Some(mac) = trimmed.split_whitespace().nth(1) {
                        cur_mac = mac.to_string();
                    }
                }
            }
        }
        if !cur_name.is_empty() && (!cur_ip.is_empty() || !cur_mac.is_empty()) {
            ifaces.push(LocalNetworkInterface {
                name: cur_name,
                ip: if cur_ip.is_empty() {
                    "Unassigned".into()
                } else {
                    cur_ip
                },
                netmask: if cur_mask.is_empty() {
                    "255.255.255.0".into()
                } else {
                    cur_mask
                },
                mac: if cur_mac.is_empty() {
                    "--".into()
                } else {
                    cur_mac
                },
                is_up,
            });
        }
    }
    ifaces
}

/// Query local listening TCP ports via macOS `lsof`
pub fn discover_listening_sockets() -> Vec<ListeningSocket> {
    let mut sockets = Vec::new();
    if let Ok(out) = std::process::Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 9 {
                let cmd = cols[0].to_string();
                let pid = cols[1].to_string();
                let node = cols[8];
                // node is e.g. "*:22", "127.0.0.1:8787", "[::]:5000"
                if let Some(port_str) = node.split(':').next_back() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        if !sockets.iter().any(|s: &ListeningSocket| s.port == port) {
                            sockets.push(ListeningSocket {
                                port,
                                protocol: "TCP".into(),
                                process_name: cmd,
                                pid,
                            });
                        }
                    }
                }
            }
        }
    }
    sockets.sort_by_key(|s| s.port);
    sockets
}

/// Multi-probe ICMP / TCP ping to compute latency & packet loss
pub fn run_ping_probe(target_host: &str) -> Option<PingProbeResult> {
    let host = target_host.trim();
    if host.is_empty() {
        return None;
    }
    let output = std::process::Command::new("ping")
        .args(["-c", "3", "-t", "2", host])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut min_ms = 0.0f32;
    let mut avg_ms = 0.0f32;
    let mut max_ms = 0.0f32;
    let mut loss_pct = 100.0f32;

    for line in text.lines() {
        if line.contains("packet loss") {
            if let Some(pct_str) = line
                .split("% packet loss")
                .next()
                .and_then(|s| s.split(',').next_back())
            {
                loss_pct = pct_str.trim().parse::<f32>().unwrap_or(0.0);
            }
        }
        if line.contains("round-trip") || line.contains("rtt") {
            if let Some(stats) = line.split('=').nth(1) {
                let parts: Vec<&str> = stats.trim().split('/').collect();
                if parts.len() >= 3 {
                    min_ms = parts[0].trim().parse().unwrap_or(0.0);
                    avg_ms = parts[1].trim().parse().unwrap_or(0.0);
                    max_ms = parts[2].trim().parse().unwrap_or(0.0);
                }
            }
        }
    }

    Some(PingProbeResult {
        host: host.to_string(),
        transmitted: 3,
        received: ((3.0 * (1.0 - loss_pct / 100.0)).round().clamp(0.0, 3.0)) as usize,
        loss_percent: loss_pct,
        min_ms,
        avg_ms,
        max_ms,
    })
}

/// Resolve DNS records for target hostname
pub fn resolve_dns_query(host: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(addrs) = format!("{}:80", host.trim()).to_socket_addrs() {
        for a in addrs {
            results.push(a.ip().to_string());
        }
    }
    results.dedup();
    results
}

/// Spawn a lightweight background HTTP Key Server on `0.0.0.0:<port>` serving `/authorized_keys`.
pub fn start_http_key_server(port: u16, stop_flag: Arc<AtomicBool>) -> Result<SocketAddr, String> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .map_err(|e| format!("Failed to bind HTTP Key Server on port {port}: {e}"))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to read local_addr: {e}"))?;
    let _ = listener.set_nonblocking(true);

    std::thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _peer)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                    let mut req_buf = [0u8; 512];
                    let _ = stream.read(&mut req_buf);
                    let keys = DiscoveredSshKey::discover_local_keys();
                    let mut body = String::new();
                    for k in keys {
                        body.push_str(&k.public_key_line);
                        body.push('\n');
                    }
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(60));
                }
                Err(_) => break,
            }
        }
    });

    Ok(local_addr)
}

/// Network deployment mode; packet engines live in the pinned PXE library used by well-boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PxeServerMode {
    ProxyDhcp,
    StandaloneDhcp,
}

// ============================================================================
// SMB / CIFS Network Shares & Netboot Payload Mounts
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmbShareEntry {
    pub title: String,
    pub server_host: String,
    pub share_name: String,
    pub username: String,
    pub mount_point: String,
    pub purpose_badge: String,
}

impl SmbShareEntry {
    pub fn smb_url(&self) -> String {
        if self.username.trim().is_empty() {
            format!(
                "smb://{}/{}",
                self.server_host.trim(),
                self.share_name.trim()
            )
        } else {
            format!(
                "smb://{}@{}/{}",
                self.username.trim(),
                self.server_host.trim(),
                self.share_name.trim()
            )
        }
    }

    pub fn mount_smbfs_command(&self) -> String {
        let user = if self.username.trim().is_empty() {
            "guest"
        } else {
            self.username.trim()
        };
        let url = format!(
            "//{user}@{}/{}",
            self.server_host.trim(),
            self.share_name.trim()
        );
        let quote = well_shell::ssh::quote_argument;
        format!(
            "mkdir -p {} && mount_smbfs {} {}",
            quote(&self.mount_point),
            quote(&url),
            quote(&self.mount_point)
        )
    }

    pub fn is_mounted(&self) -> bool {
        std::process::Command::new("mount")
            .output()
            .is_ok_and(|out| {
                out.status.success()
                    && String::from_utf8_lossy(&out.stdout).lines().any(|line| {
                        line.split_once(" on ").is_some_and(|(_, rest)| {
                            rest.split_once(" (").is_some_and(|(path, kind)| {
                                path == self.mount_point
                                    && (kind.contains("smbfs") || kind.contains("cifs"))
                            })
                        })
                    })
            })
    }
}

// ============================================================================
// Networking Tools: TCP Port Scanner, ARP Table, Wake-on-LAN (WoL) & Ping
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortScanResult {
    pub port: u16,
    pub service: &'static str,
    pub open: bool,
    pub latency_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpNeighborEntry {
    pub hostname: String,
    pub ip: String,
    pub mac: String,
    pub interface: String,
}

/// Detect the primary LAN IPv4 address of this macOS host by connecting a UDP socket.
pub fn detect_primary_lan_ip() -> Ipv4Addr {
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(SocketAddr::V4(v4)) = sock.local_addr() {
                return *v4.ip();
            }
        }
    }
    Ipv4Addr::UNSPECIFIED
}

/// Scan key infrastructure & PXE/SSH/SMB ports on `target_host` with bounded timeouts.
pub fn run_fast_port_scan(target_host: &str) -> Vec<PortScanResult> {
    const PORTS: &[(u16, &str)] = &[
        (22, "SSH"),
        (53, "DNS"),
        (67, "DHCP"),
        (69, "TFTP"),
        (80, "HTTP"),
        (139, "NetBIOS"),
        (443, "HTTPS"),
        (445, "SMB/CIFS"),
        (1080, "SOCKS5"),
        (2222, "Theia-SSH"),
        (4011, "ProxyDHCP"),
        (5432, "Postgres"),
        (6379, "Redis"),
        (8080, "iPXE-HTTP"),
        (8787, "Key-Srv"),
    ];
    let host = if target_host.trim().is_empty() {
        "127.0.0.1"
    } else {
        target_host.trim()
    };

    let resolved_ip: Option<IpAddr> = (host, 80u16)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
        .map(|sa| sa.ip());

    let Some(ip) = resolved_ip else {
        return PORTS
            .iter()
            .map(|(port, service)| PortScanResult {
                port: *port,
                service,
                open: false,
                latency_ms: 0,
            })
            .collect();
    };

    std::thread::scope(|s| {
        let handles: Vec<_> = PORTS
            .iter()
            .map(|(port, service)| {
                let port = *port;
                let service = *service;
                s.spawn(move || {
                    let sa = SocketAddr::new(ip, port);
                    let start = Instant::now();
                    let open = TcpStream::connect_timeout(&sa, Duration::from_millis(65)).is_ok();
                    let latency_ms = start.elapsed().as_millis() as u64;
                    PortScanResult {
                        port,
                        service,
                        open,
                        latency_ms,
                    }
                })
            })
            .collect();

        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// Query and parse the live macOS `arp -a` neighbor table.
pub fn discover_arp_neighbors() -> Vec<ArpNeighborEntry> {
    let mut entries = Vec::new();
    if let Ok(out) = std::process::Command::new("arp").arg("-a").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            // Example macOS `arp -a` line:
            // cachey.local (192.168.1.12) at aa:bb:cc:dd:ee:ff on en0 ifscope [ethernet]
            let host = line.split_whitespace().next().unwrap_or("?").to_string();
            let ip = line
                .split('(')
                .nth(1)
                .and_then(|s| s.split(')').next())
                .unwrap_or("")
                .to_string();
            let mac = line
                .split(" at ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("")
                .to_string();
            let iface = line
                .split(" on ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("en0")
                .to_string();
            if !ip.is_empty() && !mac.is_empty() && mac != "(incomplete)" {
                entries.push(ArpNeighborEntry {
                    hostname: host,
                    ip,
                    mac,
                    interface: iface,
                });
            }
        }
    }
    entries
}

/// Construct a standard 102-byte Wake-on-LAN (WoL) magic packet (`6x 0xFF` + `16x 6-byte MAC`).
pub fn build_wol_magic_packet(mac_str: &str) -> Result<[u8; 102], String> {
    let parts: Vec<&str> = mac_str
        .trim()
        .split([':', '-'])
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 6 {
        return Err(format!(
            "Invalid MAC address '{mac_str}': expected 6 hex octets (e.g., aa:bb:cc:dd:ee:ff)"
        ));
    }
    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16)
            .map_err(|_| format!("Invalid hex byte '{part}' in MAC address"))?;
    }
    let mut pkt = [0xFFu8; 102];
    for rep in 0..16 {
        let offset = 6 + rep * 6;
        pkt[offset..offset + 6].copy_from_slice(&mac);
    }
    Ok(pkt)
}

/// Broadcast a Wake-on-LAN magic packet over UDP to `255.255.255.255:9`.
pub fn send_wol_magic_packet(mac_str: &str) -> Result<usize, String> {
    let pkt = build_wol_magic_packet(mac_str)?;
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP bind failed: {e}"))?;
    sock.set_broadcast(true)
        .map_err(|e| format!("UDP set_broadcast failed: {e}"))?;
    sock.send_to(&pkt, "255.255.255.255:9")
        .map_err(|e| format!("UDP broadcast send failed: {e}"))
}

pub fn authorize_public_key(line: &str) -> Result<(), String> {
    let line = line.trim();
    if line.contains(['\n', '\r']) {
        return Err("Paste one public key at a time".into());
    }
    well_ssh::public_key_fingerprint(line)?;
    let home = std::env::var_os("HOME").ok_or("HOME unavailable")?;
    let directory = PathBuf::from(home).join(".ssh");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join("authorized_keys");
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Refusing to edit a symlinked authorized_keys file".into());
    }
    if std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .any(|old| old.trim() == line)
    {
        return Ok(());
    }
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    writeln!(file, "\n{line}").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_editor_draft_roundtrip() {
        let mut draft = SshHostEditorDraft::new_blank();
        draft.name = "chyna".into();
        draft.hostname = "cachey.local".into();
        draft.user = "chyna".into();
        draft.port = "2222".into();
        draft.identity_file = "~/.ssh/id_ed25519".into();
        draft.proxy_jump = "bastion.local".into();
        draft.forward_agent = true;
        draft.use_control_master = true;
        draft.forward_caduceus_ipc = true;

        let prof = draft.to_profile().expect("valid profile");
        assert_eq!(prof.host.name, "chyna");
        assert_eq!(prof.host.hostname, "cachey.local");
        assert_eq!(prof.host.user.as_deref(), Some("chyna"));
        assert_eq!(prof.host.port, 2222);
        assert_eq!(prof.host.proxy_jump.as_deref(), Some("bastion.local"));
        assert!(prof.host.forward_agent);
        assert!(prof.forward_caduceus_ipc);

        let restored = SshHostEditorDraft::from_profile(Some(0), &prof);
        assert_eq!(restored.editing_index, Some(0));
        assert_eq!(restored.name, "chyna");
        assert_eq!(restored.port, "2222");
    }

    #[test]
    fn test_default_tunnels_catalog_and_cli_builder() {
        let tunnels = SshTunnelEntry::default_catalog();
        assert_eq!(tunnels.len(), 5);
        assert_eq!(tunnels[0].kind, SshTunnelKind::Local);
        assert_eq!(tunnels[2].kind, SshTunnelKind::Dynamic);
        assert_eq!(tunnels[4].kind, SshTunnelKind::Remote);

        let cmd = tunnels[0].build_ssh_command("chyna@192.168.1.12");
        assert!(cmd.contains("-L '127.0.0.1:5433:127.0.0.1:5432'"));
        assert!(cmd.contains("chyna@192.168.1.12"));
    }

    #[test]
    fn test_wol_magic_packet_builder() {
        let pkt = build_wol_magic_packet("a8:a1:59:2c:41:0e").expect("valid mac");
        assert_eq!(pkt.len(), 102);
        assert_eq!(&pkt[0..6], &[0xFF; 6]);
        assert_eq!(&pkt[6..12], &[0xa8, 0xa1, 0x59, 0x2c, 0x41, 0x0e]);
        assert_eq!(&pkt[96..102], &[0xa8, 0xa1, 0x59, 0x2c, 0x41, 0x0e]);
    }
}
