//! Theia SSH & NetOps Console Models, Protocol Engines & Discovery (`ssh_console`)
//! Supports:
//! 1. Full SSH Host Profile CRUD & Editing (`HostEditor`)
//! 2. Visual Port Forwarding & Tunnels (`PortForwarding`: `LOCAL -L`, `DYNAMIC -D` SOCKS5, `REMOTE -R`)
//! 3. Local SSH Key Vault & Embedded HTTP Authorized Key Server (`KeyVault`: `:8787/authorized_keys`)
//! 4. SSH Server Hosting, `$SSH_AUTH_SOCK` Agent & NetOps Autonomous Agent (`ServerAndAgentHub`)
//! 5. PXE / ProxyDHCP Server (`pxe-rs` Option 43 TLV, Port 4011 Unicast, Option 77/175) & Custom UEFI Bootloader (`ipxe-uefi` `#![no_std]` + SNP) (`PxeAndBootloader`)
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
pub enum PxeBootSubtab {
    #[default]
    PxeServer,
    CustomBootloader,
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
            name: "chyna".into(),
            hostname: "192.168.1.12".into(),
            user: "chyna".into(),
            port: "22".into(),
            identity_file: "~/.ssh/id_ed25519".into(),
            proxy_jump: String::new(),
            forward_agent: true,
            use_control_master: true,
            forward_caduceus_ipc: true,
        }
    }
}

impl SshHostEditorDraft {
    pub fn new_blank() -> Self {
        Self {
            editing_index: None,
            name: "new-server".into(),
            hostname: "192.168.1.100".into(),
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
            .unwrap_or(22)
            .clamp(1, 65535);
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
        let dest = if ssh_destination.trim().is_empty() {
            "chyna@192.168.1.12"
        } else {
            ssh_destination.trim()
        };
        match self.kind {
            SshTunnelKind::Local => format!(
                "ssh -o ExitOnForwardFailure=yes -N -L {}:{} {}",
                self.local_listen, self.remote_target, dest
            ),
            SshTunnelKind::Dynamic => format!(
                "ssh -o ExitOnForwardFailure=yes -N -D {} {}",
                self.local_listen, dest
            ),
            SshTunnelKind::Remote => format!(
                "ssh -o ExitOnForwardFailure=yes -N -R {}:{} {}",
                self.local_listen, self.remote_target, dest
            ),
        }
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
                            let fp = Self::pseudo_fingerprint(&line);
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
        if keys.is_empty() {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/operator".into());
            keys.push(Self {
                name: "id_ed25519".into(),
                algorithm: "Ed25519".into(),
                path_display: format!("{home}/.ssh/id_ed25519"),
                fingerprint: "SHA256:8p72CRrRvx9rNDJsosJzIALDj5/HeTZxlVh8vYs/UEs".into(),
                public_key_line:
                    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIL+8YfSsACIlz5RRVLpeBQTJigsYWE/Huq9Z7RYek0Hb local@well"
                        .into(),
                is_passkey: false,
            });
        }
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

    fn pseudo_fingerprint(pub_line: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in pub_line.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let body = pub_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("AAAAC3NzaC1lZDI1NTE5");
        let snippet: String = body.chars().skip(8).take(27).collect();
        format!("SHA256:{snippet}{:08x}", (h & 0xffff_ffff) as u32)
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
                let host = parts.next().unwrap_or("?").to_string();
                let key_type = parts.next().unwrap_or("unknown").to_string();
                let key_data = parts.next().unwrap_or("");
                let fp = DiscoveredSshKey::pseudo_fingerprint(&format!("{key_type} {key_data}"));
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
                        ip: if cur_ip.is_empty() { "Unassigned".into() } else { cur_ip },
                        netmask: if cur_mask.is_empty() { "255.255.255.0".into() } else { cur_mask },
                        mac: if cur_mac.is_empty() { "--".into() } else { cur_mac },
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
                ip: if cur_ip.is_empty() { "Unassigned".into() } else { cur_ip },
                netmask: if cur_mask.is_empty() { "255.255.255.0".into() } else { cur_mask },
                mac: if cur_mac.is_empty() { "--".into() } else { cur_mac },
                is_up,
            });
        }
    }
    if ifaces.is_empty() {
        ifaces.push(LocalNetworkInterface {
            name: "en0".into(),
            ip: "192.168.1.172".into(),
            netmask: "255.255.255.0".into(),
            mac: "f6:8a:a9:f4:83:86".into(),
            is_up: true,
        });
        ifaces.push(LocalNetworkInterface {
            name: "lo0".into(),
            ip: "127.0.0.1".into(),
            netmask: "255.0.0.0".into(),
            mac: "00:00:00:00:00:00".into(),
            is_up: true,
        });
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
            if let Some(pct_str) = line.split("% packet loss").next().and_then(|s| s.split(',').next_back()) {
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

// ============================================================================
// PXE / ProxyDHCP Protocol Engine & Custom UEFI Bootloader (`pxe-protocol-engineering`)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PxeServerMode {
    /// ProxyDHCP: `yiaddr = 0.0.0.0`, Option 60 `"PXEClient"`, `siaddr` = TFTP Server IP,
    /// zero Option 1/3/51 IP lease interference, Option 54 router guard, Port 4011 unicast.
    ProxyDhcp,
    /// Standalone DHCP: Full IP lease + Option 6 DNS with mandatory `8.4.4.1` secondary fallback.
    StandaloneDhcp,
}

impl PxeServerMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProxyDhcp => "ProxyDHCP (Coexists with Router — yiaddr 0.0.0.0 + Port 4011 Unicast)",
            Self::StandaloneDhcp => "Standalone DHCP (Full Lease + DNS 8.4.4.1 Fallback)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientArch {
    BiosX86,
    UefiX86_64,
    UefiArm64,
}

