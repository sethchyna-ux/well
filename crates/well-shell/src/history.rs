//! Deterministic command ranking over a compressed prefix trie.
//! Cached winner IDs make a single suggestion lookup proportional to prefix
//! length, plus copying its result. Arena storage avoids recursive destruction.
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetisRecipe {
    pub prompt: String,
    pub command: String,
    pub cluster_id: String,
    pub confidence: f32,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MetisHistory {
    nodes: Vec<MetisTrieNode>,
    entries: Vec<Entry>,
    total_commands: u64,
    recipes: Vec<MetisRecipe>,
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
            recipes: Vec::new(),
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
            return Some(0);
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
        if prefix.is_empty() {
            return None;
        }
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
    /// Returns the most recently executed commands up to `limit`.
    pub fn recent_commands(&self, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let mut entries: Vec<&Entry> = self.entries.iter().collect();
        entries.sort_unstable_by_key(|a| std::cmp::Reverse(a.last_used));
        entries
            .into_iter()
            .take(limit)
            .map(|e| e.command.clone())
            .collect()
    }
    /// Returns the suffix of the autosuggestion that comes after `prefix`.
    pub fn remaining_suggestion(&self, prefix: &str) -> Option<String> {
        let full = self.get_autosuggestion(prefix)?;
        full.strip_prefix(prefix).map(|s| s.to_string())
    }
    /// Returns the next word chunk of the autosuggestion beyond `prefix`.
    pub fn next_word_suggestion(&self, prefix: &str) -> Option<String> {
        let rem = self.remaining_suggestion(prefix)?;
        if rem.is_empty() {
            return None;
        }
        let leading_spaces = rem.chars().take_while(|c| c.is_whitespace()).count();
        let after_spaces = &rem[leading_spaces..];
        let word_len = after_spaces
            .chars()
            .take_while(|c| !c.is_whitespace())
            .count();
        let total_chars = leading_spaces + word_len;
        let mut char_indices = rem.char_indices();
        let byte_end = if let Some((idx, _)) = char_indices.nth(total_chars) {
            idx
        } else {
            rem.len()
        };
        Some(rem[..byte_end].to_string())
    }
    pub fn total_commands(&self) -> u64 {
        self.total_commands
    }
    pub fn unique_commands(&self) -> usize {
        self.entries.len()
    }
    pub fn default_path() -> io::Result<PathBuf> {
        if let Some(path) = std::env::var_os("WELL_HISTORY_FILE") {
            return Ok(PathBuf::from(path));
        }
        let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?;
        #[cfg(target_os = "macos")]
        let base = PathBuf::from(home).join("Library/Application Support/Well");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| PathBuf::from(home).join(".local/share"))
            .join("well");
        Ok(base.join("history"))
    }
    pub fn load_from_file(&mut self, path: &Path) -> io::Result<usize> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let reader = BufReader::new(file);
        let mut count = 0;
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                self.learn_command(trimmed);
                count += 1;
            }
        }
        Ok(count)
    }
    /// Ingests Fish shell history YAML formatted lines (`- cmd: <command>`).
    pub fn load_fish_history(&mut self, path: &Path) -> io::Result<usize> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let reader = BufReader::new(file);
        let mut count = 0;
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if let Some(cmd) = trimmed.strip_prefix("- cmd:") {
                let cmd = cmd.trim();
                let unescaped = cmd.replace("\\n", "\n");
                let first_line = unescaped.lines().next().unwrap_or("").trim();
                if !first_line.is_empty() {
                    self.learn_command(first_line);
                    count += 1;
                }
            }
        }
        Ok(count)
    }
    /// Ingests Zsh history lines (`: timestamp:duration;command` or plain lines).
    pub fn load_zsh_history(&mut self, path: &Path) -> io::Result<usize> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let reader = BufReader::new(file);
        let mut count = 0;
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let cmd = if trimmed.starts_with(':') {
                if let Some(semi_pos) = trimmed.find(';') {
                    trimmed[semi_pos + 1..].trim()
                } else {
                    trimmed
                }
            } else {
                trimmed
            };
            if !cmd.is_empty() {
                self.learn_command(cmd);
                count += 1;
            }
        }
        Ok(count)
    }
    /// Ingests Bash history lines (skipping `#timestamp` lines).
    pub fn load_bash_history(&mut self, path: &Path) -> io::Result<usize> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let reader = BufReader::new(file);
        let mut count = 0;
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            self.learn_command(trimmed);
            count += 1;
        }
        Ok(count)
    }
    /// Discovers and loads commands from Fish, Zsh, Bash, and Well histories.
    pub fn load_all_shell_histories(&mut self) -> usize {
        let mut total = 0;
        if let Ok(well_path) = Self::default_path() {
            if let Ok(c) = self.load_from_file(&well_path) {
                total += c;
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            let fish_path = home_path.join(".local/share/fish/fish_history");
            if let Ok(c) = self.load_fish_history(&fish_path) {
                total += c;
            }
            let zsh_path = home_path.join(".zsh_history");
            if let Ok(c) = self.load_zsh_history(&zsh_path) {
                total += c;
            }
            let bash_path = home_path.join(".bash_history");
            if let Ok(c) = self.load_bash_history(&bash_path) {
                total += c;
            }
        }
        total
    }
    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut temp = path.to_path_buf();
        temp.set_extension("tmp");
        {
            let mut file = File::create(&temp)?;
            for entry in &self.entries {
                writeln!(file, "{}", entry.command)?;
            }
            file.sync_all()?;
        }
        fs::rename(&temp, path)?;
        Ok(())
    }
    pub fn append_to_file(path: &Path, command: &str) -> io::Result<()> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", trimmed)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn add_recipe(&mut self, recipe: MetisRecipe) {
        self.learn_command(&recipe.command);
        self.recipes
            .retain(|r| r.prompt != recipe.prompt || r.command != recipe.command);
        self.recipes.push(recipe);
    }

    pub fn sync_recipes_json(&mut self, json_str: &str) -> Result<usize, serde_json::Error> {
        let recipes: Vec<MetisRecipe> = serde_json::from_str(json_str)?;
        let count = recipes.len();
        for r in recipes {
            self.add_recipe(r);
        }
        Ok(count)
    }

    pub fn search_recipes(&self, query: &str, limit: usize) -> Vec<MetisRecipe> {
        if query.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        let q = query.to_lowercase();
        let query_tokens: Vec<&str> = q.split_whitespace().collect();

        let mut scored: Vec<(f32, &MetisRecipe)> = self
            .recipes
            .iter()
            .filter_map(|r| {
                let prompt_lower = r.prompt.to_lowercase();
                let cmd_lower = r.command.to_lowercase();

                let mut score = 0.0f32;
                if prompt_lower == q {
                    score += 10.0;
                } else if prompt_lower.starts_with(&q) {
                    score += 5.0;
                } else if prompt_lower.contains(&q) {
                    score += 3.0;
                }

                if cmd_lower.contains(&q) {
                    score += 2.0;
                }

                let mut matched_tokens = 0;
                for token in &query_tokens {
                    if prompt_lower.contains(token)
                        || cmd_lower.contains(token)
                        || r.tags.iter().any(|t| t.to_lowercase().contains(token))
                    {
                        matched_tokens += 1;
                    }
                }

                if matched_tokens > 0 {
                    score += (matched_tokens as f32 / query_tokens.len() as f32) * 4.0;
                }

                if score > 0.0 {
                    let final_score = score * (1.0 + r.confidence);
                    Some((final_score, r))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, r)| r.clone())
            .collect()
    }

    pub fn recipes(&self) -> &[MetisRecipe] {
        &self.recipes
    }

    pub fn save_recipes_to_file(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.recipes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load_recipes_from_file(&mut self, path: &Path) -> io::Result<usize> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        self.sync_recipes_json(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
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
    #[test]
    fn test_history_file_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history");
        let mut h = MetisHistory::new();
        h.learn_command("cargo check");
        h.learn_command("cargo test --workspace");
        h.save_to_file(&path).unwrap();

        MetisHistory::append_to_file(&path, "cargo run --release").unwrap();

        let mut loaded = MetisHistory::new();
        let count = loaded.load_from_file(&path).unwrap();
        assert_eq!(count, 3);
        assert_eq!(
            loaded.get_autosuggestion("cargo r").as_deref(),
            Some("cargo run --release")
        );
        assert_eq!(
            loaded.get_autosuggestion("cargo t").as_deref(),
            Some("cargo test --workspace")
        );
    }

    #[test]
    fn test_word_and_remaining_suggestions() {
        let mut h = MetisHistory::new();
        h.learn_command("cargo test --workspace --all-targets");

        assert_eq!(
            h.remaining_suggestion("cargo "),
            Some("test --workspace --all-targets".to_string())
        );
        assert_eq!(h.next_word_suggestion("cargo "), Some("test".to_string()));
        assert_eq!(
            h.next_word_suggestion("cargo test "),
            Some("--workspace".to_string())
        );
        assert_eq!(h.remaining_suggestion("nonexistent"), None);
        assert_eq!(h.next_word_suggestion("nonexistent"), None);
    }

    #[test]
    fn test_fish_and_zsh_history_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let fish_file = dir.path().join("fish_history");
        std::fs::write(
            &fish_file,
            "- cmd: git checkout -b feature/metis\n  when: 1600000000\n- cmd: cargo build --release\n  when: 1600000010\n",
        ).unwrap();

        let zsh_file = dir.path().join("zsh_history");
        std::fs::write(
            &zsh_file,
            ": 1600000000:0;echo 'hello from zsh'\ngit status\n",
        )
        .unwrap();

        let mut h = MetisHistory::new();
        let fish_count = h.load_fish_history(&fish_file).unwrap();
        assert_eq!(fish_count, 2);
        let zsh_count = h.load_zsh_history(&zsh_file).unwrap();
        assert_eq!(zsh_count, 2);

        assert_eq!(
            h.get_autosuggestion("git ch").as_deref(),
            Some("git checkout -b feature/metis")
        );
        assert_eq!(
            h.get_autosuggestion("echo ").as_deref(),
            Some("echo 'hello from zsh'")
        );
    }

    #[test]
    fn test_metis_recipe_sync_and_search() {
        let mut h = MetisHistory::new();
        let payload = r#"[
            {
                "prompt": "build release binaries",
                "command": "cargo build --release --workspace",
                "cluster_id": "cluster-rust-build",
                "confidence": 0.98,
                "tags": ["rust", "build", "release"]
            },
            {
                "prompt": "run tests with backtrace",
                "command": "RUST_BACKTRACE=1 cargo test",
                "cluster_id": "cluster-rust-test",
                "confidence": 0.92,
                "tags": ["rust", "test", "debug"]
            },
            {
                "prompt": "list kubernetes pods across all namespaces",
                "command": "kubectl get pods -A -o wide",
                "cluster_id": "cluster-k8s-diagnostics",
                "confidence": 0.95,
                "tags": ["k8s", "kubectl", "pods"]
            }
        ]"#;

        let count = h
            .sync_recipes_json(payload)
            .expect("Valid JSON recipe payload");
        assert_eq!(count, 3);
        assert_eq!(h.recipes().len(), 3);

        // Learning recipes into trie automatically provides autosuggestion!
        assert_eq!(
            h.get_autosuggestion("cargo b").as_deref(),
            Some("cargo build --release --workspace")
        );

        // Sub-2ms recipe search with exact prompt
        let exact = h.search_recipes("build release binaries", 5);
        assert!(!exact.is_empty());
        assert_eq!(exact[0].command, "cargo build --release --workspace");

        // Token-based fuzzy search
        let fuzzy = h.search_recipes("kubernetes pods", 5);
        assert!(!fuzzy.is_empty());
        assert_eq!(fuzzy[0].command, "kubectl get pods -A -o wide");

        // Tag matching
        let tag_match = h.search_recipes("debug", 5);
        assert!(!tag_match.is_empty());
        assert_eq!(tag_match[0].command, "RUST_BACKTRACE=1 cargo test");
    }

    #[test]
    fn test_metis_recipe_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("recipes.json");

        let mut h = MetisHistory::new();
        h.add_recipe(MetisRecipe {
            prompt: "git status porcelain".to_string(),
            command: "git status --porcelain=v2".to_string(),
            cluster_id: "git-status".to_string(),
            confidence: 0.99,
            tags: vec!["git".into(), "status".into()],
        });

        h.save_recipes_to_file(&file_path).expect("Save recipes");
        assert!(file_path.exists());

        let mut h2 = MetisHistory::new();
        let loaded = h2.load_recipes_from_file(&file_path).expect("Load recipes");
        assert_eq!(loaded, 1);
        assert_eq!(h2.recipes().len(), 1);
        assert_eq!(h2.recipes()[0].command, "git status --porcelain=v2");
    }
}
