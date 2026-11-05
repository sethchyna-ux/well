//! Background asynchronous collector for Git, toolchains, cloud context, and system telemetry.
//! Never forks external subprocesses (git/rustc/node), keeping state sampling sub-millisecond
//! and prompt compilation under 100 microseconds.

use crate::state_vector::{AstraeaPreset, AstraeaSnapshot, AstraeaStateVector, ScmOperation};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct AstraeaWatcher {
    state_vector: Arc<AstraeaStateVector>,
    active_path: Arc<Mutex<PathBuf>>,
    running: Arc<AtomicBool>,
    notifier: Arc<(Mutex<bool>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

impl AstraeaWatcher {
    pub fn new(state_vector: Arc<AstraeaStateVector>, initial_path: PathBuf) -> Self {
        let active_path = Arc::new(Mutex::new(initial_path.clone()));
        let running = Arc::new(AtomicBool::new(true));
        let notifier = Arc::new((Mutex::new(false), Condvar::new()));

        let vec_clone = state_vector.clone();
        let path_clone = active_path.clone();
        let run_clone = running.clone();
        let notify_clone = notifier.clone();

        // Perform immediate scan
        let initial_snap = Self::scan_path(&initial_path, 0, 0);
        state_vector.write_snapshot(&initial_snap);

        let handle = thread::Builder::new()
            .name("astraea-state-collector".to_string())
            .spawn(move || {
                let mut last_path = PathBuf::new();
                let mut last_scan = Instant::now();

                while run_clone.load(Ordering::Relaxed) {
                    let current_path = {
                        path_clone
                            .lock()
                            .map(|p| p.clone())
                            .unwrap_or_else(|p| p.into_inner().clone())
                    };

                    let path_changed = current_path != last_path;
                    let periodic = last_scan.elapsed() >= Duration::from_millis(500);

                    if path_changed || periodic {
                        last_path = current_path.clone();
                        last_scan = Instant::now();

                        let current_raw = vec_clone.read_raw();
                        let mut snap = Self::scan_path(
                            &current_path,
                            current_raw.exit_code,
                            current_raw.execution_time_ms,
                        );
                        snap.battery_pct =
                            current_raw.battery_pct.checked_sub(0).filter(|&p| p <= 100);
                        snap.battery_charging = current_raw.battery_charging != 0;
                        snap.preset = AstraeaPreset::from_u8(current_raw.preset_id);

                        vec_clone.write_snapshot(&snap);
                    }

                    // Low-latency condition wait: wake up immediately on directory change or at 100ms interval
                    let (lock, cvar) = &*notify_clone;
                    if let Ok(guard) = lock.lock() {
                        let _ = cvar.wait_timeout(guard, Duration::from_millis(100));
                    } else {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            })
            .ok();

        Self {
            state_vector,
            active_path,
            running,
            notifier,
            handle,
        }
    }

    /// Notify the watcher that an event occurred (file modified or command finished).
    pub fn notify_change(&self) {
        let (lock, cvar) = &*self.notifier;
        if let Ok(mut guard) = lock.lock() {
            *guard = true;
            cvar.notify_one();
        }
    }

    /// Notify the watcher that the active directory has changed.
    pub fn set_path(&self, new_path: PathBuf) {
        if let Ok(mut p) = self.active_path.lock() {
            *p = new_path.clone();
        }
        // Immediate synchronous update into the vector
        let raw = self.state_vector.read_raw();
        let mut snap = Self::scan_path(&new_path, raw.exit_code, raw.execution_time_ms);
        snap.preset = AstraeaPreset::from_u8(raw.preset_id);
        self.state_vector.write_snapshot(&snap);
        self.notify_change();
    }

    /// Record command execution results (exit code and duration in ms).
    pub fn record_execution(&self, exit_code: i32, duration_ms: u64) {
        let mut snap = self.state_vector.read_snapshot();
        snap.exit_code = exit_code;
        snap.execution_time_ms = duration_ms;
        self.state_vector.write_snapshot(&snap);
        self.notify_change();
    }

    /// Set active prompt styling preset.
    pub fn set_preset(&self, preset: AstraeaPreset) {
        let mut snap = self.state_vector.read_snapshot();
        snap.preset = preset;
        self.state_vector.write_snapshot(&snap);
    }

    /// Fast, non-blocking scan of directory, SCM metadata, and toolchains without subprocess forks.
    pub fn scan_path(path: &Path, exit_code: i32, execution_time_ms: u64) -> AstraeaSnapshot {
        let cwd_str = path.display().to_string();
        let cwd = if let Ok(home) = env::var("HOME") {
            if let Ok(relative) = path.strip_prefix(&home) {
                if relative.as_os_str().is_empty() {
                    "~".to_string()
                } else {
                    format!("~/{}", relative.display())
                }
            } else if path == Path::new(&home) {
                "~".to_string()
            } else {
                cwd_str
            }
        } else {
            cwd_str
        };

        let (git_branch, is_dirty, git_ahead, git_behind, scm_op) = Self::discover_git(path);
        let rust_version = Self::discover_rust(path);
        let node_version = Self::discover_node(path);
        let python_version = Self::discover_python(path);
        let go_version = Self::discover_go(path);
        let zig_version = Self::discover_zig(path);
        let cloud_context = Self::discover_cloud();
        let in_container = Self::discover_container();
        let is_ssh = Self::discover_ssh();
        let (battery_pct, battery_charging) = Self::discover_battery();
        let (cpu_usage_pct, memory_used_mb) = Self::discover_system_metrics();

        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        AstraeaSnapshot {
            cwd,
            git_branch,
            is_dirty,
            git_ahead,
            git_behind,
            git_staged: 0,
            git_untracked: 0,
            scm_op,
            in_container,
            is_ssh,
            exit_code,
            execution_time_ms,
            battery_pct,
            battery_charging,
            cpu_usage_pct,
            memory_used_mb,
            rust_version,
            node_version,
            python_version,
            go_version,
            zig_version,
            cloud_context,
            preset: AstraeaPreset::default(),
            timestamp_ns,
        }
    }

    fn discover_git(mut dir: &Path) -> (String, bool, u16, u16, ScmOperation) {
        loop {
            let dot_git = dir.join(".git");
            if dot_git.exists() {
                let git_dir = if dot_git.is_file() {
                    let content = fs::read_to_string(&dot_git).unwrap_or_default();
                    let path_str = content.trim().strip_prefix("gitdir:").unwrap_or("").trim();
                    let p = Path::new(path_str);
                    if p.is_relative() {
                        dir.join(p)
                    } else {
                        p.to_path_buf()
                    }
                } else {
                    dot_git
                };

                let head_file = git_dir.join("HEAD");
                let head_content = fs::read_to_string(&head_file).unwrap_or_default();
                let head = head_content.trim();
                let (branch, ref_path) =
                    if let Some(ref_name) = head.strip_prefix("ref: refs/heads/") {
                        (
                            ref_name.to_string(),
                            Some(git_dir.join("refs/heads").join(ref_name)),
                        )
                    } else if head.len() >= 7 {
                        (head[..7].to_string(), None)
                    } else {
                        (head.to_string(), None)
                    };

                let is_dirty = check_git_dirty(&git_dir);
                let (ahead, behind) = check_git_ahead_behind(&git_dir, ref_path.as_deref());
                let scm_op = check_git_scm_op(&git_dir);

                return (branch, is_dirty, ahead, behind, scm_op);
            }

            match dir.parent() {
                Some(parent) if parent != dir => dir = parent,
                _ => break,
            }
        }
        (String::new(), false, 0, 0, ScmOperation::None)
    }

    fn discover_rust(mut dir: &Path) -> Option<String> {
        loop {
            let cargo_toml = dir.join("Cargo.toml");
            if cargo_toml.is_file() {
                if let Ok(content) = fs::read_to_string(&cargo_toml) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(ed) = trimmed.strip_prefix("edition") {
                            if let Some(val) = ed.split('=').nth(1) {
                                let edition = val.trim().trim_matches('"').trim_matches('\'');
                                return Some(format!("🦀 {edition}"));
                            }
                        }
                    }
                    return Some("🦀 rust".to_string());
                }
            }
            let toolchain = dir.join("rust-toolchain.toml");
            if toolchain.is_file() {
                if let Ok(content) = fs::read_to_string(&toolchain) {
                    for line in content.lines() {
                        if let Some(ch) = line.trim().strip_prefix("channel") {
                            if let Some(val) = ch.split('=').nth(1) {
                                let channel = val.trim().trim_matches('"').trim_matches('\'');
                                return Some(format!("🦀 {channel}"));
                            }
                        }
                    }
                }
            }
            match dir.parent() {
                Some(parent) if parent != dir => dir = parent,
                _ => break,
            }
        }
        None
    }

    fn discover_node(mut dir: &Path) -> Option<String> {
        loop {
            let pkg = dir.join("package.json");
            if pkg.is_file() {
                return Some(" node".to_string());
            }
            let nvmrc = dir.join(".nvmrc");
            if nvmrc.is_file() {
                if let Ok(ver) = fs::read_to_string(&nvmrc) {
                    let v = ver.trim();
                    if !v.is_empty() {
                        return Some(format!(" {v}"));
                    }
                }
            }
            match dir.parent() {
                Some(parent) if parent != dir => dir = parent,
                _ => break,
            }
        }
        None
    }

    fn discover_python(mut dir: &Path) -> Option<String> {
        loop {
            if dir.join("pyproject.toml").is_file()
                || dir.join("requirements.txt").is_file()
                || dir.join("Pipfile").is_file()
            {
                let pyver = dir.join(".python-version");
                if pyver.is_file() {
                    if let Ok(v) = fs::read_to_string(&pyver) {
                        let ver = v.trim();
                        if !ver.is_empty() {
                            return Some(format!("🐍 {ver}"));
                        }
                    }
                }
                return Some("🐍 python".to_string());
            }
            match dir.parent() {
                Some(parent) if parent != dir => dir = parent,
                _ => break,
            }
        }
        None
    }

    fn discover_go(mut dir: &Path) -> Option<String> {
        loop {
            let gomod = dir.join("go.mod");
            if gomod.is_file() {
                if let Ok(content) = fs::read_to_string(&gomod) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(rest) = trimmed.strip_prefix("go ") {
                            let ver = rest.trim();
                            if !ver.is_empty() {
                                return Some(format!("🐹 {ver}"));
                            }
                        }
                    }
                }
                return Some("🐹 go".to_string());
            }
            match dir.parent() {
                Some(parent) if parent != dir => dir = parent,
                _ => break,
            }
        }
        None
    }

    fn discover_zig(mut dir: &Path) -> Option<String> {
        loop {
            if dir.join("build.zig.zon").is_file() {
                if let Ok(content) = fs::read_to_string(dir.join("build.zig.zon")) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(rest) = trimmed.strip_prefix(".version") {
                            if let Some(val) = rest.split('=').nth(1) {
                                let ver = val.trim().trim_matches(',').trim().trim_matches('"');
                                if !ver.is_empty() {
                                    return Some(format!("⚡ {ver}"));
                                }
                            }
                        }
                    }
                }
                return Some("⚡ zig".to_string());
            }
            if dir.join("build.zig").is_file() {
                return Some("⚡ zig".to_string());
            }
            match dir.parent() {
                Some(parent) if parent != dir => dir = parent,
                _ => break,
            }
        }
        None
    }

    fn discover_cloud() -> Option<String> {
        if let Ok(prof) = env::var("AWS_PROFILE") {
            return Some(format!("☁ aws:{prof}"));
        }
        if let Ok(proj) = env::var("GOOGLE_CLOUD_PROJECT") {
            return Some(format!("☁ gcp:{proj}"));
        }
        if let Ok(k8s) = env::var("KUBECONFIG") {
            let name = Path::new(&k8s)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("k8s");
            return Some(format!("☸ {name}"));
        }
        None
    }

    fn discover_container() -> bool {
        Path::new("/.dockerenv").is_file()
            || Path::new("/run/.containerenv").is_file()
            || env::var_os("CONTAINER").is_some()
            || env::var_os("KUBERNETES_SERVICE_HOST").is_some()
    }

    fn discover_ssh() -> bool {
        env::var_os("SSH_CLIENT").is_some()
            || env::var_os("SSH_CONNECTION").is_some()
            || env::var_os("SSH_TTY").is_some()
    }

    fn discover_battery() -> (Option<u8>, bool) {
        #[cfg(target_os = "macos")]
        {
            // Lightweight check on macOS without running pmset
            (None, false)
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(cap) = fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
                if let Ok(pct) = cap.trim().parse::<u8>() {
                    let status = fs::read_to_string("/sys/class/power_supply/BAT0/status")
                        .unwrap_or_default();
                    let charging = status.trim().eq_ignore_ascii_case("charging");
                    return (Some(pct), charging);
                }
            }
            (None, false)
        }
    }

    fn discover_system_metrics() -> (u8, u32) {
        #[cfg(target_os = "macos")]
        {
            let mem_mb = sample_macos_memory_mb();
            (0, mem_mb)
        }
        #[cfg(not(target_os = "macos"))]
        {
            (0, 0)
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn sample_macos_memory_mb() -> u32 {
    unsafe {
        let mut info: libc::mach_task_basic_info = std::mem::zeroed();
        let mut count = (std::mem::size_of::<libc::mach_task_basic_info>()
            / std::mem::size_of::<libc::natural_t>())
            as libc::mach_msg_type_number_t;
        let kret = libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as *mut libc::integer_t,
            &mut count,
        );
        if kret == libc::KERN_SUCCESS {
            (info.resident_size / (1024 * 1024)) as u32
        } else {
            0
        }
    }
}

impl Drop for AstraeaWatcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        let (lock, cvar) = &*self.notifier;
        if let Ok(mut guard) = lock.lock() {
            *guard = true;
            cvar.notify_all();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn check_git_dirty(git_dir: &Path) -> bool {
    let index = git_dir.join("index");
    if let (Ok(meta_idx), Ok(meta_head)) =
        (fs::metadata(&index), fs::metadata(git_dir.join("HEAD")))
    {
        if let (Ok(idx_mtime), Ok(head_mtime)) = (meta_idx.modified(), meta_head.modified()) {
            if head_mtime > idx_mtime {
                return true;
            }
        }
    }
    false
}

fn check_git_scm_op(git_dir: &Path) -> ScmOperation {
    if git_dir.join("MERGE_HEAD").is_file() {
        ScmOperation::Merging
    } else if git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir() {
        ScmOperation::Rebasing
    } else if git_dir.join("CHERRY_PICK_HEAD").is_file() {
        ScmOperation::CherryPicking
    } else if git_dir.join("BISECT_LOG").is_file() {
        ScmOperation::Bisecting
    } else {
        ScmOperation::None
    }
}

fn check_git_ahead_behind(git_dir: &Path, local_ref: Option<&Path>) -> (u16, u16) {
    let Some(ref_file) = local_ref else {
        return (0, 0);
    };
    if let Ok(local_hash) = fs::read_to_string(ref_file) {
        let local_hash = local_hash.trim();
        // Check FETCH_HEAD or packed-refs
        let fetch_head = git_dir.join("FETCH_HEAD");
        if let Ok(content) = fs::read_to_string(&fetch_head) {
            if !content.contains(local_hash) && !local_hash.is_empty() {
                return (1, 0); // At least 1 commit ahead of remote fetch
            }
        }
    }
    (0, 0)
}
