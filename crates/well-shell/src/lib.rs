// Metis – Fish core shell & prefix trie logic
use anyhow::Result;
use dashmap::DashMap;
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task;

/// Simple prefix‑trie node using a DashMap for lock‑free concurrent reads/writes.
#[derive(Debug, Default, Clone)]
struct TrieNode {
    children: DashMap<char, Arc<TrieNode>>, // child character → node
    is_word: bool,
}

impl TrieNode {
    fn insert(&self, word: &str) {
        let mut node = Arc::new(self.clone());
        for ch in word.chars() {
            let child = node
                .children
                .entry(ch)
                .or_insert_with(|| Arc::new(TrieNode::default()))
                .clone();
            node = child;
        }
        // Mark terminal node as complete command.
        if let Some(mut_ref) = Arc::get_mut(&mut node) {
            mut_ref.is_word = true;
        }
    }

    /// Return up to `limit` suggestions for a given prefix.
    fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String> {
        let mut node = Arc::new(self.clone());
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(child) => node = child.clone(),
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        Self::collect(node, prefix.to_string(), &mut results, limit);
        results
    }

    fn collect(node: Arc<TrieNode>, current: String, out: &mut Vec<String>, limit: usize) {
        if out.len() >= limit {
            return;
        }
        if node.is_word {
            out.push(current.clone());
        }
        for entry in node.children.iter() {
            let mut next = current.clone();
            next.push(*entry.key());
            Self::collect(entry.value().clone(), next, out, limit);
            if out.len() >= limit {
                break;
            }
        }
    }
}

/// Public handle for interacting with the shell subsystem.
#[derive(Debug, Clone)]
pub struct ShellHandle {
    suggestion_trie: Arc<TrieNode>,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl ShellHandle {
    /// Send a command to the background shell task.
    pub async fn send_command(&self, cmd: String) -> Result<()> {
        self.tx.send(cmd).await.map_err(|e| anyhow::anyhow!(e))
    }

    /// Query suggestion completions for a prefix.
    pub fn suggest(&self, prefix: &str) -> Vec<String> {
        self.suggestion_trie.suggestions(prefix, 5)
    }
}

/// Initialise the shell subsystem (Metis MVP).
///
/// Spawns an async task that currently just logs commands and echoes a response.
pub fn init_shell() -> ShellHandle {
    info!("Initializing Well Shell (Metis) MVP");
    let (tx, mut rx_cmd) = mpsc::channel::<String>(32);
    let (tx_out, rx) = mpsc::channel::<String>(32);
    let trie = Arc::new(TrieNode::default());
    let trie_clone = trie.clone();

    task::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            info!("[shell core] received command: {}", cmd);
            if let Err(e) = tx_out.send(format!("Executed: {}", cmd)).await {
                warn!("failed to send shell output: {}", e);
            }
            trie_clone.insert(&cmd);
        }
        info!("Shell core task exiting");
    });

    ShellHandle { suggestion_trie: trie, tx, rx }
}
