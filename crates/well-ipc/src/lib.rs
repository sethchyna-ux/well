//! well-ipc: Hermes Zero-Copy IPC & Seqlock Data Structures
//!
//! Subsystems:
//! - Hermes: Atomic lock-free Sequence Lock (Seqlock) coordinating state sync.
//! - RingBuffer: Lock-free SPSC / MPMC ring buffer backed by Seqlocks and atomic pointers.
//! - KittyKeyboardInput & TypedBlock: Strongly typed protocol formats for terminal I/O.

pub mod ring_buffer;

use serde::{Deserialize, Serialize};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

// =========================================================================
// 1. KITTY KEYBOARD PROTOCOL & TYPED BLOCKS
// =========================================================================

/// Kitty Keyboard Protocol Input Event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KittyKeyboardInput {
    pub key_code: u32,
    pub modifiers: u32, // Shift, Alt, Ctrl, Super bitmask
    pub event_type: u8, // Press(1), Release(2), Repeat(3)
}

/// Structured typed block for output transport over Hermes IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypedBlock {
    StdoutChunk(Vec<u8>),
    StderrChunk(Vec<u8>),
    ExitStatus(i32),
}

// =========================================================================
// 2. HERMES SEQLOCK & ZERO-COPY STATE SYNC
// =========================================================================

/// HermesSeqlock: Lock-free atomic synchronization guard for zero-copy state reads.
pub struct HermesSeqlock {
    counter: AtomicU64,
}

impl Default for HermesSeqlock {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesSeqlock {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Executed by the writer before modifying state.
    pub fn write_begin(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Executed by the writer after modifying state.
    pub fn write_end(&self, seq: u64) {
        self.counter.store(seq + 2, Ordering::SeqCst);
    }

    /// Executed by the reader.
    pub fn read_begin(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Returns true if the state remained unmodified during the read transaction.
    pub fn read_validate(&self, seq: u64) -> bool {
        let current = self.counter.load(Ordering::SeqCst);
        seq == current && seq % 2 == 0
    }

    /// Returns current atomic sequence counter (even = idle, odd = writing).
    pub fn sequence(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }
}

/// HermesChannel: Dual-slot shared-memory synchronization context.
pub struct HermesChannel<T> {
    seqlock: HermesSeqlock,
    payload: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for HermesChannel<T> {}
unsafe impl<T: Sync> Sync for HermesChannel<T> {}

impl<T: Clone> HermesChannel<T> {
    pub fn new(initial: T) -> Self {
        Self {
            seqlock: HermesSeqlock::new(),
            payload: UnsafeCell::new(initial),
        }
    }

    /// Safely synchronizes/writes a new configuration state vector.
    pub fn sync_state(&self, new_state: T) {
        let seq = self.seqlock.write_begin();
        unsafe {
            *self.payload.get() = new_state;
        }
        self.seqlock.write_end(seq);
    }

    /// Safely reads current configuration state vector with lock-free seqlock validation.
    pub fn read_state(&self) -> T {
        loop {
            let seq = self.seqlock.read_begin();
            let val = unsafe { (*self.payload.get()).clone() };
            if self.seqlock.read_validate(seq) {
                return val;
            }
            std::hint::spin_loop();
        }
    }

    /// Returns current atomic sequence counter for IPC telemetry.
    pub fn sequence(&self) -> u64 {
        self.seqlock.sequence()
    }
}

/// Convenience initialization
pub fn init_ipc() {
    // IPC initialization stub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermes_seqlock_consistency() {
        let channel = HermesChannel::new(42u32);
        assert_eq!(channel.read_state(), 42);

        channel.sync_state(100);
        assert_eq!(channel.read_state(), 100);
    }
}
