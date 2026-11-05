//! Kernel-mediated, memory-mapped lock-free state vector for Astraea.
//! Uses a 1024-byte cache-aligned POD struct with Seqlock synchronization
//! to achieve prompt compile times strictly under 100 microseconds.

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};

pub const ASTRAEA_MAGIC: u64 = 0x41535452_41454101; // "ASTRAEA\x01"

/// Current SCM operation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ScmOperation {
    #[default]
    None = 0,
    Merging = 1,
    Rebasing = 2,
    CherryPicking = 3,
    Bisecting = 4,
}

impl ScmOperation {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Merging,
            2 => Self::Rebasing,
            3 => Self::CherryPicking,
            4 => Self::Bisecting,
            _ => Self::None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Merging => "MERGING",
            Self::Rebasing => "REBASING",
            Self::CherryPicking => "CHERRY-PICKING",
            Self::Bisecting => "BISECTING",
        }
    }
}

/// Prompt styling presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum AstraeaPreset {
    #[default]
    Starship = 0,
    Minimal = 1,
    Powerline = 2,
    TokyoNight = 3,
    Pastel = 4,
    Pure = 5,
    BracketedSegments = 6,
}

impl AstraeaPreset {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Minimal,
            2 => Self::Powerline,
            3 => Self::TokyoNight,
            4 => Self::Pastel,
            5 => Self::Pure,
            6 => Self::BracketedSegments,
            _ => Self::Starship,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Starship => "Starship",
            Self::Minimal => "Minimal",
            Self::Powerline => "Powerline",
            Self::TokyoNight => "Tokyo Night",
            Self::Pastel => "Pastel",
            Self::Pure => "Pure",
            Self::BracketedSegments => "Bracketed Segments",
        }
    }
}

/// Fixed-size POD binary layout representing the system & SCM state vector.
/// Total size: 1024 bytes (strictly 64-byte aligned for cache locality).
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct AstraeaRawVector {
    pub magic: u64,
    pub seqlock: u64,
    pub timestamp_ns: u64,
    pub exit_code: i32,
    pub _align1: u32,
    pub execution_time_ms: u64,
    pub is_dirty: u8,
    pub battery_pct: u8,
    pub battery_charging: u8,
    pub cpu_usage_pct: u8,
    pub git_ahead: u16,
    pub git_behind: u16,
    pub git_staged: u16,
    pub git_untracked: u16,
    pub scm_op: u8,
    pub in_container: u8,
    pub is_ssh: u8,
    pub preset_id: u8,
    pub memory_used_mb: u32,
    pub cwd_len: u16,
    pub git_branch_len: u8,
    pub rust_version_len: u8,
    pub node_version_len: u8,
    pub python_version_len: u8,
    pub go_version_len: u8,
    pub zig_version_len: u8,
    pub cloud_context_len: u8,
    pub _reserved: [u8; 11],
    pub cwd_bytes: [u8; 512],
    pub git_branch_bytes: [u8; 64],
    pub rust_version_bytes: [u8; 32],
    pub node_version_bytes: [u8; 32],
    pub python_version_bytes: [u8; 32],
    pub go_version_bytes: [u8; 32],
    pub zig_version_bytes: [u8; 32],
    pub cloud_context_bytes: [u8; 64],
    pub _padding: [u8; 144],
}

const _: () = assert!(std::mem::size_of::<AstraeaRawVector>() == 1024);

impl Default for AstraeaRawVector {
    fn default() -> Self {
        Self {
            magic: ASTRAEA_MAGIC,
            seqlock: 0,
            timestamp_ns: 0,
            exit_code: 0,
            _align1: 0,
            execution_time_ms: 0,
            is_dirty: 0,
            battery_pct: 100,
            battery_charging: 0,
            cpu_usage_pct: 0,
            git_ahead: 0,
            git_behind: 0,
            git_staged: 0,
            git_untracked: 0,
            scm_op: 0,
            in_container: 0,
            is_ssh: 0,
            preset_id: 0,
            memory_used_mb: 0,
            cwd_len: 0,
            git_branch_len: 0,
            rust_version_len: 0,
            node_version_len: 0,
            python_version_len: 0,
            go_version_len: 0,
            zig_version_len: 0,
            cloud_context_len: 0,
            _reserved: [0; 11],
            cwd_bytes: [0; 512],
            git_branch_bytes: [0; 64],
            rust_version_bytes: [0; 32],
            node_version_bytes: [0; 32],
            python_version_bytes: [0; 32],
            go_version_bytes: [0; 32],
            zig_version_bytes: [0; 32],
            cloud_context_bytes: [0; 64],
            _padding: [0; 144],
        }
    }
}

