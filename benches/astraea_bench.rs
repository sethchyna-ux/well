use criterion::{black_box, criterion_group, criterion_main, Criterion};
use well_prompt::{AstraeaCompiler, AstraeaPreset, AstraeaSnapshot};

fn bench_astraea_compilation(c: &mut Criterion) {
    let snapshot = AstraeaSnapshot {
        cwd: "/workspace/well/crates/well-prompt".to_string(),
        git_branch: "main".to_string(),
        is_dirty: true,
        in_container: false,
        is_ssh: false,
        execution_time_ms: 42,
        ..Default::default()
    };

    let presets = [
        ("starship", AstraeaPreset::Starship),
        ("minimal", AstraeaPreset::Minimal),
        ("powerline", AstraeaPreset::Powerline),
        ("tokyo_night", AstraeaPreset::TokyoNight),
        ("pastel", AstraeaPreset::Pastel),
        ("pure", AstraeaPreset::Pure),
    ];

    for (name, preset) in presets {
        let mut snap = snapshot.clone();
        snap.preset = preset;
        let compiler = AstraeaCompiler::with_snapshot(snap);

        c.bench_function(&format!("astraea_compile_{}", name), |b| {
            b.iter(|| {
                let prompt = compiler.compile_prompt();
                black_box(prompt);
            });
        });
    }
}

criterion_group!(benches, bench_astraea_compilation);
criterion_main!(benches);
