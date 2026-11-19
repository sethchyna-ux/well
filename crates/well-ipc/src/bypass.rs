//! Hermes PTY-Bypass Protocol.
//!
//! Replaces legacy serial character stream terminal drivers (/dev/ptmx) with
//! zero-copy shared-memory queues directly connecting the Metis shell engine
//! and the Orpheus GPU renderer.
//!
//! Features:
//! - Strongly typed binary messages instead of ANSI escape parsing overhead.
//! - Direct, non-blocking I/O queues eliminating kernel termios line discipline bottlenecks.
//! - Full progressive Kitty Keyboard Protocol message frames.

use crate::ring_buffer::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};

/// Strongly typed protocol messages exchanged across the Hermes PTY-bypass bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HermesBypassMessage {
    /// Raw unescaped text chunk intended for terminal cell insertion
    Stdout(Vec<u8>),
    /// Diagnostic / error stream
    Stderr(Vec<u8>),
    /// Process exit with integer status code
    ExitStatus(i32),
    /// Working directory synchronization event
    CwdChanged(String),
    /// Process environment variable modification
    EnvChanged { key: String, value: String },
    /// Dynamic window resize event
    Resize { rows: u16, cols: u16 },
    /// Progressive Kitty keyboard input event
    KeyboardInput {
        key_code: u32,
        modifiers: u32,
        event_type: u8, // 1=press, 2=release, 3=repeat
    },
    /// Direct command line synchronization (Metis predictive command pipeline)
    CommandSubmit(String),
    /// GPU viewport scrollback adjustment
    ScrollbackDelta(i32),
}

/// Bidirectional, lock-free PTY bypass bus connecting Shell Core and Terminal Host.
pub struct HermesBypassBus<const CAPACITY: usize = 1024> {
    pub shell_to_host_tx: Producer<HermesBypassMessage, CAPACITY>,
    pub shell_to_host_rx: Consumer<HermesBypassMessage, CAPACITY>,
    pub host_to_shell_tx: Producer<HermesBypassMessage, CAPACITY>,
    pub host_to_shell_rx: Consumer<HermesBypassMessage, CAPACITY>,
}

impl<const CAPACITY: usize> HermesBypassBus<CAPACITY> {
    pub fn new() -> (
        Self,
        HermesBypassEndpoint<CAPACITY>,
        HermesBypassEndpoint<CAPACITY>,
    ) {
        let (s2h_tx, s2h_rx) = RingBuffer::<HermesBypassMessage, CAPACITY>::new().split();
        let (h2s_tx, h2s_rx) = RingBuffer::<HermesBypassMessage, CAPACITY>::new().split();

        let host_endpoint = HermesBypassEndpoint {
            outgoing: s2h_tx.clone(),
            incoming: h2s_rx,
        };

        let shell_endpoint = HermesBypassEndpoint {
            outgoing: h2s_tx.clone(),
            incoming: s2h_rx,
        };

        let bus = Self {
            shell_to_host_tx: s2h_tx,
            shell_to_host_rx: shell_endpoint.incoming.clone(),
            host_to_shell_tx: h2s_tx,
            host_to_shell_rx: host_endpoint.incoming.clone(),
        };

        (bus, host_endpoint, shell_endpoint)
    }
}

pub struct HermesBypassEndpoint<const CAPACITY: usize> {
    pub outgoing: Producer<HermesBypassMessage, CAPACITY>,
    pub incoming: Consumer<HermesBypassMessage, CAPACITY>,
}

impl<const CAPACITY: usize> HermesBypassEndpoint<CAPACITY> {
    pub fn send(&self, msg: HermesBypassMessage) -> Result<(), HermesBypassMessage> {
        self.outgoing.try_push(msg)
    }

    pub fn poll(&self) -> Option<HermesBypassMessage> {
        self.incoming.try_pop()
    }
}

/// Dual-mode execution routing decision for the PTY-Bypass Daemon (Phase P2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BypassExecutionMode {
    /// Cooperating high-throughput stream routed via zero-copy POSIX `mmap` SPSC ring.
    SharedMemoryZeroCopy,
    /// Interactive application requiring `termios` ioctls routed via `/dev/ptmx`.
    PosixPtyFallback,
}

