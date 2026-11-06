use criterion::{black_box, criterion_group, criterion_main, Criterion};
use well_shell::history::MetisHistory;

fn bench_metis_history_trie(c: &mut Criterion) {
    let mut history = MetisHistory::new();
    // Seed trie with common commands and patterns
    let commands = [
        "git status",
        "git commit -m 'feat: update prompt compiler'",
        "git push origin main",
        "git pull --rebase",
        "git log --oneline -n 20",
        "git diff HEAD~1",
        "cargo build --release",
        "cargo check --workspace",
        "cargo test --workspace",
        "cargo run -p well-gpui",
        "cargo clippy --workspace",
        "kubectl get pods -A",
        "kubectl logs -f deployment/well-daemon",
        "docker ps -a",
        "docker compose up -d",
        "ssh chyna@192.168.1.141",
        "curl -sSL https://get.well.sh | sh",
        "find . -name '*.rs' -type f",
        "grep -rn 'Seqlock' crates/",
        "htop",
    ];

    for _ in 0..100 {
        for cmd in &commands {
            history.learn_command(cmd);
        }
    }

    c.bench_function("metis_trie_insert", |b| {
        let mut h = history.clone();
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            h.learn_command(&format!("custom_command_{}", counter));
        });
    });

    c.bench_function("metis_trie_autosuggestion_git", |b| {
        b.iter(|| {
            let res = history.get_autosuggestion(black_box("git c"));
            black_box(res);
        });
    });

    c.bench_function("metis_trie_suggestions_cargo", |b| {
        b.iter(|| {
            let res = history.suggestions(black_box("cargo "), 5);
            black_box(res);
        });
    });
}

criterion_group!(benches, bench_metis_history_trie);
criterion_main!(benches);

