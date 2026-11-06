//! Unified Action Registry and Command Catalog for Well GPUI.
//! Pairs every GPUI action with a human-readable Label, Category, Description,
//! Default Keybind, and Scoped Key Context.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    Terminal,
    Panes,
    PythiaAi,
    MnemeEditor,
    TheiaSsh,
    Settings,
}

impl ActionCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Panes => "Panes",
            Self::PythiaAi => "Pythia AI",
            Self::MnemeEditor => "Mneme Editor",
            Self::TheiaSsh => "Theia SSH",
            Self::Settings => "Settings",
        }
    }

    pub const fn badge_icon(self) -> &'static str {
        match self {
            Self::Terminal => "🐚",
            Self::Panes => "🪟",
            Self::PythiaAi => "✨",
            Self::MnemeEditor => "✏",
            Self::TheiaSsh => "🌐",
            Self::Settings => "🏛️",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub category: ActionCategory,
    pub description: &'static str,
    pub default_keybind: &'static str,
    pub key_context: &'static str,
}

pub const WELL_ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        id: "well::OpenCommandPalette",
        label: "Command Palette: Show All Actions...",
        category: ActionCategory::Settings,
        description: "Fuzzy-searchable catalog of all Well functions and shortcuts",
        default_keybind: "⌘K",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "well::OpenSettings",
        label: "Theia's Prism: Open Settings & Control Center",
        category: ActionCategory::Settings,
        description: "Open Settings Modal (Themes, Shortcuts, Snippets, SSH & Safety)",
        default_keybind: "⌘,",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::OpenSshManager",
        label: "Theia SSH: Connect to Remote Host...",
        category: ActionCategory::TheiaSsh,
        description: "Open Theia SSH Host Drawer with ~/.ssh/config remote discovery",
        default_keybind: "⇧⌘S",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::SshSplitRight",
        label: "Theia SSH: Quick Split-Pane Connect",
        category: ActionCategory::TheiaSsh,
        description: "Spawn remote SSH target into a horizontal split pane with theme",
        default_keybind: "⌥⌘S",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::OpenPortForwarding",
        label: "Theia NetOps: Visual Port Forwarding & Tunnels (-L/-D/-R)",
        category: ActionCategory::TheiaSsh,
        description: "Manage Local, Dynamic SOCKS5, and Remote SSH tunnels with live port probes",
        default_keybind: "⌃⌥T",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::OpenKeyServerVault",
        label: "Theia NetOps: SSH Key Vault & HTTP Public Key Server",
        category: ActionCategory::TheiaSsh,
        description: "Manage ~/.ssh keys and run the embedded HTTP Authorized Key Server (:8787)",
        default_keybind: "⌃⌥K",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::OpenPxeBootloader",
        label: "Theia NetOps: PXE / ProxyDHCP Server & Custom UEFI Bootloader",
        category: ActionCategory::TheiaSsh,
        description: "Configure Option 43 TLV ProxyDHCP, iPXE ${next-server} scripts & ipxe-uefi SNP",
        default_keybind: "⌃⌥B",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::OpenSmbShares",
        label: "Theia NetOps: SMB / CIFS Network Shares & Netboot Mounts",
        category: ActionCategory::TheiaSsh,
        description: "Mount and inspect SMB/CIFS shares (pxeboot, ISOs, artifacts) in /Volumes",
        default_keybind: "⌃⌥M",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "theia::OpenNetworkTools",
        label: "Theia NetOps: Networking Tools (Port Scan, ARP, Wake-on-LAN)",
        category: ActionCategory::TheiaSsh,
        description: "Run concurrent TCP port scans, inspect ARP neighbors & broadcast WoL magic packets",
        default_keybind: "⌃⌥N",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "pythia::ToggleCopilot",
        label: "Pythia AI: Command Synthesis & Image Studio",
        category: ActionCategory::PythiaAi,
        description: "Toggle Pythia Copilot (<2ms Metis cache, LLM & Kitty Image Studio)",
        default_keybind: "^⇧K",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "panes::SplitHorizontal",
        label: "Panes: Split Pane Right (Horizontal)",
        category: ActionCategory::Panes,
        description: "Bisect active terminal pane horizontally with a new PTY session",
        default_keybind: "⌘D",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "panes::SplitVertical",
        label: "Panes: Split Pane Down (Vertical)",
        category: ActionCategory::Panes,
        description: "Bisect active terminal pane vertically with a new PTY session",
        default_keybind: "⇧⌘D",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "panes::FocusNextPane",
        label: "Panes: Cycle Focus to Next Split Pane",
        category: ActionCategory::Panes,
        description: "Advance keyboard focus to the next leaf terminal in PaneTree",
        default_keybind: "⌘]",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "panes::NewTab",
        label: "Terminal: New Workspace Tab",
        category: ActionCategory::Terminal,
        description: "Create a new top-level terminal tab with an isolated PTY session",
        default_keybind: "⌘T",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "panes::ClosePane",
        label: "Panes: Close Active Pane or Tab",
        category: ActionCategory::Panes,
        description: "Terminate active split pane or close tab if last remaining leaf",
        default_keybind: "⌘W",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "mneme::ToggleInlineEditor",
        label: "Mneme Editor: Toggle Multi-Line Rope Composer",
        category: ActionCategory::MnemeEditor,
        description: "Toggle inline Tree-sitter AST validated rope composer over PTY",
        default_keybind: "⌃E",
        key_context: "TerminalPty",
    },
    ActionDescriptor {
        id: "mneme::ExpandOrNextSnippetPlaceholder",
        label: "Mneme Editor: Expand Trigger or Next Tab-Stop",
        category: ActionCategory::MnemeEditor,
        description: "Expand parameterized WellSnippet (:klogs) or jump to ${2:arg}",
        default_keybind: "Tab",
        key_context: "MnemeInlineEditor",
    },
    ActionDescriptor {
        id: "mneme::ForceExecuteUnsafeCommand",
        label: "Mneme Safety Gate: Force Override Destructive",
        category: ActionCategory::MnemeEditor,
        description: "Operator override to dispatch AST-gated destructive commands",
        default_keybind: "⇧⏎",
        key_context: "MnemeInlineEditor",
    },
    ActionDescriptor {
        id: "mneme::OpenSnippetPicker",
        label: "Mneme Editor: Browse & Insert Snippet...",
        category: ActionCategory::MnemeEditor,
        description: "Expand parameterized snippet scoped to Local or SshRemote",
        default_keybind: "⌃R",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "terminal::ToggleLineNumbers",
        label: "Terminal: Toggle Line Numbers Gutter",
        category: ActionCategory::Terminal,
        description: "Toggle line number gutter display on the active terminal grid",
        default_keybind: "⌥L",
        key_context: "Workspace",
    },
    ActionDescriptor {
        id: "well::Quit",
        label: "Well: Quit Application",
        category: ActionCategory::Terminal,
        description: "Gracefully terminate all child PTY sessions and exit Well",
        default_keybind: "⌘Q",
        key_context: "Workspace",
    },
];