/// Cache-line aligned shared-memory header placed at the start of the `mmap` region.
#[repr(C, align(64))]
struct ShmRingHeader {
    write_pos: std::sync::atomic::AtomicUsize,
    _pad0: [u8; 64 - std::mem::size_of::<std::sync::atomic::AtomicUsize>()],
    read_pos: std::sync::atomic::AtomicUsize,
    _pad1: [u8; 64 - std::mem::size_of::<std::sync::atomic::AtomicUsize>()],
    capacity: usize,
}

/// POSIX `mmap` (`MAP_SHARED`) lock-free SPSC circular byte ring buffer that bypasses `/dev/ptmx`.
pub struct SharedMemoryBypassRing {
    ptr: *mut u8,
    total_len: usize,
    data_offset: usize,
    capacity: usize,
}

unsafe impl Send for SharedMemoryBypassRing {}
unsafe impl Sync for SharedMemoryBypassRing {}

impl SharedMemoryBypassRing {
    /// Allocates a shared-memory SPSC circular buffer of `capacity` bytes (rounded up to power of two).
    pub fn new(capacity: usize) -> std::io::Result<Self> {
        let cap = capacity.next_power_of_two().max(4096);
        let data_offset = std::mem::size_of::<ShmRingHeader>().next_multiple_of(64);
        let total_len = data_offset + cap;

        #[cfg(unix)]
        let ptr = unsafe {
            let addr = libc::mmap(
                std::ptr::null_mut(),
                total_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED | libc::MAP_ANON,
                -1,
                0,
            );
            if addr == libc::MAP_FAILED {
                return Err(std::io::Error::last_os_error());
            }
            let header = addr as *mut ShmRingHeader;
            std::ptr::write(
                header,
                ShmRingHeader {
                    write_pos: std::sync::atomic::AtomicUsize::new(0),
                    _pad0: [0u8; 64 - std::mem::size_of::<std::sync::atomic::AtomicUsize>()],
                    read_pos: std::sync::atomic::AtomicUsize::new(0),
                    _pad1: [0u8; 64 - std::mem::size_of::<std::sync::atomic::AtomicUsize>()],
                    capacity: cap,
                },
            );
            addr as *mut u8
        };

        #[cfg(not(unix))]
        let ptr = {
            let mut vec = vec![0u8; total_len].into_boxed_slice();
            let raw = Box::into_raw(vec) as *mut u8;
            raw
        };

        Ok(Self {
            ptr,
            total_len,
            data_offset,
            capacity: cap,
        })
    }

    #[inline]
    fn header(&self) -> &ShmRingHeader {
        unsafe { &*(self.ptr as *const ShmRingHeader) }
    }

    #[inline]
    fn data_ptr(&self) -> *mut u8 {
        unsafe { self.ptr.add(self.data_offset) }
    }

    /// Writes as many bytes from `src` into the shared-memory ring as currently fit.
    pub fn write_chunk(&self, src: &[u8]) -> usize {
        use std::sync::atomic::Ordering;
        let hdr = self.header();
        let write = hdr.write_pos.load(Ordering::Relaxed);
        let read = hdr.read_pos.load(Ordering::Acquire);
        let used = write.wrapping_sub(read);
        let avail = self.capacity.saturating_sub(used);
        let to_write = src.len().min(avail);
        if to_write == 0 {
            return 0;
        }

        let mask = self.capacity - 1;
        let start = write & mask;
        let first_part = to_write.min(self.capacity - start);
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.data_ptr().add(start), first_part);
            if first_part < to_write {
                std::ptr::copy_nonoverlapping(
                    src.as_ptr().add(first_part),
                    self.data_ptr(),
                    to_write - first_part,
                );
            }
        }
        hdr.write_pos
            .store(write.wrapping_add(to_write), Ordering::Release);
        to_write
    }

    /// Reads available bytes from the shared-memory ring into `dst`.
    pub fn read_chunk(&self, dst: &mut [u8]) -> usize {
        use std::sync::atomic::Ordering;
        let hdr = self.header();
        let read = hdr.read_pos.load(Ordering::Relaxed);
        let write = hdr.write_pos.load(Ordering::Acquire);
        let avail = write.wrapping_sub(read);
        let to_read = dst.len().min(avail);
        if to_read == 0 {
            return 0;
        }

        let mask = self.capacity - 1;
        let start = read & mask;
        let first_part = to_read.min(self.capacity - start);
        unsafe {
            std::ptr::copy_nonoverlapping(self.data_ptr().add(start), dst.as_mut_ptr(), first_part);
            if first_part < to_read {
                std::ptr::copy_nonoverlapping(
                    self.data_ptr(),
                    dst.as_mut_ptr().add(first_part),
                    to_read - first_part,
                );
            }
        }
        hdr.read_pos
            .store(read.wrapping_add(to_read), Ordering::Release);
        to_read
    }

    /// Determines whether `command` requires `/dev/ptmx` `termios` or can use `SharedMemoryZeroCopy`.
    pub fn classify_command(command: &str) -> BypassExecutionMode {
        let first_token = command
            .split_whitespace()
            .next()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("");
        const TERMIOS_INTERACTIVE_BINARIES: &[&str] = &[
            "vim", "nvim", "vi", "nano", "emacs", "ssh", "tmux", "screen", "htop", "top", "less",
            "more", "man", "fzf", "lazygit",
        ];
        if TERMIOS_INTERACTIVE_BINARIES.contains(&first_token) {
            BypassExecutionMode::PosixPtyFallback
        } else {
            BypassExecutionMode::SharedMemoryZeroCopy
        }
    }
}

