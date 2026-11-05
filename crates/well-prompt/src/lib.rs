//! Astraea State-Vector Prompt Engine
//!
//! Features:
//! - Kernel-mediated, lock-free memory-mapped state vectors (`AstraeaStateVector`)
//! - Zero-subprocess background telemetry collector (`AstraeaWatcher`)
//! - Sub-100 microsecond prompt compiler (`AstraeaCompiler`) with 6 Starship styling presets
//! - Strict prompt transience for clean scrollback history and zero GPU memory waste
//! - Native shell integration hooks for Zsh, Fish, and Bash

pub mod compiler;
pub mod state_vector;
pub mod watcher;

pub use compiler::{AstraeaCompiler, AstraeaConfig, AstraeaShellIntegration};
pub use state_vector::{
    AstraeaPreset, AstraeaRawVector, AstraeaSnapshot, AstraeaStateVector, ScmOperation,
    ASTRAEA_MAGIC,
};
pub use watcher::AstraeaWatcher;

use std::path::Path;

/// Backward-compatible lightweight prompt state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptState {
    pub cwd: String,
    pub git_branch: String,
    pub is_dirty: bool,
    pub exit_code: i32,
    pub execution_time_ms: u64,
}

impl PromptState {
    pub fn from_path(path: &Path, exit_code: i32, execution_time_ms: u64) -> Self {
        let snap = AstraeaWatcher::scan_path(path, exit_code, execution_time_ms);
        Self {
            cwd: snap.cwd,
            git_branch: snap.git_branch,
            is_dirty: snap.is_dirty,
            exit_code: snap.exit_code,
            execution_time_ms: snap.execution_time_ms,
        }
    }
}

impl From<AstraeaSnapshot> for PromptState {
    fn from(s: AstraeaSnapshot) -> Self {
        Self {
            cwd: s.cwd,
            git_branch: s.git_branch,
            is_dirty: s.is_dirty,
            exit_code: s.exit_code,
            execution_time_ms: s.execution_time_ms,
        }
    }
}

impl From<PromptState> for AstraeaSnapshot {
    fn from(p: PromptState) -> Self {
        Self {
            cwd: p.cwd,
            git_branch: p.git_branch,
            is_dirty: p.is_dirty,
            exit_code: p.exit_code,
            execution_time_ms: p.execution_time_ms,
            ..Default::default()
        }
    }
}

impl AstraeaCompiler {
    pub fn with_state(state: PromptState) -> Self {
        Self::with_snapshot(state.into())
    }

    pub fn update_state(&mut self, state: PromptState) {
        self.update_snapshot(state.into());
    }

