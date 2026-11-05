//! Parameterized Snippet Engine for Mneme Inline Editor and Metis History.
//! Supports interactive tab-stop placeholders (`${1:namespace}`, `${2:pod_name}`),
//! scope filtering (`Local`, `SshRemote`, `Global`), and atomic TOML persistence.

use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SnippetScope {
    Local,
    SshRemote,
    #[default]
    Global,
}

impl SnippetScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::SshRemote => "SSH Remote",
            Self::Global => "Global",
        }
    }

    pub fn matches_context(self, is_ssh: bool) -> bool {
        match self {
            Self::Global => true,
            Self::Local => !is_ssh,
            Self::SshRemote => is_ssh,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WellSnippet {
    pub trigger: String,
    pub label: String,
    pub template: String,
    #[serde(default)]
    pub scope: SnippetScope,
}

/// A parsed placeholder stop after expanding a template into plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpandedTabStop {
    pub index: u32,
    pub default_value: String,
    /// Character range (`start_char..end_char`) inside the expanded text.
    pub char_range: Range<usize>,
}

/// Result of expanding a `WellSnippet::template` into concrete editor text and tab-stop ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpandedSnippet {
    pub text: String,
    /// Sorted by `index` ascending (`1`, `2`, `3`, ...).
    pub tab_stops: Vec<ExpandedTabStop>,
}

impl WellSnippet {
    /// Default built-in catalog of operator snippets across Local, SshRemote, and Global scopes.
    pub fn builtin_catalog() -> Vec<Self> {
        vec![
            Self {
                trigger: ":klogs".into(),
                label: "Tail Kubernetes Pod Logs".into(),
                template: "kubectl logs -f -n ${1:default} ${2:pod-name}".into(),
                scope: SnippetScope::Global,
            },
            Self {
                trigger: ":kexec".into(),
                label: "Exec Interactive Shell in Pod".into(),
                template: "kubectl exec -it -n ${1:default} ${2:pod-name} -- ${3:/bin/sh}".into(),
                scope: SnippetScope::Global,
            },
            Self {
                trigger: ":tfplan".into(),
                label: "Terraform Plan with Var File".into(),
                template: "terraform plan -var-file=${1:vars/env.tfvars} -out=${2:tfplan}".into(),
                scope: SnippetScope::Local,
            },
            Self {
                trigger: ":killport".into(),
                label: "Kill Process Listening on TCP Port".into(),
                template: "lsof -ti :${1:8080} | xargs kill -9".into(),
                scope: SnippetScope::Local,
            },
            Self {
                trigger: ":jctl".into(),
                label: "Follow Remote Systemd Unit Journal".into(),
                template: "sudo journalctl -u ${1:nginx.service} -f -n ${2:100}".into(),
                scope: SnippetScope::SshRemote,
            },
            Self {
                trigger: ":dockersh".into(),
                label: "Attach to Running Remote Container".into(),
                template: "docker exec -it ${1:container_name} ${2:/bin/bash}".into(),
                scope: SnippetScope::SshRemote,
            },
            Self {
                trigger: ":gwork".into(),
                label: "Create Git Worktree Branch".into(),
                template: "git worktree add ../${1:worktree-dir} -b ${2:feat/branch-name}".into(),
                scope: SnippetScope::Local,
            },
        ]
    }

    /// Expand `${N:default}` or `${N}` placeholders into plain text and character offset ranges.
    pub fn expand(&self) -> ExpandedSnippet {
        expand_snippet_template(&self.template)
    }
}

/// Parse a template string containing `${1:default}` or `${1}` markers and return
/// the rendered plain text along with sorted `ExpandedTabStop` character ranges.
pub fn expand_snippet_template(template: &str) -> ExpandedSnippet {
    let mut output = String::with_capacity(template.len());
    let mut stops = Vec::new();
    let mut current_char_len = 0usize;

    let bytes = template.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            // Find closing '}'
            if let Some(rel_close) = template[i + 2..].find('}') {
                let inner = &template[i + 2..i + 2 + rel_close];
                let (idx_str, default_val) = match inner.split_once(':') {
                    Some((num, def)) => (num.trim(), def),
                    None => (inner.trim(), ""),
                };
                if let Ok(index) = idx_str.parse::<u32>() {
                    let val_chars = default_val.chars().count();
                    let start_char = current_char_len;
                    let end_char = start_char + val_chars;
                    output.push_str(default_val);
                    current_char_len = end_char;
                    stops.push(ExpandedTabStop {
                        index,
                        default_value: default_val.to_string(),
                        char_range: start_char..end_char,
                    });
                    i = i + 2 + rel_close + 1;
                    continue;
                }
            }
        }
        // Normal UTF-8 char
        if let Some(ch) = template[i..].chars().next() {
            output.push(ch);
            current_char_len += 1;
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    stops.sort_by_key(|s| s.index);
    ExpandedSnippet {
        text: output,
        tab_stops: stops,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_snippet_template_with_multiple_stops() {
        let snippet = WellSnippet {
            trigger: ":klogs".into(),
            label: "Tail Kubernetes Pod Logs".into(),
            template: "kubectl logs -f -n ${1:default} ${2:pod-name}".into(),
            scope: SnippetScope::Global,
        };
        let expanded = snippet.expand();
        assert_eq!(expanded.text, "kubectl logs -f -n default pod-name");
        assert_eq!(expanded.tab_stops.len(), 2);

        let first = &expanded.tab_stops[0];
        assert_eq!(first.index, 1);
        assert_eq!(first.default_value, "default");
        assert_eq!(&expanded.text[first.char_range.clone()], "default");

        let second = &expanded.tab_stops[1];
        assert_eq!(second.index, 2);
        assert_eq!(second.default_value, "pod-name");
        assert_eq!(&expanded.text[second.char_range.clone()], "pod-name");
    }

    #[test]
    fn test_scope_matching() {
        assert!(SnippetScope::Global.matches_context(false));
        assert!(SnippetScope::Global.matches_context(true));
        assert!(SnippetScope::Local.matches_context(false));
        assert!(!SnippetScope::Local.matches_context(true));
        assert!(!SnippetScope::SshRemote.matches_context(false));
        assert!(SnippetScope::SshRemote.matches_context(true));
    }
}
