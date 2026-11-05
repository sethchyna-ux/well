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
}