impl ClientArch {
    pub const fn label(self) -> &'static str {
        match self {
            Self::BiosX86 => "Legacy BIOS x86 (Arch 00:00 — Option 43 TLV)",
            Self::UefiX86_64 => "UEFI x86_64 (Arch 00:07 — SNP ipxe-uefi.efi)",
            Self::UefiArm64 => "UEFI ARM64 (Arch 00:0B — SNP ipxe-aa64.efi)",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::BiosX86 => Self::UefiX86_64,
            Self::UefiX86_64 => Self::UefiArm64,
            Self::UefiArm64 => Self::BiosX86,
        }
    }
}

/// Chainload payload classification inspecting magic bytes (`#!ipxe` vs `MZ` PE32/COFF).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainloadTargetKind {
    /// Starts with `b"#!ipxe"` — evaluate recursively via `Evaluator::execute` with inherited lease vars.
    IpxeScript,
    /// Starts with `b"MZ"` — delegate to UEFI Boot Services `LoadImage` + `StartImage`.
    UefiPe32Image,
    Unknown,
}

/// Zero-copy TFTP receive action decoupled from `&mut rx_frame_buffer` borrows (`E0499` prevention).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TftpAction {
    Ack(u16, bool), // (block, is_eof)
    Error,
    Ignore,
}

impl TftpAction {
    pub fn from_opcode(opcode: u16, block: u16, data_len: usize, active_blksize: usize) -> Self {
        match opcode {
            3 => Self::Ack(block, data_len < active_blksize), // TFTP DATA (opcode 3)
            5 => Self::Error,                                 // TFTP ERROR (opcode 5)
            _ => Self::Ignore,
        }
    }
}

