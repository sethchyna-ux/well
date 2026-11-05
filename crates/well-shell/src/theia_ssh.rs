//! Theia SSH Integration Engine (`theia_ssh`)
//! Bridges OpenSSH config discovery (`~/.ssh/config`), per-host environment tier classification,
//! Theia's Prism auto-theme palette assignment (e.g., Crimson for `prod-*`, Amber for `staging-*`,
//! Emerald for `dev-*`), ControlMaster socket multiplexing detection, and Caduceus IPC socket
//! forwarding (`-R /tmp/well-caduceus.sock:/tmp/well-caduceus.sock`).

use crate::ssh::{SshHostConfig, SshTarget};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Environment risk/tier classification for a remote SSH host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SshHostTier {
    Production,
    Staging,
    #[default]
    Development,
}

impl SshHostTier {
    pub fn classify(host_name: &str, hostname: &str) -> Self {
        let h = format!("{} {}", host_name, hostname).to_ascii_lowercase();
        if h.contains("prod") || h.contains("prd") || h.contains("live") || h.contains("master") {
            Self::Production
        } else if h.contains("stag")
            || h.contains("uat")
            || h.contains("qa")
            || h.contains("preprod")
        {
            Self::Staging
        } else {
            Self::Development
        }
    }

    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::Production => "PROD",
            Self::Staging => "STAGING",
            Self::Development => "DEV",
        }
    }

    /// Suggested Theia's Prism theme ID or accent hex color `(border_rgb, bg_tint_rgb, badge_rgb)`
    /// so operators never confuse a production remote pane with a local shell.
    pub const fn prism_palette(self) -> (u32, u32, u32) {
        match self {
            // Crimson border & dark crimson tint for Production
            Self::Production => (0xfb4934, 0x2b1616, 0xcc241d),
            // Amber Gold border & warm obsidian tint for Staging
            Self::Staging => (0xfabd2f, 0x292214, 0xd79921),
            // Emerald Cyan border & forest tint for Dev
            Self::Development => (0x8ec07c, 0x1d261f, 0x689d6a),
        }
    }
}

/// Rich Theia SSH host profile combining `~/.ssh/config` with Well-specific operator settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TheiaSshProfile {
    pub host: SshHostConfig,
    pub tier: SshHostTier,
    pub forward_caduceus_ipc: bool,
    pub use_control_master: bool,
    pub auto_reconnect: bool,
}

impl TheiaSshProfile {
    pub fn from_host(host: SshHostConfig) -> Self {
        let tier = SshHostTier::classify(&host.name, &host.hostname);
        Self {
            host,
            tier,
            forward_caduceus_ipc: false,
            use_control_master: true,
            auto_reconnect: true,
        }
    }

    /// Return a secret-redacted display string for the identity key path (shows filename only).
    pub fn redacted_identity_display(&self) -> Option<String> {
        self.host.identity_file.as_ref().map(|p| {
            let fname = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "id_key".into());
            format!("~/.ssh/{fname} (redacted)")
        })
    }

    /// Check whether a local OpenSSH ControlMaster socket exists for this host in `~/.ssh/` or `/tmp/`.
    pub fn has_active_control_master(&self) -> bool {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let ssh_dir = home.join(".ssh");
            let candidate = ssh_dir.join(format!("cm-{}", self.host.name));
            if candidate.exists() {
                return true;
            }
        }
        let tmp_candidate = PathBuf::from(format!("/tmp/well-ssh-cm-{}", self.host.name));
        tmp_candidate.exists()
    }

    /// Build an `SshTarget` enriched with optional Caduceus IPC socket forwarding (`-R`)
    /// and ControlMaster multiplexing flags.
    pub fn build_target(&self, base_target: SshTarget) -> SshTarget {
        let mut target = base_target;
        if self.forward_caduceus_ipc {
            target.extra_args.push("-R".into());
            target
                .extra_args
                .push("/tmp/well-caduceus.sock:/tmp/well-caduceus.sock".into());
        }
        if self.use_control_master {
            target.extra_args.push("-o".into());
            target.extra_args.push("ControlMaster=auto".into());
            target.extra_args.push("-o".into());
            target.extra_args.push("ControlPersist=600".into());
        }
        target
    }

    /// Format the Astraea prompt badge (`[THEIA-SSH: user@host (prod)]`) for remote context awareness.
    pub fn astraea_badge(&self) -> String {
        let user = self.host.user.as_deref().unwrap_or("root");
        let tier_lower = match self.tier {
            SshHostTier::Production => "prod",
            SshHostTier::Staging => "staging",
            SshHostTier::Development => "dev",
        };
        format!("[THEIA-SSH: {}@{} ({})]", user, self.host.name, tier_lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_classification_and_palette() {
        assert_eq!(
            SshHostTier::classify("prod-api-01", "10.0.1.10"),
            SshHostTier::Production
        );
        assert_eq!(
            SshHostTier::classify("staging-db", "10.0.2.10"),
            SshHostTier::Staging
        );
        assert_eq!(
            SshHostTier::classify("dev-box", "192.168.1.50"),
            SshHostTier::Development
        );

        let (border, bg, _) = SshHostTier::Production.prism_palette();
        assert_eq!(border, 0xfb4934); // Crimson border for production
        assert_eq!(bg, 0x2b1616);
    }

    #[test]
    fn test_caduceus_ipc_forwarding_and_astraea_badge() {
        let host = SshHostConfig {
            name: "prod-k8s".into(),
            hostname: "10.20.30.40".into(),
            user: Some("ops".into()),
            port: 2222,
            ..Default::default()
        };
        let mut profile = TheiaSshProfile::from_host(host);
        profile.forward_caduceus_ipc = true;

        assert_eq!(profile.astraea_badge(), "[THEIA-SSH: ops@prod-k8s (prod)]");

        let base = SshTarget {
            destination: "10.20.30.40".into(),
            user: Some("ops".into()),
            port: 2222,
            ..Default::default()
        };
        let enriched = profile.build_target(base);
        let cmd_args = enriched.to_command_args();
        assert!(cmd_args.contains(&"-R".to_string()));
        assert!(cmd_args.contains(&"/tmp/well-caduceus.sock:/tmp/well-caduceus.sock".to_string()));
    }
}