/// Strongly-typed, owned snapshot of the state vector.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AstraeaSnapshot {
    pub cwd: String,
    pub git_branch: String,
    pub is_dirty: bool,
    pub git_ahead: u16,
    pub git_behind: u16,
    pub git_staged: u16,
    pub git_untracked: u16,
    pub scm_op: ScmOperation,
    pub in_container: bool,
    pub is_ssh: bool,
    pub exit_code: i32,
    pub execution_time_ms: u64,
    pub battery_pct: Option<u8>,
    pub battery_charging: bool,
    pub cpu_usage_pct: u8,
    pub memory_used_mb: u32,
    pub rust_version: Option<String>,
    pub node_version: Option<String>,
    pub python_version: Option<String>,
    pub go_version: Option<String>,
    pub zig_version: Option<String>,
    pub cloud_context: Option<String>,
    pub preset: AstraeaPreset,
    pub timestamp_ns: u64,
}

impl AstraeaSnapshot {
    #[allow(clippy::field_reassign_with_default)]
    pub fn to_raw(&self) -> AstraeaRawVector {
        let mut raw = AstraeaRawVector::default();
        raw.timestamp_ns = self.timestamp_ns;
        raw.exit_code = self.exit_code;
        raw.execution_time_ms = self.execution_time_ms;
        raw.is_dirty = if self.is_dirty { 1 } else { 0 };
        raw.git_ahead = self.git_ahead;
        raw.git_behind = self.git_behind;
        raw.git_staged = self.git_staged;
        raw.git_untracked = self.git_untracked;
        raw.scm_op = self.scm_op.to_u8();
        raw.in_container = if self.in_container { 1 } else { 0 };
        raw.is_ssh = if self.is_ssh { 1 } else { 0 };
        raw.preset_id = self.preset.to_u8();
        raw.battery_pct = self.battery_pct.unwrap_or(255);
        raw.battery_charging = if self.battery_charging { 1 } else { 0 };
        raw.cpu_usage_pct = self.cpu_usage_pct;
        raw.memory_used_mb = self.memory_used_mb;

        let cwd_bytes = self.cwd.as_bytes();
        let cwd_len = cwd_bytes.len().min(512);
        raw.cwd_len = cwd_len as u16;
        raw.cwd_bytes[..cwd_len].copy_from_slice(&cwd_bytes[..cwd_len]);

        let branch_bytes = self.git_branch.as_bytes();
        let branch_len = branch_bytes.len().min(64);
        raw.git_branch_len = branch_len as u8;
        raw.git_branch_bytes[..branch_len].copy_from_slice(&branch_bytes[..branch_len]);

        if let Some(rust) = &self.rust_version {
            let b = rust.as_bytes();
            let len = b.len().min(32);
            raw.rust_version_len = len as u8;
            raw.rust_version_bytes[..len].copy_from_slice(&b[..len]);
        }

        if let Some(node) = &self.node_version {
            let b = node.as_bytes();
            let len = b.len().min(32);
            raw.node_version_len = len as u8;
            raw.node_version_bytes[..len].copy_from_slice(&b[..len]);
        }

        if let Some(py) = &self.python_version {
            let b = py.as_bytes();
            let len = b.len().min(32);
            raw.python_version_len = len as u8;
            raw.python_version_bytes[..len].copy_from_slice(&b[..len]);
        }

        if let Some(go) = &self.go_version {
            let b = go.as_bytes();
            let len = b.len().min(32);
            raw.go_version_len = len as u8;
            raw.go_version_bytes[..len].copy_from_slice(&b[..len]);
        }

        if let Some(zig) = &self.zig_version {
            let b = zig.as_bytes();
            let len = b.len().min(32);
            raw.zig_version_len = len as u8;
            raw.zig_version_bytes[..len].copy_from_slice(&b[..len]);
        }

        if let Some(cloud) = &self.cloud_context {
            let b = cloud.as_bytes();
            let len = b.len().min(64);
            raw.cloud_context_len = len as u8;
            raw.cloud_context_bytes[..len].copy_from_slice(&b[..len]);
        }

        raw
    }

