# 📨 well-ipc: Hermes Zero-Copy IPC & Caduceus RPC

The `well-ipc` crate implements the high-speed inter-thread and inter-process communication fabric for Well.

---

## Subsystems

### 1. Hermes Seqlock (`HermesChannel` / `HermesSeqlock`)
* **Role**: Coordinates lock-free state synchronization between the GUI configuration thread (Theia) and concurrent worker/rendering threads (Atlas & Orpheus).
* **Guarantees**:
  * Lock-free atomic sequence counter (`AtomicU64`) using `SeqCst` ordering.
  * Zero-blocking reads: Readers (e.g. 250 FPS GPU frame render passes) read state in pure spin-loop validation cycles without waiting on mutexes or write locks.
  * Dynamic sequence telemetry via `.sequence()`.

```rust
use well_ipc::{HermesChannel, TheiaConfigPayload};

let channel = HermesChannel::new(TheiaConfigPayload::default());

// Writer:
let mut state = channel.read_state();
state.background_opacity = 0.85;
channel.sync_state(state);

// Reader (Metal draw pass):
let current_config = channel.read_state();
```

### 2. Caduceus RPC (`CaduceusServer`)
* **Role**: In-process asynchronous JSON-RPC 2.0 protocol engine for AI agent orchestration and remote tooling.
* **Capabilities**:
  * Registers custom RPC methods (`echo`, `eval`, `inspect_metrics`).
  * Non-blocking Tokio asynchronous request-response queues.

---

## Testing

```bash
cargo test -p well-ipc
```
