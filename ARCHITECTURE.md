# Well architecture

This document describes the current implementation boundaries. It intentionally separates production-spine components from experimental subsystems.

## Production spine

### Desktop host

The root `well` binary in `src/` owns the desktop event loop, windowing, input routing, clipboard helpers, a single interactive terminal session, and integration between the PTY session, renderer, configuration panel, and AI client.

### Shell and PTY

`crates/well-shell` contains the canonical interactive terminal path:

- `pty::PtySession` owns interactive shell spawning, PTY resize/write/read, `vt100` parser updates, and terminal query responses.
- `engine::ExecutionRouter` is a typed-block/native-command experiment used by server/headless paths. It is not the primary desktop terminal runtime.
- `MetisExecutor` is a simple non-interactive command executor and history trie. It should not define desktop terminal semantics.

### Rendering

`crates/well-render` converts parsed terminal state into renderable cells and owns renderer-side helpers. Effects such as CRT shaders and density scrollbars should remain optional until the basic terminal rendering path is correct and stable.

### Configuration

`crates/well-config` owns the `egui` control panel and Hyprlang import/export. The production goal is a stable, versioned config schema with safe fallback behavior.

### IPC and server

`crates/well-ipc` contains typed blocks, ring-buffer primitives, JSON-RPC/MCP experiments, and shared config payloads.
The desktop runtime currently supports `system.ping`, agent lifecycle events, and direct `shell.input` delivery to the single active PTY. RPCs for tabs, panes, and Mneme editor insertion return explicit unsupported errors until those runtimes exist.

`crates/well-server` is a headless command execution/server experiment. It is useful for protocol development but should not block desktop terminal stabilization.

## Experimental subsystems

- `crates/well-llm`: AI translation/image generation providers and offline command rules.
- `crates/well-prompt`: Astraea prompt state/compiler. It can refresh basic Git branch state without spawning `git`, and FFI uses it for standalone prompt rendering; it does not replace the user's interactive shell prompt in the desktop PTY.
- `crates/well-editor`: Mneme rope editor. FFI exposes a standalone editor buffer, but the desktop terminal does not yet intercept live shell input for inline composition.
- `crates/well-ffi` plus `app/`: Flutter/mobile bindings and Android packaging.
- `crates/well-ssh`: SSH/keyring experiments.
- `crates/well-wasm`: WebAssembly plugin host.
- `k8s/`: cloud runner manifests.
- Firebase/Data Connect files: sync/account/profile experiments.
- Multi-session tabs/panes, Kitty image rendering, and VCR sandbox replay are sketches only; they are not exposed by the production desktop UI.

These should be treated as opt-in or experimental until the desktop terminal is daily-driver reliable.

## Execution model decision

For production v1:

1. `PtySession` is the source of truth for interactive terminal behavior.
2. Renderer state is derived from the PTY parser/screen state.
3. Command execution outside the PTY is a helper/server capability, not a replacement for shell semantics.
4. AI-generated commands must be reviewed/confirmed before execution when destructive or ambiguous.

## Reliability principles

- Runtime code should return errors or typed failure blocks instead of panicking.
- Generated artifacts should be produced by scripts and excluded from source control.
- Packaging must be reproducible from a fresh clone.
- Documentation must describe current behavior separately from future plans.