    pub fn from_raw(raw: &AstraeaRawVector) -> Self {
        let cwd_len = (raw.cwd_len as usize).min(512);
        let cwd = String::from_utf8_lossy(&raw.cwd_bytes[..cwd_len]).to_string();

        let branch_len = (raw.git_branch_len as usize).min(64);
        let git_branch = String::from_utf8_lossy(&raw.git_branch_bytes[..branch_len]).to_string();

        let rust_version = if raw.rust_version_len > 0 {
            let len = (raw.rust_version_len as usize).min(32);
            Some(String::from_utf8_lossy(&raw.rust_version_bytes[..len]).to_string())
        } else {
            None
        };

        let node_version = if raw.node_version_len > 0 {
            let len = (raw.node_version_len as usize).min(32);
            Some(String::from_utf8_lossy(&raw.node_version_bytes[..len]).to_string())
        } else {
            None
        };

        let python_version = if raw.python_version_len > 0 {
            let len = (raw.python_version_len as usize).min(32);
            Some(String::from_utf8_lossy(&raw.python_version_bytes[..len]).to_string())
        } else {
            None
        };

        let go_version = if raw.go_version_len > 0 {
            let len = (raw.go_version_len as usize).min(32);
            Some(String::from_utf8_lossy(&raw.go_version_bytes[..len]).to_string())
        } else {
            None
        };

        let zig_version = if raw.zig_version_len > 0 {
            let len = (raw.zig_version_len as usize).min(32);
            Some(String::from_utf8_lossy(&raw.zig_version_bytes[..len]).to_string())
        } else {
            None
        };

        let cloud_context = if raw.cloud_context_len > 0 {
            let len = (raw.cloud_context_len as usize).min(64);
            Some(String::from_utf8_lossy(&raw.cloud_context_bytes[..len]).to_string())
        } else {
            None
        };

        Self {
            cwd,
            git_branch,
            is_dirty: raw.is_dirty != 0,
            git_ahead: raw.git_ahead,
            git_behind: raw.git_behind,
            git_staged: raw.git_staged,
            git_untracked: raw.git_untracked,
            scm_op: ScmOperation::from_u8(raw.scm_op),
            in_container: raw.in_container != 0,
            is_ssh: raw.is_ssh != 0,
            exit_code: raw.exit_code,
            execution_time_ms: raw.execution_time_ms,
            battery_pct: if raw.battery_pct <= 100 {
                Some(raw.battery_pct)
            } else {
                None
            },
            battery_charging: raw.battery_charging != 0,
            cpu_usage_pct: raw.cpu_usage_pct,
            memory_used_mb: raw.memory_used_mb,
            rust_version,
            node_version,
            python_version,
            go_version,
            zig_version,
            cloud_context,
            preset: AstraeaPreset::from_u8(raw.preset_id),
            timestamp_ns: raw.timestamp_ns,
        }
    }
}

/// Seqlock-controlled memory-mapped state vector.
pub struct AstraeaStateVector {
    ptr: *mut AstraeaRawVector,
    _mmap_len: usize,
    _file: Option<File>,
    is_mmap: bool,
    #[allow(dead_code)]
    owned: Option<Box<AstraeaRawVector>>,
}

// Safety: AstraeaStateVector access is synchronized via atomic Seqlock protocol.
unsafe impl Send for AstraeaStateVector {}
unsafe impl Sync for AstraeaStateVector {}

impl AstraeaStateVector {
    /// Create an in-memory state vector allocated on the heap with 64-byte alignment.
    pub fn in_memory() -> Self {
        let mut boxed = Box::new(AstraeaRawVector::default());
        let ptr = boxed.as_mut() as *mut AstraeaRawVector;
        Self {
            ptr,
            _mmap_len: std::mem::size_of::<AstraeaRawVector>(),
            _file: None,
            is_mmap: false,
            owned: Some(boxed),
        }
    }

