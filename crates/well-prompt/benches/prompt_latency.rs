//! Micro-benchmark suite for Astraea <100µs prompt compilation across all presets.

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use well_prompt::{
    AstraeaCompiler, AstraeaPreset, AstraeaSnapshot, AstraeaStateVector, ScmOperation,
};

fn main() {
    let presets = [
        ("starship", AstraeaPreset::Starship),
        ("minimal", AstraeaPreset::Minimal),
        ("powerline", AstraeaPreset::Powerline),
        ("tokyo_night", AstraeaPreset::TokyoNight),
        ("pastel", AstraeaPreset::Pastel),
        ("pure", AstraeaPreset::Pure),
        ("bracketed_segments", AstraeaPreset::BracketedSegments),
    ];

    let state_vector = AstraeaStateVector::in_memory();
    let iterations: u32 = 10_000;
    let mut report_lines = Vec::new();
    report_lines.push("=== Astraea Prompt Compilation Latency Benchmark ===".to_string());

    for (name, preset) in presets {
        let snapshot = AstraeaSnapshot {
            cwd: "~/Desktop/well/crates/well-prompt".into(),
            git_branch: "feature/phase2-benchmarks".into(),
            is_dirty: true,
            git_ahead: 3,
            git_behind: 1,
            scm_op: ScmOperation::None,
            rust_version: Some("🦀 1.85.0".into()),
            node_version: Some("v22.4.0".into()),
            python_version: Some("🐍 3.12".into()),
            go_version: Some("🐹 1.23".into()),
            zig_version: Some("⚡ 0.17.0".into()),
            cloud_context: Some("☁ gcp:well-prod".into()),
            preset,
            exit_code: 0,
            execution_time_ms: 420,
            battery_pct: Some(94),
            battery_charging: true,
            cpu_usage_pct: 12,
            memory_used_mb: 58,
            ..Default::default()
        };
        state_vector.write_snapshot(&snapshot);

        // Warmup
        for _ in 0..256 {
            let compiler = AstraeaCompiler::from_vector(&state_vector);
            black_box(compiler.compile_prompt());
        }

        let start = Instant::now();
        for _ in 0..iterations {
            let compiler = AstraeaCompiler::from_vector(&state_vector);
            let out = compiler.compile_prompt();
            black_box(out);
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        let line = format!(
            "preset={name:<20} iterations={iterations} mean_latency={per_iter:?} (<100µs SLA: {})",
            per_iter < Duration::from_micros(100)
        );
        println!("{line}");
        assert!(
            per_iter < Duration::from_micros(100),
            "Preset {name} exceeded 100µs SLA: {per_iter:?}"
        );
        report_lines.push(line);
    }

    let criterion_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("target/criterion/prompt_latency"))
        .unwrap_or_else(|| PathBuf::from("target/criterion/prompt_latency"));
    let _ = fs::create_dir_all(&criterion_dir);
    let _ = fs::write(
        criterion_dir.join("summary.txt"),
        report_lines.join("\n") + "\n",
    );
}
