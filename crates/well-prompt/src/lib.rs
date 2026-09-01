//! well-prompt: Astraea Memory-Mapped State Vector & Sub-100µs Prompt Compiler
//!
//! Subsystems:
//! - AstraeaStateVector: Atomic state vector tracking git branch, status, runtime metrics.
//! - PromptCompiler: Instantaneous zero-fork prompt builder supporting transient collapsing.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitMetrics {
    pub branch: String,
    pub is_dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

impl Default for GitMetrics {
    fn default() -> Self {
        Self {
            branch: "main".to_string(),
            is_dirty: false,
            ahead: 0,
            behind: 0,
        }
    }
}

pub struct AstraeaStateVector {
    pub git: std::sync::RwLock<GitMetrics>,
    pub last_exit_code: AtomicU32,
    pub transient_enabled: AtomicBool,
    pub active_duration_ms: AtomicU32,
}

impl Default for AstraeaStateVector {
    fn default() -> Self {
        Self::new()
    }
}

impl AstraeaStateVector {
    pub fn new() -> Self {
        Self {
            git: std::sync::RwLock::new(GitMetrics::default()),
            last_exit_code: AtomicU32::new(0),
            transient_enabled: AtomicBool::new(true),
            active_duration_ms: AtomicU32::new(0),
        }
    }

    pub fn set_git(&self, git: GitMetrics) {
        if let Ok(mut g) = self.git.write() {
            *g = git;
        }
    }

    pub fn set_last_exit_code(&self, code: u32) {
        self.last_exit_code.store(code, Ordering::Release);
    }
}

pub struct PromptCompiler {
    state: Arc<AstraeaStateVector>,
}

impl PromptCompiler {
    pub fn new(state: Arc<AstraeaStateVector>) -> Self {
        Self { state }
    }

    /// Compiles prompt string in sub-100 microseconds without subprocess forks.
    pub fn compile_prompt(&self, cwd_display: &str) -> String {
        let exit_code = self.state.last_exit_code.load(Ordering::Acquire);
        let status_color = if exit_code == 0 { "\x1b[32m" } else { "\x1b[31m" };

        let git_part = if let Ok(g) = self.state.git.read() {
            if !g.branch.is_empty() {
                let dirty = if g.is_dirty { "*" } else { "" };
                format!(" \x1b[35mon\x1b[0m \x1b[34m\u{e0a0} {}{}\x1b[0m", g.branch, dirty)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        format!(
            "\x1b[36m{}\x1b[0m{}\n{}>\x1b[0m ",
            cwd_display, git_part, status_color
        )
    }

    /// Renders transient prompt symbol once command execution begins
    pub fn render_transient(&self) -> &'static str {
        "\x1b[32m❯\x1b[0m "
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_astraea_sub_100us_compile_time() {
        let state = Arc::new(AstraeaStateVector::new());
        state.set_git(GitMetrics {
            branch: "feature/well-core".to_string(),
            is_dirty: true,
            ahead: 1,
            behind: 0,
        });

        let compiler = PromptCompiler::new(state);
        
        let start = Instant::now();
        let prompt = compiler.compile_prompt("~/projects/well");
        let elapsed = start.elapsed();

        assert!(prompt.contains("feature/well-core*"));
        assert!(prompt.contains("~/projects/well"));
        // Ensure prompt compiles well under 100 microseconds (0.1ms = 100_000 ns)
        assert!(elapsed.as_nanos() < 100_000, "Elapsed was {:?}", elapsed);
    }
}
