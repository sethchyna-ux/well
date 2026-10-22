//! Typed terminal transport, synchronized application state, and local RPC.
pub mod ring_buffer;
pub mod rpc;
pub use rpc::HermesAppCommand;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KittyKeyboardInput {
    pub key_code: u32,
    pub modifiers: u32,
    /// Legacy internal event encoding: press=1, release=2, repeat=3.
    pub event_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypedBlock {
    StdoutChunk(Vec<u8>),
    StderrChunk(Vec<u8>),
    ExitStatus(i32),
}

/// Versioned state with Rust's read/write synchronization for owned payloads.
/// A sequence retry cannot make concurrent non-atomic String/Vec reads safe.
/// Use try_read_state on render threads to skip a busy update without blocking.
pub struct HermesChannel<T> {
    payload: RwLock<T>,
    generation: AtomicU64,
}
impl<T> HermesChannel<T> {
    pub fn new(initial: T) -> Self {
        Self {
            payload: RwLock::new(initial),
            generation: AtomicU64::new(0),
        }
    }
    pub fn sync_state(&self, value: T) {
        let mut guard = self.payload.write().unwrap_or_else(|p| p.into_inner());
        *guard = value;
        self.generation.fetch_add(2, Ordering::Release);
    }
    pub fn sequence(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}
impl<T: Clone> HermesChannel<T> {
    pub fn read_state(&self) -> T {
        self.payload
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn try_read_state(&self) -> Option<T> {
        match self.payload.try_read() {
            Ok(guard) => Some(guard.clone()),
            Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner().clone()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }
    pub fn snapshot(&self) -> (u64, T) {
        let guard = self.payload.read().unwrap_or_else(|p| p.into_inner());
        (self.sequence(), guard.clone())
    }
}

/// Create a fresh, empty typed transport with both endpoints owned by the caller.
pub fn init_ipc<const N: usize>() -> (
    ring_buffer::Producer<TypedBlock, N>,
    ring_buffer::Consumer<TypedBlock, N>,
) {
    ring_buffer::RingBuffer::new().split()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn versioned_owned_state_is_consistent_under_multiple_writers() {
        let channel = Arc::new(HermesChannel::new(vec![String::from("initial"); 32]));
        std::thread::scope(|scope| {
            for writer in 0..4 {
                let channel = &channel;
                scope.spawn(move || {
                    for n in 0..1000 {
                        channel.sync_state(vec![format!("{writer}:{n}"); 32]);
                    }
                });
            }
            for _ in 0..4 {
                let channel = &channel;
                scope.spawn(move || {
                    for _ in 0..1000 {
                        let (generation, state) = channel.snapshot();
                        assert_eq!(generation % 2, 0);
                        assert!(state.iter().all(|s| s == &state[0]));
                    }
                });
            }
        });
        assert_eq!(channel.sequence(), 8000);
    }
    #[test]
    fn render_read_can_skip_a_writer() {
        let channel = HermesChannel::new(String::from("value"));
        let writer = channel.payload.write().unwrap();
        assert_eq!(channel.try_read_state(), None);
        drop(writer);
        assert_eq!(channel.try_read_state().as_deref(), Some("value"));
    }
}
