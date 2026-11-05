//! Micro-benchmark suite for Metis O(k) Trie prefix lookup across 260,000 history commands.

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use well_shell::MetisHistory;

fn main() {
    let mut history = MetisHistory::new();
    let total_commands = 260_000usize;

    let populate_start = Instant::now();
    for i in 0..total_commands {
        let cmd = format!("cargo test --workspace --package well-bench-{i} -- --nocapture");
        history.learn_command(&cmd);
    }
    history.learn_command("git status --short --branch");
    let populate_elapsed = populate_start.elapsed();

    let queries = [
        "cargo test --workspace --package well-bench-25999",
        "cargo test --workspace",
        "git status",
        "nonexistent_prefix_xyz",
    ];

    let iterations: u32 = 50_000;
    let start = Instant::now();
    for i in 0..iterations {
        let q = queries[(i as usize) % queries.len()];
        black_box(history.get_autosuggestion(black_box(q)));
    }
    let elapsed = start.elapsed();
    let per_lookup = elapsed / iterations;

    let summary = format!(
        "=== Metis O(k) Prefix Trie Benchmark ===\npopulated_commands={total_commands} populate_time={populate_elapsed:?}\nlookup_iterations={iterations} mean_lookup_latency={per_lookup:?}\n"
    );
    print!("{summary}");
    assert!(
        per_lookup < Duration::from_micros(50),
        "Trie lookup exceeded 50µs target: {per_lookup:?}"
    );

    let criterion_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("target/criterion/trie_search"))
        .unwrap_or_else(|| PathBuf::from("target/criterion/trie_search"));
    let _ = fs::create_dir_all(&criterion_dir);
    let _ = fs::write(criterion_dir.join("summary.txt"), summary);
}