    pub fn state(&self) -> PromptState {
        self.snapshot().clone().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn filesystem_and_branch_names_cannot_inject_terminal_controls() {
        let mut compiler = AstraeaCompiler::new();
        compiler.update_state(PromptState {
            cwd: "/tmp/\x1b[2J\nname".into(),
            git_branch: "branch\x07".into(),
            exit_code: 1,
            ..Default::default()
        });
        let prompt = compiler.compile_prompt();
        assert!(!prompt.contains("\x1b[2J"));
        assert!(!prompt.contains('\x07'));
        assert_eq!(prompt.chars().filter(|&ch| ch == '\n').count(), 1);
        assert!(prompt.contains("\\u{1b}"));
        assert!(prompt.contains("\x1b[31m"));

        let transient = compiler.compile_transient_prompt();
        assert!(transient.contains("\x1b[31m❯\x1b[0m"));
    }

    #[test]
    fn test_git_prompt_acquisition_with_merging() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let dot_git = repo.join(".git");
        fs::create_dir_all(&dot_git).unwrap();
        fs::write(dot_git.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
        fs::write(dot_git.join("MERGE_HEAD"), "1234567890abcdef\n").unwrap();

        let snap = AstraeaWatcher::scan_path(repo, 0, 150);
        assert_eq!(snap.git_branch, "feature");
        assert_eq!(snap.scm_op, ScmOperation::Merging);
        assert_eq!(snap.exit_code, 0);

        let compiler = AstraeaCompiler::with_snapshot(snap);
        let prompt = compiler.compile_prompt();
        assert!(prompt.contains(" feature"));
        assert!(prompt.contains("MERGING"));
    }

    #[test]
    fn test_seqlock_state_vector_roundtrip_and_mmap() {
        let dir = tempfile::tempdir().unwrap();
        let mmap_path = dir.path().join("astraea.vec");

        let vector = AstraeaStateVector::open_or_create_mmap(&mmap_path).unwrap();
        let snapshot = AstraeaSnapshot {
            cwd: "~/Desktop/well".into(),
            git_branch: "feature/astraea".into(),
            is_dirty: true,
            git_ahead: 2,
            git_behind: 1,
            scm_op: ScmOperation::Rebasing,
            in_container: true,
            is_ssh: true,
            rust_version: Some("🦀 2024".into()),
            node_version: Some(" v22".into()),
            python_version: Some("🐍 3.12".into()),
            go_version: Some("🐹 1.22".into()),
            zig_version: Some("⚡ 0.13.0".into()),
            cloud_context: Some("☁ aws:production".into()),
            preset: AstraeaPreset::TokyoNight,
            exit_code: 0,
            execution_time_ms: 850,
            battery_pct: Some(92),
            battery_charging: true,
            cpu_usage_pct: 15,
            memory_used_mb: 64,
            ..Default::default()
        };

        vector.write_snapshot(&snapshot);
        let read = vector.read_snapshot();
        assert_eq!(read.cwd, "~/Desktop/well");
        assert_eq!(read.git_branch, "feature/astraea");
        assert!(read.is_dirty);
        assert_eq!(read.git_ahead, 2);
        assert_eq!(read.git_behind, 1);
        assert_eq!(read.scm_op, ScmOperation::Rebasing);
        assert!(read.in_container);
        assert!(read.is_ssh);
        assert_eq!(read.rust_version.as_deref(), Some("🦀 2024"));
        assert_eq!(read.node_version.as_deref(), Some(" v22"));
        assert_eq!(read.python_version.as_deref(), Some("🐍 3.12"));
        assert_eq!(read.go_version.as_deref(), Some("🐹 1.22"));
        assert_eq!(read.zig_version.as_deref(), Some("⚡ 0.13.0"));
        assert_eq!(read.cloud_context.as_deref(), Some("☁ aws:production"));
        assert_eq!(read.preset, AstraeaPreset::TokyoNight);
        assert_eq!(read.battery_pct, Some(92));
        assert!(read.battery_charging);
    }

    #[test]
    fn test_all_presets_render_valid_prompts() {
        let snapshot = AstraeaSnapshot {
            cwd: "~/Desktop/well".into(),
            git_branch: "main".into(),
            rust_version: Some("🦀 2024".into()),
            ..Default::default()
        };

        let presets = [
            AstraeaPreset::Starship,
            AstraeaPreset::Minimal,
            AstraeaPreset::Powerline,
            AstraeaPreset::TokyoNight,
            AstraeaPreset::Pastel,
            AstraeaPreset::Pure,
            AstraeaPreset::BracketedSegments,
        ];

        for preset in presets {
            let mut snap = snapshot.clone();
            snap.preset = preset;
            let compiler = AstraeaCompiler::with_snapshot(snap);
            let prompt = compiler.compile_prompt();
            assert!(
                !prompt.is_empty(),
                "Preset {:?} rendered empty prompt",
                preset
            );
            assert!(prompt.contains("~/Desktop/well"));
        }
    }

    #[test]
    fn test_compile_time_strictly_below_100_microseconds() {
        let state_vector = AstraeaStateVector::in_memory();
        let snapshot = AstraeaSnapshot {
            cwd: "~/Desktop/well".into(),
            git_branch: "main".into(),
            is_dirty: true,
            git_ahead: 1,
            git_behind: 0,
            scm_op: ScmOperation::None,
            rust_version: Some("🦀 2024".into()),
            node_version: Some(" 22".into()),
            python_version: Some("🐍 3.12".into()),
            go_version: Some("🐹 1.22".into()),
            zig_version: Some("⚡ 0.13".into()),
            cloud_context: Some("☁ aws:prod".into()),
            preset: AstraeaPreset::Starship,
            exit_code: 0,
            execution_time_ms: 1200,
            battery_pct: Some(85),
            battery_charging: true,
            ..Default::default()
        };
        state_vector.write_snapshot(&snapshot);

        // Warmup
        let _ = AstraeaCompiler::from_vector(&state_vector).compile_prompt();

        let start = std::time::Instant::now();
        let iterations = 1000;
        for _ in 0..iterations {
            let compiler = AstraeaCompiler::from_vector(&state_vector);
            let prompt = compiler.compile_prompt();
            assert!(!prompt.is_empty());
        }
        let elapsed = start.elapsed();
        let per_compile = elapsed / iterations;
        println!("Astraea prompt compile time per call: {per_compile:?}");
        assert!(
            per_compile < std::time::Duration::from_micros(100),
            "Target is < 100µs, achieved {:?}",
            per_compile
        );
    }

    #[test]
    fn test_prompt_transience_and_escape_sequence() {
        let snapshot = AstraeaSnapshot {
            cwd: "~/Desktop/well".into(),
            git_branch: "main".into(),
            exit_code: 0,
            ..Default::default()
        };
        let compiler = AstraeaCompiler::with_snapshot(snapshot);
        let full = compiler.compile_prompt();
        let transient = compiler.compile_transient_prompt();

        assert!(full.contains('\n'));
        assert!(!transient.contains('\n'));
        assert_eq!(transient, "\x1b[32m❯\x1b[0m ");

        let esc = compiler.transient_collapse_escape_sequence(1, "cargo check");
        assert!(esc.starts_with("\x1b[1A\r\x1b[0J"));
        assert!(esc.contains("cargo check\n"));
    }

    #[test]
    fn test_shell_integration_snippets() {
        assert!(AstraeaShellIntegration::zsh().contains("add-zsh-hook"));
        assert!(AstraeaShellIntegration::fish().contains("fish_prompt"));
        assert!(AstraeaShellIntegration::bash().contains("PROMPT_COMMAND"));
    }
}