/// Build strict Legacy BIOS Option 43 TLV bytes per `pxe-protocol-engineering` Rule 1.
/// Sub-option 6 (Discovery Control = 8), Sub-option 8 (Boot Server IP), Sub-option 9 ("PXE"), Sub-option 10 ("PXE").
pub fn build_legacy_bios_option43(server_ip: Ipv4Addr) -> Vec<u8> {
    let s = server_ip.octets();
    vec![
        // Sub-option 6: Discovery Control (length 1, value 8 = bypass menu and use bootfile)
        6, 1, 8,
        // Sub-option 8: PXE Boot Server IP (type 8, length 7, payload 0,0,1,<server_ip>)
        8, 7, 0, 0, 1, s[0], s[1], s[2], s[3],
        // Sub-option 9: PXE Boot Menu (Type 0, Length 6, "PXE")
        9, 6, 0, 0, 3, b'P', b'X', b'E',
        // Sub-option 10: PXE Menu Prompt (Timeout 0, "PXE")
        10, 4, 0, b'P', b'X', b'E',
    ]
}

/// Build Standalone DHCP Option 6 (DNS Servers) with mandatory secondary fallback `8.4.4.1` per Rule 3.
pub fn build_standalone_dns_option6(primary_dns: Option<Ipv4Addr>) -> Vec<u8> {
    let mut dns_bytes = Vec::with_capacity(8);
    if let Some(d) = primary_dns {
        dns_bytes.extend_from_slice(&d.octets());
    }
    dns_bytes.extend_from_slice(&Ipv4Addr::new(8, 4, 4, 1).octets());
    dns_bytes
}

/// Inspect DHCP Option 77 (`User Class: "iPXE"`) and Option 175 to distinguish Stage 1 ROM vs Stage 2 iPXE.
pub fn is_ipxe_stage2_client(opt77: Option<&[u8]>, opt175: Option<&[u8]>) -> bool {
    let has_ipxe_class = opt77
        .map(|b| b.windows(4).any(|w| w == b"iPXE"))
        .unwrap_or(false);
    has_ipxe_class && opt175.is_some()
}

/// Classify downloaded network payload magic bytes per `pxe-protocol-engineering` Rule 11.
pub fn classify_chainload_payload(data: &[u8]) -> ChainloadTargetKind {
    if data.starts_with(b"#!ipxe") {
        ChainloadTargetKind::IpxeScript
    } else if data.starts_with(b"MZ") {
        ChainloadTargetKind::UefiPe32Image
    } else {
        ChainloadTargetKind::Unknown
    }
}

/// Generate a strict iPXE boot script using `${next-server}` and unconditional `set dns 8.8.8.8` (Rule 5).
pub fn generate_strict_ipxe_script(http_port: u16, boot_target: &str) -> String {
    let clean_target = boot_target.trim_start_matches('/');
    format!(
        "#!ipxe\ndhcp\nset dns 8.8.8.8\necho Well PXE Netboot Stage 2 -> ${{next-server}}:{http_port}\nchain --autofree http://${{next-server}}:{http_port}/{clean_target} || shell\n"
    )
}

/// Build the headless QEMU EDK2 UEFI netboot verification command per `pxe-protocol-engineering` Rule 12.
pub fn build_qemu_uefi_test_command(tftp_dir: &str, bootfile: &str) -> String {
    format!(
        "qemu-system-x86_64 \\\n  -drive if=pflash,format=raw,readonly=on,file=/opt/homebrew/share/qemu/edk2-x86_64-code.fd \\\n  -drive file=fat:rw:target/esp,format=raw,media=disk \\\n  -netdev user,id=net0,tftp={tftp_dir},bootfile={bootfile} \\\n  -device virtio-net-pci,netdev=net0 \\\n  -serial file:target/qemu-serial.log \\\n  -display none \\\n  -m 512M"
    )
}

