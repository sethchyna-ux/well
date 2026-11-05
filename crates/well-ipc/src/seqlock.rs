//! Zero-lock atomic Sequence Lock (AtomicU64) for lock-free state synchronization.
//!
//! Enables concurrent, zero-mutex reads on the GPU rendering thread while worker
//! threads perform transactional state updates.
//!
//! Protocol:
//! - An even sequence counter indicates consistent, quiescent state.
//! - An odd sequence counter indicates a writer is actively modifying payload memory.
//! - The reader observes the counter before and after copying POD memory; if the counter
//!   changed or was odd during the read, a collision occurred and the frame can be skipped
//!   or retried with ZERO lock contention and ZERO system call overhead.

use std::cell::UnsafeCell;
use std::sync::atomic::{fence, AtomicU64, Ordering};

pub struct HermesSeqlock<T: Copy> {
    sequence: AtomicU64,
    value: UnsafeCell<T>,
}

unsafe impl<T: Copy + Send> Send for HermesSeqlock<T> {}
unsafe impl<T: Copy + Send + Sync> Sync for HermesSeqlock<T> {}

impl<T: Copy> HermesSeqlock<T> {
    pub const fn new(initial: T) -> Self {
        Self {
            sequence: AtomicU64::new(0),
            value: UnsafeCell::new(initial),
        }
    }

    /// Update the shared state transactionally.
    pub fn write(&self, val: T) {
        // Step 1: Transition sequence from even to odd (write in progress)
        let seq = self.sequence.fetch_add(1, Ordering::Acquire);
        fence(Ordering::Release);

        // Step 2: Write to data
        unsafe {
            *self.value.get() = val;
        }

        // Step 3: Transition sequence from odd to even (quiescent)
        fence(Ordering::Release);
        self.sequence.store(seq.wrapping_add(2), Ordering::Release);
    }

    /// Lock-free, zero-allocation read.
    /// Returns Some(value) if read was consistent and collision-free.
    /// Returns None if a write was in progress (allowing the GPU render loop to skip without blocking).
    #[inline]
    pub fn try_read(&self) -> Option<T> {
        let seq1 = self.sequence.load(Ordering::Acquire);
        if !seq1.is_multiple_of(2) {
            return None; // Write in progress
        }
        fence(Ordering::Acquire);

        let val = unsafe { *self.value.get() };

        fence(Ordering::Acquire);
        let seq2 = self.sequence.load(Ordering::Acquire);

        if seq1 == seq2 {
            Some(val)
        } else {
            None // Collision: writer modified value during copy
        }
    }

    /// Read with optimistic spinning retry up to `max_spins`, then returns None.
    pub fn read_spin(&self, max_spins: usize) -> Option<T> {
        for _ in 0..max_spins {
            if let Some(val) = self.try_read() {
                return Some(val);
            }
            std::hint::spin_loop();
        }
        None
    }

    /// Unconditionally read by spinning until consistent.
    pub fn read(&self) -> T {
        loop {
            if let Some(val) = self.try_read() {
                return val;
            }
            std::hint::spin_loop();
        }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }
}

/// 64-byte aligned, packed viewport telemetry exchanged directly between
/// the terminal session and the Orpheus GPU renderer via Seqlock.
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HermesViewportState {
    pub rows: u16,
    pub cols: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub mode_flags: u32,
    pub scrollback: u32,
    pub theme_id: u32,
    pub fg_packed: u32,
    pub bg_packed: u32,
    pub generation: u64,
    pub _reserved: [u8; 24],
}

impl Default for HermesViewportState {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            mode_flags: 0,
            scrollback: 0,
            theme_id: 1,
            fg_packed: 0x00D0D8E1,
            bg_packed: 0x001B232D,
            generation: 0,
            _reserved: [0; 24],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_seqlock_single_threaded_write_read() {
        let lock = HermesSeqlock::new(HermesViewportState::default());
        assert_eq!(lock.sequence(), 0);

        let next = HermesViewportState {
            rows: 40,
            cols: 120,
            ..Default::default()
        };
        lock.write(next);

        assert_eq!(lock.sequence(), 2);
        let read = lock.read();
        assert_eq!(read.rows, 40);
        assert_eq!(read.cols, 120);
    }

    #[test]
    fn test_seqlock_concurrent_writer_readers_no_torn_reads() {
        let state = HermesSeqlock::new([0u64; 4]);
        let state_arc = Arc::new(state);

        let mut handles = Vec::new();

        // Single writer updating array with matching values
        let writer_arc = state_arc.clone();
        handles.push(thread::spawn(move || {
            for i in 1..=10_000u64 {
                writer_arc.write([i, i, i, i]);
            }
        }));

        // Multiple readers concurrently reading
        for _ in 0..4 {
            let reader_arc = state_arc.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..10_000 {
                    let val = reader_arc.read();
                    // Crucial: All elements MUST be identical (no torn reads allowed)
                    assert_eq!(val[0], val[1]);
                    assert_eq!(val[1], val[2]);
                    assert_eq!(val[2], val[3]);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