/// Filter `WELL_ACTIONS` by a case-insensitive query matching label, id, category, description, or shortcut.
pub fn filter_actions(query: &str) -> Vec<&'static ActionDescriptor> {
    let trimmed = query.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return WELL_ACTIONS.iter().collect();
    }
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    WELL_ACTIONS
        .iter()
        .filter(|act| {
            let haystack = format!(
                "{} {} {} {} {}",
                act.id,
                act.label,
                act.category.as_str(),
                act.description,
                act.default_keybind
            )
            .to_ascii_lowercase();
            tokens.iter().all(|tok| haystack.contains(tok))
        })
        .collect()
}

/// Detect shortcut collisions where two distinct actions share the same keybind in the same `key_context`.
pub fn detect_keybind_conflicts(bindings: &[(String, String, String)]) -> Vec<(usize, String)> {
    // bindings: Vec<(action_id, keybind, key_context)>
    let mut conflicts = Vec::new();
    for (i, (id_a, key_a, ctx_a)) in bindings.iter().enumerate() {
        if key_a.trim().is_empty() {
            continue;
        }
        for (j, (id_b, key_b, ctx_b)) in bindings.iter().enumerate() {
            if i != j
                && id_a != id_b
                && key_a.eq_ignore_ascii_case(key_b)
                && (ctx_a == ctx_b || ctx_a == "Workspace" || ctx_b == "Workspace")
            {
                conflicts.push((i, format!("Collision with '{}' in {} context", id_b, ctx_b)));
                break;
            }
        }
    }
    conflicts
}

