//! well-core: Core Foundation & Subsystem Coordination Engine for Well
//!
//! Provides the primary terminal session coordinator, zero-allocation ring buffers
//! for scrollback history, and cross-subsystem event dispatch contracts.

pub mod ai;

use std::collections::HashMap;
use std::time::Instant;

/// Zero-allocation, fixed-capacity circular ring buffer for terminal lines or events
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    buffer: Vec<Option<T>>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T> RingBuffer<T> {
    /// Creates a new `RingBuffer` with a fixed maximum capacity
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "RingBuffer capacity must be greater than zero"
        );
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }
        Self {
            buffer,
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Pushes an item into the ring buffer, overwriting the oldest item if full
    pub fn push(&mut self, item: T) {
        self.buffer[self.head] = Some(item);
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Current number of elements stored
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if the ring buffer contains zero elements
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Total capacity of the ring buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears the ring buffer
    pub fn clear(&mut self) {
        for slot in &mut self.buffer {
            *slot = None;
        }
        self.head = 0;
        self.len = 0;
    }

    /// Returns the most recently pushed item, if any
    pub fn latest(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            let index = if self.head == 0 {
                self.capacity - 1
            } else {
                self.head - 1
            };
            self.buffer[index].as_ref()
        }
    }
}

/// Identifies an active terminal surface / tab / split
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

/// Subsystem event contracts dispatched across Atlas, Orpheus, Metis, Mneme, Astraea, and Hermes
#[derive(Debug, Clone)]
pub enum SubsystemEvent {
    PtyOutputReceived {
        surface_id: SurfaceId,
        byte_count: usize,
    },
    UserKeystroke {
        key: String,
        is_ctrl: bool,
        is_alt: bool,
    },
    SplitCreated {
        parent_id: SurfaceId,
        child_id: SurfaceId,
        is_horizontal: bool,
    },
    SurfaceClosed {
        surface_id: SurfaceId,
    },
    ThemeReloaded {
        theme_id: u32,
    },
    AiTranslationCompleted {
        query_len: usize,
        duration_us: u64,
    },
}

/// Information about an active terminal session
#[derive(Debug, Clone)]
pub struct SessionMetadata {
    pub surface_id: SurfaceId,
    pub title: String,
    pub shell: String,
    pub created_at: Instant,
    pub rows: u16,
    pub cols: u16,
}

/// Coordinates active terminal sessions, window surface layouts, and lifecycle events
#[derive(Debug, Default)]
pub struct SessionCoordinator {
    sessions: HashMap<SurfaceId, SessionMetadata>,
    active_surface: Option<SurfaceId>,
    next_id: u64,
}

impl SessionCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawns a new managed terminal surface
    pub fn create_session(
        &mut self,
        title: impl Into<String>,
        shell: impl Into<String>,
        rows: u16,
        cols: u16,
    ) -> SurfaceId {
        self.next_id += 1;
        let id = SurfaceId(self.next_id);
        let meta = SessionMetadata {
            surface_id: id,
            title: title.into(),
            shell: shell.into(),
            created_at: Instant::now(),
            rows,
            cols,
        };
        self.sessions.insert(id, meta);
        if self.active_surface.is_none() {
            self.active_surface = Some(id);
        }
        id
    }

    /// Focuses the given surface
    pub fn focus_surface(&mut self, id: SurfaceId) -> bool {
        if self.sessions.contains_key(&id) {
            self.active_surface = Some(id);
            true
        } else {
            false
        }
    }

    /// Closes and removes a session
    pub fn close_session(&mut self, id: SurfaceId) -> Option<SessionMetadata> {
        let removed = self.sessions.remove(&id);
        if self.active_surface == Some(id) {
            self.active_surface = self.sessions.keys().next().copied();
        }
        removed
    }

    /// Total count of active managed surfaces
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Retrieves the currently focused surface ID
    pub fn active_surface(&self) -> Option<SurfaceId> {
        self.active_surface
    }

    /// Retrieves metadata for a specific surface
    pub fn get_session(&self, id: SurfaceId) -> Option<&SessionMetadata> {
        self.sessions.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_fifo_and_overwrite() {
        let mut rb = RingBuffer::<&str>::new(3);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());

        rb.push("first");
        rb.push("second");
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.latest(), Some(&"second"));

        rb.push("third");
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.latest(), Some(&"third"));

        // Overwrite oldest
        rb.push("fourth");
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.latest(), Some(&"fourth"));
    }

    #[test]
    fn test_session_coordinator_lifecycle() {
        let mut coord = SessionCoordinator::new();
        assert_eq!(coord.session_count(), 0);
        assert!(coord.active_surface().is_none());

        let s1 = coord.create_session("Fish Shell", "/usr/local/bin/fish", 40, 120);
        assert_eq!(coord.session_count(), 1);
        assert_eq!(coord.active_surface(), Some(s1));

        let s2 = coord.create_session("Zsh", "/bin/zsh", 40, 120);
        assert_eq!(coord.session_count(), 2);
        assert_eq!(coord.active_surface(), Some(s1));

        assert!(coord.focus_surface(s2));
        assert_eq!(coord.active_surface(), Some(s2));

        coord.close_session(s2);
        assert_eq!(coord.session_count(), 1);
        assert_eq!(coord.active_surface(), Some(s1));
    }
}
