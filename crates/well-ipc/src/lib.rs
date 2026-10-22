//! well-ipc: typed terminal messages and synchronized state primitives.
//!
//! Subsystems:
//! - Hermes: sequence tracking for synchronized state updates.
//! - RingBuffer: bounded queue with synchronized producer and consumer access.
//! - KittyKeyboardInput & TypedBlock: Strongly typed protocol formats for terminal I/O.

pub mod ring_buffer;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

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

/// Sequence counter for state updates.
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

    /// Acquires the write sequence, waiting for other writers to finish.
    pub fn write_begin(&self) -> u64 {
        loop {
            let sequence = self.counter.load(Ordering::Acquire);
            if sequence % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }
            if self
                .counter
                .compare_exchange(sequence, sequence + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return sequence;
            }
        }
    }

    /// Releases the write sequence.
    pub fn write_end(&self, seq: u64) {
        self.counter.store(seq + 2, Ordering::Release);
    }

    /// Reads the sequence before accessing state.
    pub fn read_begin(&self) -> u64 {
        self.counter.load(Ordering::Acquire)
    }

    /// Returns true if the state remained unmodified during the read transaction.
    pub fn read_validate(&self, seq: u64) -> bool {
        let current = self.counter.load(Ordering::Acquire);
        seq == current && seq % 2 == 0
    }

    /// Returns current atomic sequence counter (even = idle, odd = writing).
    pub fn sequence(&self) -> u64 {
        self.counter.load(Ordering::Acquire)
    }
}

/// Synchronized state channel with an atomic update generation.
pub struct HermesChannel<T> {
    seqlock: HermesSeqlock,
    payload: RwLock<T>,
}

impl<T: Clone> HermesChannel<T> {
    pub fn new(initial: T) -> Self {
        Self {
            seqlock: HermesSeqlock::new(),
            payload: RwLock::new(initial),
        }
    }

    /// Replaces the state and advances the generation.
    pub fn sync_state(&self, new_state: T) {
        let mut payload = self.payload.write().expect("Hermes state lock poisoned");
        let seq = self.seqlock.write_begin();
        *payload = new_state;
        self.seqlock.write_end(seq);
    }

    /// Returns a consistent snapshot of the current state.
    pub fn read_state(&self) -> T {
        self.payload
            .read()
            .expect("Hermes state lock poisoned")
            .clone()
    }

    /// Returns the current state generation for telemetry.
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

    #[test]
    fn concurrent_state_updates_are_serialized() {
        use std::sync::Arc;
        use std::thread;

        let channel = Arc::new(HermesChannel::new(0u64));
        let writers: Vec<_> = (0..4)
            .map(|_| {
                let channel = Arc::clone(&channel);
                thread::spawn(move || {
                    for value in 0..1_000 {
                        channel.sync_state(value);
                    }
                })
            })
            .collect();

        for writer in writers {
            writer.join().unwrap();
        }

        assert_eq!(channel.sequence(), 8_000);
        assert!(channel.read_state() < 1_000);
    }
}