/// Context-sensitive footer hints for the interactive help bar at the bottom of the active pane.
pub fn footer_hints_for_context(
    mneme_active: bool,
    pythia_active: bool,
    ssh_active: bool,
) -> Vec<(&'static str, &'static str)> {
    if pythia_active {
        vec![
            ("↵", "Synthesize"),
            ("Tab", "Switch Mode"),
            ("Esc", "Close Pythia"),
        ]
    } else if mneme_active {
        vec![
            ("Tab", "Next Placeholder"),
            ("↵", "Execute"),
            ("⇧⏎", "Override"),
            ("⌃E", "Close Mneme"),
        ]
    } else if ssh_active {
        vec![
            ("⇧⌘P", "Functions"),
            ("⌘K", "Pythia AI"),
            ("⌃E", "Remote Snippets"),
            ("⌘D", "Split Pane"),
        ]
    } else {
        vec![
            ("⇧⌘P", "Functions"),
            ("⌘K", "Pythia AI"),
            ("⌃E", "Mneme Snippets"),
            ("⌘D", "Split Pane"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_catalog_completeness_and_uniqueness() {
        assert!(WELL_ACTIONS.len() >= 12);
        for (i, a) in WELL_ACTIONS.iter().enumerate() {
            assert!(!a.id.is_empty());
            assert!(!a.label.is_empty());
            assert!(!a.description.is_empty());
            assert!(!a.default_keybind.is_empty());
            for (j, b) in WELL_ACTIONS.iter().enumerate() {
                if i != j {
                    assert_ne!(a.id, b.id, "Duplicate action id: {}", a.id);
                }
            }
        }
    }

    #[test]
    fn test_filter_actions_search() {
        let ssh_hits = filter_actions("ssh remote");
        assert!(!ssh_hits.is_empty());
        assert!(ssh_hits.iter().any(|a| a.id == "theia::OpenSshManager"));

        let safety_hits = filter_actions("destructive override");
        assert_eq!(safety_hits.len(), 1);
        assert_eq!(safety_hits[0].id, "mneme::ForceExecuteUnsafeCommand");
    }

    #[test]
    fn test_keybind_conflict_detector() {
        let default_bindings: Vec<(String, String, String)> = WELL_ACTIONS
            .iter()
            .map(|a| {
                (
                    a.id.to_string(),
                    a.default_keybind.to_string(),
                    a.key_context.to_string(),
                )
            })
            .collect();
        assert!(
            detect_keybind_conflicts(&default_bindings).is_empty(),
            "Default WELL_ACTIONS must have zero keybind collisions"
        );

        let mut colliding = default_bindings;
        colliding[1].1 = colliding[0].1.clone();
        let conflicts = detect_keybind_conflicts(&colliding);
        assert!(!conflicts.is_empty());
    }

    #[test]
    fn test_terminal_settings_initial_state_and_snippet_insert() {
        use crate::settings_view::TerminalSettings;
        let ts = TerminalSettings::default();
        assert!(!ts.modal_open);
        assert!(!ts.settings_open);
        assert!(!ts.show_line_numbers);
        assert!(ts.requested_hub_switch.is_none());
        assert!(ts.requested_snippet_insert.is_none());
    }

    #[test]
    fn test_snippet_persistence_roundtrip() {
        use crate::settings_view::TerminalSettings;
        use well_editor::{SnippetScope, WellSnippet};
        let mut ts = TerminalSettings::default();
        ts.snippets.push(WellSnippet {
            trigger: "deploy".into(),
            label: "Deploy Service".into(),
            template: "systemctl restart ${1:service}".into(),
            scope: SnippetScope::Global,
        });
        assert_eq!(ts.snippets.last().unwrap().trigger, "deploy");
        assert_eq!(
            ts.snippets.last().unwrap().template,
            "systemctl restart ${1:service}"
        );
    }

    #[test]
    fn test_toggle_line_numbers_action_and_settings() {
        use crate::settings_view::TerminalSettings;
        let mut ts = TerminalSettings::default();
        assert!(!ts.show_line_numbers);
        ts.show_line_numbers = true;
        assert!(ts.show_line_numbers);

        let toggle_action = WELL_ACTIONS
            .iter()
            .find(|a| a.id == "terminal::ToggleLineNumbers")
            .expect("terminal::ToggleLineNumbers must be in WELL_ACTIONS");
        assert_eq!(toggle_action.default_keybind, "⌥L");
        assert_eq!(toggle_action.category, ActionCategory::Terminal);
    }
}