impl Drop for SharedMemoryBypassRing {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            let _ = libc::munmap(self.ptr as *mut libc::c_void, self.total_len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bypass_bus_strongly_typed_exchange() {
        let (_bus, host, shell) = HermesBypassBus::<64>::new();

        // Host sends keyboard input directly without serial tty encoding
        host.send(HermesBypassMessage::KeyboardInput {
            key_code: 65, // 'a'
            modifiers: 0,
            event_type: 1,
        })
        .unwrap();

        let received = shell.poll().expect("Shell should receive keyboard event");
        assert_eq!(
            received,
            HermesBypassMessage::KeyboardInput {
                key_code: 65,
                modifiers: 0,
                event_type: 1,
            }
        );

        // Shell streams output and working directory update
        shell
            .send(HermesBypassMessage::Stdout(
                b"fast in-process output".to_vec(),
            ))
            .unwrap();
        shell
            .send(HermesBypassMessage::CwdChanged(
                "/Users/yocan/Desktop/well".into(),
            ))
            .unwrap();

        assert_eq!(
            host.poll(),
            Some(HermesBypassMessage::Stdout(
                b"fast in-process output".to_vec()
            ))
        );
        assert_eq!(
            host.poll(),
            Some(HermesBypassMessage::CwdChanged(
                "/Users/yocan/Desktop/well".into()
            ))
        );
    }

    #[test]
    fn test_shared_memory_100mib_zero_copy_stream_and_termios_fallback() {
        // 1. Verify automatic fallback to /dev/ptmx for interactive termios programs
        assert_eq!(
            SharedMemoryBypassRing::classify_command("nvim src/main.rs"),
            BypassExecutionMode::PosixPtyFallback
        );
        assert_eq!(
            SharedMemoryBypassRing::classify_command("ssh prod-node-01"),
            BypassExecutionMode::PosixPtyFallback
        );
        assert_eq!(
            SharedMemoryBypassRing::classify_command("rg --line-number fn"),
            BypassExecutionMode::SharedMemoryZeroCopy
        );

        // 2. Stream 100 MiB across the shared-memory mmap ring buffer without touching /dev/ptmx
        const TOTAL_BYTES: usize = 100 * 1024 * 1024;
        let ring = std::sync::Arc::new(
            SharedMemoryBypassRing::new(16 * 1024 * 1024).expect("allocate mmap ring"),
        );
        let send_chunk = vec![0x5A_u8; 4 * 1024 * 1024];
        let mut recv_chunk = vec![0_u8; 4 * 1024 * 1024];

        let start = std::time::Instant::now();
        let mut transferred = 0usize;
        while transferred < TOTAL_BYTES {
            let rem = (TOTAL_BYTES - transferred).min(send_chunk.len());
            let w = ring.write_chunk(&send_chunk[..rem]);
            let r = ring.read_chunk(&mut recv_chunk[..w]);
            assert_eq!(w, r);
            transferred += r;
        }
        let elapsed = start.elapsed();
        assert_eq!(transferred, TOTAL_BYTES);
        assert_eq!(recv_chunk[0], 0x5A);
        assert!(
            elapsed.as_millis() < 250,
            "100 MiB shared-memory transfer took {:?}",
            elapsed
        );
    }
}