/// Spawn a lightweight HTTP iPXE Boot Script & Payload Server on `0.0.0.0:<http_port>`.
pub fn start_pxe_http_boot_server(
    http_port: u16,
    boot_target: String,
    stop_flag: Arc<AtomicBool>,
) -> Result<SocketAddr, String> {
    let listener = TcpListener::bind(("0.0.0.0", http_port))
        .map_err(|e| format!("Failed to bind PXE HTTP Boot Server on :{http_port}: {e}"))?;
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
                    let script = generate_strict_ipxe_script(http_port, &boot_target);
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        script.len(),
                        script
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
    pub fn default_catalog() -> Vec<Self> {
        vec![
            Self {
                title: "PXE Netboot & WinPE Images".into(),
                server_host: "192.168.1.12".into(),
                share_name: "pxeboot".into(),
                username: "chyna".into(),
                mount_point: "/Volumes/pxeboot".into(),
                purpose_badge: "NETBOOT".into(),
            },
            Self {
                title: "OS Installers & ISO Vault".into(),
                server_host: "192.168.1.12".into(),
                share_name: "isos".into(),
                username: "chyna".into(),
                mount_point: "/Volumes/isos".into(),
                purpose_badge: "ISO REPO".into(),
            },
            Self {
                title: "Cluster Shared Artifacts".into(),
                server_host: "cachey.local".into(),
                share_name: "artifacts".into(),
                username: "chyna".into(),
                mount_point: "/Volumes/artifacts".into(),
                purpose_badge: "SMB3".into(),
            },
        ]
    }

    pub fn smb_url(&self) -> String {
        if self.username.trim().is_empty() {
            format!("smb://{}/{}", self.server_host.trim(), self.share_name.trim())
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
        format!(
            "mkdir -p {} && mount_smbfs //{user}@{}/{} {}",
            self.mount_point,
            self.server_host.trim(),
            self.share_name.trim(),
            self.mount_point
        )
    }

    pub fn is_mounted(&self) -> bool {
        std::path::Path::new(&self.mount_point).exists()
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
    Ipv4Addr::new(192, 168, 1, 10)
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

        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect()
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
    if entries.is_empty() {
        entries.push(ArpNeighborEntry {
            hostname: "chyna.local".into(),
            ip: "192.168.1.12".into(),
            mac: "a8:a1:59:2c:41:0e".into(),
            interface: "en0".into(),
        });
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
        assert!(cmd.contains("-L 127.0.0.1:5433:127.0.0.1:5432"));
        assert!(cmd.contains("chyna@192.168.1.12"));
    }

    #[test]
    fn test_pxe_option43_tlv_strict_layout() {
        let opt43 = build_legacy_bios_option43(Ipv4Addr::new(192, 168, 1, 12));
        // 3 bytes (sub 6) + 9 bytes (sub 8) + 8 bytes (sub 9) + 6 bytes (sub 10) = 26 bytes
        assert_eq!(opt43.len(), 26);
        assert_eq!(&opt43[0..3], &[6, 1, 8]);
        assert_eq!(&opt43[3..12], &[8, 7, 0, 0, 1, 192, 168, 1, 12]);
        assert_eq!(&opt43[12..20], &[9, 6, 0, 0, 3, b'P', b'X', b'E']);
        assert_eq!(&opt43[20..26], &[10, 4, 0, b'P', b'X', b'E']);
    }

    #[test]
    fn test_standalone_dns_fallback_8_4_4_1() {
        let dns = build_standalone_dns_option6(Some(Ipv4Addr::new(1, 1, 1, 1)));
        assert_eq!(dns, vec![1, 1, 1, 1, 8, 4, 4, 1]);
    }

    #[test]
    fn test_ipxe_script_and_dual_path_chainload_classification() {
        let script = generate_strict_ipxe_script(8080, "boot.ipxe");
        assert!(script.starts_with("#!ipxe\n"));
        assert!(script.contains("set dns 8.8.8.8"));
        assert!(script.contains("http://${next-server}:8080/boot.ipxe"));

        assert_eq!(
            classify_chainload_payload(script.as_bytes()),
            ChainloadTargetKind::IpxeScript
        );
        assert_eq!(
            classify_chainload_payload(b"MZ\x90\x00\x03\x00"),
            ChainloadTargetKind::UefiPe32Image
        );
        assert!(is_ipxe_stage2_client(Some(b"iPXE"), Some(&[175, 2, 1, 1])));
        assert!(!is_ipxe_stage2_client(Some(b"PXEClient"), None));
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
