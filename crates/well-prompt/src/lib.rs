//! well-prompt: Astraea Memory-Mapped State Vector & Sub-100µs Prompt Compiler
//!
//! Subsystems:
//! - AstraeaStateVector: Atomic state vector tracking git branch, status, runtime metrics.
//! - PromptCompiler: Instantaneous zero-fork prompt builder supporting transient collapsing.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum GitDirtyConfidence {
    #[default]
    Unknown,
    LockFileOnly,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum GitOperation {
    #[default]
    None,
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

impl GitOperation {
    fn label(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Merge => Some("merge"),
            Self::Rebase => Some("rebase"),
            Self::CherryPick => Some("cherry-pick"),
            Self::Revert => Some("revert"),
            Self::Bisect => Some("bisect"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitMetrics {
    #[serde(default)]
    pub repo_root: Option<PathBuf>,
    pub branch: String,
    #[serde(default)]
    pub head_oid: Option<String>,
    #[serde(default)]
    pub detached: bool,
    pub is_dirty: bool,
    #[serde(default)]
    pub dirty_confidence: GitDirtyConfidence,
    #[serde(default)]
    pub operation: GitOperation,
    pub ahead: u32,
    pub behind: u32,
}

impl Default for GitMetrics {
    fn default() -> Self {
        Self {
            repo_root: None,
            branch: "main".to_string(),
            head_oid: None,
            detached: false,
            is_dirty: false,
            dirty_confidence: GitDirtyConfidence::Unknown,
            operation: GitOperation::None,
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

    pub fn refresh_from_cwd(&self, cwd: impl AsRef<Path>) -> Result<(), String> {
        let cwd = cwd.as_ref();
        let Some(repo) = discover_git_repository(cwd) else {
            self.set_git(GitMetrics {
                repo_root: None,
                branch: String::new(),
                head_oid: None,
                detached: false,
                is_dirty: false,
                dirty_confidence: GitDirtyConfidence::Unknown,
                operation: GitOperation::None,
                ahead: 0,
                behind: 0,
            });
            return Ok(());
        };

        self.set_git(read_git_metrics(&repo)?);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct GitRepository {
    git_dir: PathBuf,
    worktree_root: PathBuf,
}

fn discover_git_repository(start: &Path) -> Option<GitRepository> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = current.join(".git");
        if candidate.is_dir() {
            return Some(GitRepository {
                git_dir: candidate,
                worktree_root: current,
            });
        }
        if candidate.is_file() {
            if let Ok(data) = std::fs::read_to_string(&candidate) {
                if let Some(path) = data.trim().strip_prefix("gitdir:") {
                    let path = PathBuf::from(path.trim());
                    let git_dir = if path.is_absolute() {
                        path
                    } else {
                        current.join(path)
                    };
                    return Some(GitRepository {
                        git_dir,
                        worktree_root: current,
                    });
                }
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

fn read_git_metrics(repo: &GitRepository) -> Result<GitMetrics, String> {
    let git_dir = &repo.git_dir;
    let common_dir = common_git_dir(git_dir);
    let head = std::fs::read_to_string(git_dir.join("HEAD"))
        .map_err(|error| format!("Failed to read {}: {error}", git_dir.join("HEAD").display()))?;
    let head = head.trim();
    let (branch, detached, head_oid) = if let Some(ref_name) = head.strip_prefix("ref: ") {
        let branch = ref_name
            .strip_prefix("refs/heads/")
            .unwrap_or(ref_name)
            .to_string();
        let head_oid = resolve_ref(git_dir, &common_dir, ref_name);
        (branch, false, head_oid)
    } else {
        let short = head.chars().take(12).collect::<String>();
        (
            short,
            true,
            if head.is_empty() {
                None
            } else {
                Some(head.to_string())
            },
        )
    };

    let operation = detect_operation(git_dir, &common_dir);
    let has_index_lock =
        git_dir.join("index.lock").exists() || common_dir.join("index.lock").exists();

    Ok(GitMetrics {
        repo_root: Some(repo.worktree_root.clone()),
        branch,
        head_oid,
        detached,
        is_dirty: has_index_lock,
        dirty_confidence: GitDirtyConfidence::LockFileOnly,
        operation,
        ahead: 0,
        behind: 0,
    })
}

fn common_git_dir(git_dir: &Path) -> PathBuf {
    let common_dir_file = git_dir.join("commondir");
    let Ok(data) = std::fs::read_to_string(&common_dir_file) else {
        return git_dir.to_path_buf();
    };
    let path = PathBuf::from(data.trim());
    if path.is_absolute() {
        path
    } else {
        git_dir.join(path)
    }
}

fn resolve_ref(git_dir: &Path, common_dir: &Path, ref_name: &str) -> Option<String> {
    for root in [git_dir, common_dir] {
        let ref_path = root.join(ref_name);
        if let Ok(oid) = std::fs::read_to_string(ref_path) {
            let oid = oid.trim();
            if !oid.is_empty() {
                return Some(oid.to_string());
            }
        }
    }

    for root in [git_dir, common_dir] {
        if let Some(oid) = resolve_packed_ref(&root.join("packed-refs"), ref_name) {
            return Some(oid);
        }
    }

    None
}

fn resolve_packed_ref(packed_refs: &Path, ref_name: &str) -> Option<String> {
    let data = std::fs::read_to_string(packed_refs).ok()?;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let oid = parts.next()?;
        let packed_ref = parts.next()?;
        if packed_ref == ref_name {
            return Some(oid.to_string());
        }
    }
    None
}

fn detect_operation(git_dir: &Path, common_dir: &Path) -> GitOperation {
    for root in [git_dir, common_dir] {
        if root.join("rebase-merge").exists() || root.join("rebase-apply").exists() {
            return GitOperation::Rebase;
        }
        if root.join("MERGE_HEAD").exists() {
            return GitOperation::Merge;
        }
        if root.join("CHERRY_PICK_HEAD").exists() {
            return GitOperation::CherryPick;
        }
        if root.join("REVERT_HEAD").exists() {
            return GitOperation::Revert;
        }
        if root.join("BISECT_LOG").exists() {
            return GitOperation::Bisect;
        }
    }
    GitOperation::None
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
        let status_color = if exit_code == 0 {
            "\x1b[32m"
        } else {
            "\x1b[31m"
        };

        let git_part = if let Ok(g) = self.state.git.read() {
            if !g.branch.is_empty() {
                let dirty = if g.is_dirty { "*" } else { "" };
                let detached = if g.detached { " detached" } else { "" };
                let operation = g
                    .operation
                    .label()
                    .map(|label| format!(" {label}"))
                    .unwrap_or_default();
                format!(
                    " \x1b[35mon\x1b[0m \x1b[34m\u{e0a0} {}{}{}{}\x1b[0m",
                    g.branch, dirty, detached, operation
                )
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

    fn temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "well-prompt-{name}-{}-{}",
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn test_astraea_sub_100us_compile_time() {
        let state = Arc::new(AstraeaStateVector::new());
        state.set_git(GitMetrics {
            repo_root: None,
            branch: "feature/well-core".to_string(),
            head_oid: None,
            detached: false,
            is_dirty: true,
            dirty_confidence: GitDirtyConfidence::LockFileOnly,
            operation: GitOperation::None,
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

    #[test]
    fn refresh_from_cwd_reads_git_branch_without_spawning_git() {
        let temp_dir = temp_path("branch");
        let repo_dir = temp_dir.join("repo");
        let git_dir = repo_dir.join(".git");
        std::fs::create_dir_all(&git_dir).expect("git dir should be created");
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature/astraea\n")
            .expect("HEAD should be written");
        std::fs::create_dir_all(git_dir.join("refs/heads/feature"))
            .expect("refs should be created");
        std::fs::write(
            git_dir.join("refs/heads/feature/astraea"),
            "0123456789abcdef0123456789abcdef01234567\n",
        )
        .expect("branch ref should be written");

        let state = AstraeaStateVector::new();
        state
            .refresh_from_cwd(&repo_dir)
            .expect("git metrics should refresh");
        let git = state.git.read().expect("git metrics should be readable");
        assert_eq!(git.branch, "feature/astraea");
        assert_eq!(
            git.head_oid.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert_eq!(git.repo_root.as_deref(), Some(repo_dir.as_path()));
        assert!(!git.is_dirty);
        assert_eq!(git.dirty_confidence, GitDirtyConfidence::LockFileOnly);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn refresh_from_cwd_resolves_packed_branch_refs() {
        let temp_dir = temp_path("packed");
        let repo_dir = temp_dir.join("repo");
        let git_dir = repo_dir.join(".git");
        std::fs::create_dir_all(&git_dir).expect("git dir should be created");
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/packed/topic\n")
            .expect("HEAD should be written");
        std::fs::write(
            git_dir.join("packed-refs"),
            "# pack-refs with: peeled fully-peeled sorted\nabcdefabcdefabcdefabcdefabcdefabcdefabcd refs/heads/packed/topic\n",
        )
        .expect("packed-refs should be written");

        let state = AstraeaStateVector::new();
        state
            .refresh_from_cwd(repo_dir.join("nested"))
            .expect("git metrics should refresh from nested path");
        let git = state.git.read().expect("git metrics should be readable");

        assert_eq!(git.branch, "packed/topic");
        assert_eq!(
            git.head_oid.as_deref(),
            Some("abcdefabcdefabcdefabcdefabcdefabcdefabcd")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn refresh_from_cwd_tracks_detached_head_and_operation() {
        let temp_dir = temp_path("detached");
        let repo_dir = temp_dir.join("repo");
        let git_dir = repo_dir.join(".git");
        std::fs::create_dir_all(&git_dir).expect("git dir should be created");
        std::fs::write(
            git_dir.join("HEAD"),
            "fedcba9876543210fedcba9876543210fedcba98\n",
        )
        .expect("HEAD should be written");
        std::fs::write(git_dir.join("MERGE_HEAD"), "0123456789abcdef\n")
            .expect("merge marker should be written");

        let state = AstraeaStateVector::new();
        state
            .refresh_from_cwd(&repo_dir)
            .expect("git metrics should refresh");
        let git = state.git.read().expect("git metrics should be readable");

        assert_eq!(git.branch, "fedcba987654");
        assert_eq!(
            git.head_oid.as_deref(),
            Some("fedcba9876543210fedcba9876543210fedcba98")
        );
        assert!(git.detached);
        assert_eq!(git.operation, GitOperation::Merge);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn refresh_from_cwd_resolves_linked_worktree_gitdir() {
        let temp_dir = temp_path("worktree");
        let main_git_dir = temp_dir.join("main/.git");
        let worktree_dir = temp_dir.join("linked");
        let linked_git_dir = main_git_dir.join("worktrees/linked");
        std::fs::create_dir_all(main_git_dir.join("refs/heads/worktree"))
            .expect("main refs should be created");
        std::fs::create_dir_all(&linked_git_dir).expect("linked git dir should be created");
        std::fs::create_dir_all(&worktree_dir).expect("worktree should be created");
        std::fs::write(
            worktree_dir.join(".git"),
            format!("gitdir: {}\n", linked_git_dir.display()),
        )
        .expect(".git file should be written");
        std::fs::write(linked_git_dir.join("commondir"), "../..\n")
            .expect("commondir should be written");
        std::fs::write(
            linked_git_dir.join("HEAD"),
            "ref: refs/heads/worktree/topic\n",
        )
        .expect("HEAD should be written");
        std::fs::write(
            main_git_dir.join("refs/heads/worktree/topic"),
            "1111111111111111111111111111111111111111\n",
        )
        .expect("common branch ref should be written");
        std::fs::create_dir_all(linked_git_dir.join("rebase-merge"))
            .expect("rebase marker should be written");

        let state = AstraeaStateVector::new();
        state
            .refresh_from_cwd(&worktree_dir)
            .expect("git metrics should refresh");
        let git = state.git.read().expect("git metrics should be readable");

        assert_eq!(git.branch, "worktree/topic");
        assert_eq!(
            git.head_oid.as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(git.operation, GitOperation::Rebase);
        assert_eq!(git.repo_root.as_deref(), Some(worktree_dir.as_path()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn compile_prompt_surfaces_detached_and_operation_state() {
        let state = Arc::new(AstraeaStateVector::new());
        state.set_git(GitMetrics {
            repo_root: None,
            branch: "fedcba987654".to_string(),
            head_oid: Some("fedcba9876543210fedcba9876543210fedcba98".to_string()),
            detached: true,
            is_dirty: false,
            dirty_confidence: GitDirtyConfidence::LockFileOnly,
            operation: GitOperation::CherryPick,
            ahead: 0,
            behind: 0,
        });

        let compiler = PromptCompiler::new(state);
        let prompt = compiler.compile_prompt("~/repo");

        assert!(prompt.contains("fedcba987654 detached cherry-pick"));
    }
}
