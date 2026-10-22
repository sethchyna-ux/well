//! Deterministic command ranking over a compressed prefix trie.
//! Cached winner IDs make a single suggestion lookup proportional to prefix
//! length, plus copying its result. Arena storage avoids recursive destruction.
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct MetisTrieNode {
    pub children: HashMap<char, MetisTrieEdge>,
    pub is_terminal: bool,
    pub frequency: u64,
    command: Option<usize>,
    best: Option<usize>,
}
#[derive(Debug, Clone)]
pub struct MetisTrieEdge {
    pub label: String,
    pub child: usize,
}
#[derive(Debug, Clone)]
struct Entry {
    command: String,
    frequency: u64,
    last_used: u64,
}
#[derive(Debug, Clone)]
pub struct MetisHistory {
    nodes: Vec<MetisTrieNode>,
    entries: Vec<Entry>,
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
            nodes: vec![MetisTrieNode::default()],
            entries: Vec::new(),
            total_commands: 0,
        }
    }
    pub fn learn_command(&mut self, command: &str) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        let (node, path) = self.insert_path(command);
        self.total_commands = self.total_commands.saturating_add(1);
        let id = match self.nodes[node].command {
            Some(id) => id,
            None => {
                let id = self.entries.len();
                self.entries.push(Entry {
                    command: command.to_owned(),
                    frequency: 0,
                    last_used: 0,
                });
                self.nodes[node].command = Some(id);
                id
            }
        };
        self.entries[id].frequency = self.entries[id].frequency.saturating_add(1);
        self.entries[id].last_used = self.total_commands;
        self.nodes[node].is_terminal = true;
        self.nodes[node].frequency = self.entries[id].frequency;
        for node in path {
            if self.nodes[node]
                .best
                .is_none_or(|best| self.rank(id, best).is_gt())
            {
                self.nodes[node].best = Some(id);
            }
        }
    }
    fn new_node(&mut self) -> usize {
        let id = self.nodes.len();
        self.nodes.push(MetisTrieNode::default());
        id
    }
    fn insert_path(&mut self, mut remaining: &str) -> (usize, Vec<usize>) {
        let mut node = 0;
        let mut path = vec![0];
        while let Some(first) = remaining.chars().next() {
            let Some(edge) = self.nodes[node].children.get(&first).cloned() else {
                let child = self.new_node();
                self.nodes[node].children.insert(
                    first,
                    MetisTrieEdge {
                        label: remaining.to_owned(),
                        child,
                    },
                );
                path.push(child);
                return (child, path);
            };
            let common = edge
                .label
                .chars()
                .zip(remaining.chars())
                .take_while(|(a, b)| a == b)
                .map(|(ch, _)| ch.len_utf8())
                .sum::<usize>();
            if common == edge.label.len() {
                node = edge.child;
                remaining = &remaining[common..];
                path.push(node);
                continue;
            }
            let split = self.new_node();
            self.nodes[split].best = self.nodes[edge.child].best;
            self.nodes[split].children.insert(
                edge.label[common..].chars().next().unwrap(),
                MetisTrieEdge {
                    label: edge.label[common..].to_owned(),
                    child: edge.child,
                },
            );
            self.nodes[node].children.insert(
                first,
                MetisTrieEdge {
                    label: edge.label[..common].to_owned(),
                    child: split,
                },
            );
            path.push(split);
            remaining = &remaining[common..];
            if remaining.is_empty() {
                return (split, path);
            }
            let child = self.new_node();
            self.nodes[split].children.insert(
                remaining.chars().next().unwrap(),
                MetisTrieEdge {
                    label: remaining.to_owned(),
                    child,
                },
            );
            path.push(child);
            return (child, path);
        }
        (node, path)
    }
    fn prefix_node(&self, mut remaining: &str) -> Option<usize> {
        if remaining.is_empty() {
            return None;
        }
        let mut node = 0;
        loop {
            let edge = self.nodes[node].children.get(&remaining.chars().next()?)?;
            if remaining.starts_with(&edge.label) {
                remaining = &remaining[edge.label.len()..];
                node = edge.child;
                if remaining.is_empty() {
                    return Some(node);
                }
            } else if edge.label.starts_with(remaining) {
                return Some(edge.child);
            } else {
                return None;
            }
        }
    }
    fn rank(&self, a: usize, b: usize) -> std::cmp::Ordering {
        let (a, b) = (&self.entries[a], &self.entries[b]);
        a.frequency
            .cmp(&b.frequency)
            .then(a.last_used.cmp(&b.last_used))
            .then_with(|| b.command.cmp(&a.command))
    }
    pub fn get_autosuggestion(&self, prefix: &str) -> Option<String> {
        let id = self.nodes[self.prefix_node(prefix)?].best?;
        Some(self.entries[id].command.clone())
    }
    /// Rank matches by frequency, recency, then lexical order, independently of hash iteration order.
    pub fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let Some(node) = self.prefix_node(prefix) else {
            return Vec::new();
        };
        let mut pending = vec![node];
        let mut found = Vec::new();
        while let Some(node) = pending.pop() {
            let node = &self.nodes[node];
            if let Some(command) = node.command {
                found.push(command);
            }
            pending.extend(node.children.values().map(|e| e.child));
        }
        found.sort_unstable_by(|&a, &b| self.rank(b, a));
        found
            .into_iter()
            .take(limit)
            .map(|id| self.entries[id].command.clone())
            .collect()
    }
    pub fn total_commands(&self) -> u64 {
        self.total_commands
    }
    pub fn unique_commands(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_full_commands_rather_than_greedy_child_frequencies() {
        let mut h = MetisHistory::new();
        for _ in 0..20 {
            h.learn_command("git status --short");
        }
        h.learn_command("git show");
        h.learn_command("git stash");
        assert_eq!(
            h.get_autosuggestion("git s").as_deref(),
            Some("git status --short")
        );
        for _ in 0..30 {
            h.learn_command("git");
        }
        assert_eq!(h.get_autosuggestion("git").as_deref(), Some("git"));
        assert_eq!(
            h.suggestions("git", 3),
            ["git", "git status --short", "git stash"]
        );
    }
    #[test]
    fn compressed_edge_splits_and_unicode_prefixes_preserve_cached_winners() {
        let commands = [
            "cargo build --release",
            "cargo",
            "cat",
            "cargo build",
            "café λ",
            "café 界",
            "c",
        ];
        let mut h = MetisHistory::new();
        for command in commands {
            h.learn_command(command);
        }
        h.learn_command("cargo build --release");
        assert_eq!(
            h.get_autosuggestion("car").as_deref(),
            Some("cargo build --release")
        );
        assert_eq!(h.get_autosuggestion("café").as_deref(), Some("café 界"));
        assert_eq!(h.get_autosuggestion("café λ").as_deref(), Some("café λ"));
        assert_eq!(h.get_autosuggestion("cafè"), None);
        assert_eq!(h.get_autosuggestion(""), None);
        assert!(h.suggestions("c", 0).is_empty());
        assert_eq!(h.unique_commands(), 7);
        assert_eq!(h.total_commands(), 8);
    }
    #[test]
    fn arena_handles_long_commands_without_recursive_nodes() {
        let mut h = MetisHistory::new();
        let long = format!("printf {}", "x".repeat(100_000));
        h.learn_command(&long);
        assert_eq!(
            h.get_autosuggestion("printf x").as_deref(),
            Some(long.as_str())
        );
        assert_eq!(h.nodes.len(), 2);
    }
    #[test]
    fn indexing_260000_commands_returns_the_actual_most_frequent_match() {
        let mut h = MetisHistory::new();
        for n in 0..260_000 {
            h.learn_command(&format!("run task-{n:06}"));
        }
        h.learn_command("run task-123456");
        assert_eq!(h.unique_commands(), 260_000);
        assert_eq!(
            h.get_autosuggestion("run task-").as_deref(),
            Some("run task-123456")
        );
    }
}
