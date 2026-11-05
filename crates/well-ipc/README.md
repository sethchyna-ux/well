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

* **Role**: In-process asynchronous JSON-RPC 2.0 protocol engine for local automation against the current desktop runtime.
* **Capabilities**:
  * `system.ping` health check.
  * `system.capabilities` machine-readable active/unsupported surface manifest.
  * `shell.input` delivery to the single active desktop PTY.
  * `surface.switch_tab` and `surface.split` dispatch through the desktop host loop.
  * Explicit unsupported errors for editor insertion/diff application, profile save/load RPC, and sandboxed agent workspaces until those runtimes are exposed.
  * Non-blocking Tokio asynchronous request-response queues.

The capability manifest is intentionally conservative. It describes the current
desktop automation surface, not roadmap intent. In particular, GPU control is
limited to the local Orpheus `wgpu` renderer; remote GPU orchestration is not an
active Hermes surface.

The RPC test suite treats this manifest as a contract: every active method must
dispatch or return locally, and every deferred method listed in
`unsupported_methods` must return an explicit `-32004` unsupported error without
touching the host loop.

---

## Testing

```bash
cargo test -p well-ipc
```