    /// Open or create a memory-mapped file for the state vector.
    pub fn open_or_create_mmap(path: &Path) -> io::Result<Self> {
        let size = std::mem::size_of::<AstraeaRawVector>();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        file.set_len(size as u64)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let mmap_ptr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    0,
                )
            };

            if mmap_ptr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }

            let vec_ptr = mmap_ptr as *mut AstraeaRawVector;
            // Initialize magic if uninitialized
            unsafe {
                if (*vec_ptr).magic != ASTRAEA_MAGIC {
                    ptr::write(vec_ptr, AstraeaRawVector::default());
                }
            }

            Ok(Self {
                ptr: vec_ptr,
                _mmap_len: size,
                _file: Some(file),
                is_mmap: true,
                owned: None,
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Self::in_memory())
        }
    }

    /// Lock-free Seqlock write update.
    pub fn write_snapshot(&self, snapshot: &AstraeaSnapshot) {
        let raw = snapshot.to_raw();
        unsafe {
            let target_atomic = &*(&(*self.ptr).seqlock as *const u64 as *const AtomicU64);

            // 1. Begin write: bump seqlock to odd
            let seq = target_atomic.fetch_add(1, Ordering::AcqRel);
            let write_seq = seq.wrapping_add(1);

            // 2. Copy payload (except magic and seqlock: offset 16)
            let src = &raw as *const AstraeaRawVector as *const u8;
            let dst = self.ptr as *mut u8;
            ptr::copy_nonoverlapping(
                src.add(16),
                dst.add(16),
                std::mem::size_of::<AstraeaRawVector>() - 16,
            );

            // Copy magic
            (*self.ptr).magic = ASTRAEA_MAGIC;

            // 3. Finish write: bump seqlock to even
            target_atomic.store(write_seq.wrapping_add(1), Ordering::Release);
        }
    }

    /// Sub-microsecond lock-free Seqlock read.
    /// Retries if a write collision occurred during the read window.
    pub fn read_snapshot(&self) -> AstraeaSnapshot {
        let mut retries = 0;
        let mut raw = AstraeaRawVector::default();

        unsafe {
            let target_atomic = &*(&(*self.ptr).seqlock as *const u64 as *const AtomicU64);
            loop {
                let seq1 = target_atomic.load(Ordering::Acquire);
                if !seq1.is_multiple_of(2) {
                    // Writer active, spin lightly
                    std::hint::spin_loop();
                    retries += 1;
                    if retries > 100 {
                        std::thread::yield_now();
                    }
                    continue;
                }

                // Copy payload
                ptr::copy_nonoverlapping(
                    self.ptr as *const u8,
                    &mut raw as *mut AstraeaRawVector as *mut u8,
                    std::mem::size_of::<AstraeaRawVector>(),
                );

                std::sync::atomic::fence(Ordering::Acquire);
                let seq2 = target_atomic.load(Ordering::Acquire);

                if seq1 == seq2 {
                    break;
                }
            }
        }

        AstraeaSnapshot::from_raw(&raw)
    }

    /// Read raw state vector without mapping to strings.
    pub fn read_raw(&self) -> AstraeaRawVector {
        let mut raw = AstraeaRawVector::default();
        unsafe {
            let target_atomic = &*(&(*self.ptr).seqlock as *const u64 as *const AtomicU64);
            loop {
                let seq1 = target_atomic.load(Ordering::Acquire);
                if !seq1.is_multiple_of(2) {
                    std::hint::spin_loop();
                    continue;
                }
                ptr::copy_nonoverlapping(
                    self.ptr as *const u8,
                    &mut raw as *mut AstraeaRawVector as *mut u8,
                    std::mem::size_of::<AstraeaRawVector>(),
                );
                std::sync::atomic::fence(Ordering::Acquire);
                let seq2 = target_atomic.load(Ordering::Acquire);
                if seq1 == seq2 {
                    break;
                }
            }
        }
        raw
    }
}

impl Drop for AstraeaStateVector {
    fn drop(&mut self) {
        if self.is_mmap {
            #[cfg(unix)]
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self._mmap_len);
            }
        }
    }
}
