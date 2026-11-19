# Well architecture

This document describes the current implementation boundaries and architectural evolution of Well.

## Production spine

### Desktop host & Windowing

The GPUI app in `crates/well-gpui` owns the desktop event loop, windowing (`gpui`), keyboard routing, clipboard integration, and native rendering. It coordinates:

- **`TabBar` & `PaneManager`** (`src/tab_bar.rs`, `src/pane_manager.rs`): Legacy binary-tree tiling layout engine supporting arbitrary horizontal and vertical splits (`Cmd+D`, `Cmd+Shift+D`), multi-session tab lifecycle (`Cmd+T`, `Cmd+W`), tab navigation (`Cmd+[` / `Cmd+]`), pane hit-testing, and dynamic tree consolidation upon closing panes.
- **Active PTY Multiplexing**: Every leaf pane owns an independent `Arc<PtySession>` and scrollback matrix, multiplexed through `state.active_pty()`.
- **Mneme Composition Overlay**: Inline multi-line editing buffer (`Ctrl+E` toggle) backed by `crates/well-editor`'s rope structure, capturing multi-line commands with `Ctrl+Enter` execution and `Esc` dismissal.

### Shell and PTY

`crates/well-shell` contains the canonical interactive terminal path:

- `pty::PtySession` owns interactive shell spawning, PTY resize/write/read, `vt100` parser updates, and terminal query responses (DA1, DA2, DSR, bracketed paste).
- `engine::ExecutionRouter` is a dual-path typed-block/native-command router used by server and headless paths.

### Rendering & GPU Graphics Protocol

`crates/well-render` converts parsed terminal state into renderable cells on top of `wgpu`:

- **Text & Grid Rendering**: Font rasterization and glyph atlas powered by `cosmic-text` and `wgpu`.
- **Direct GPU Kitty Graphics Pipeline**: Direct GPU texture pipeline (`ImagePipeline`) supporting base64-decoded Kitty graphics escapes (`\x1b_G...;payload\x1b\`). Allocates dedicated GPU texture bind groups, filters with linear samplers, dynamic quad instance vertex buffers, and renders inline images underneath or over text cells with strict payload size bounds.

### Configuration & Shortcuts

`crates/well-config` provides versioned configuration schemas, Hyprlang import/export, and runtime-backed shortcuts for tab management, pane splitting, navigation, font sizing, and Mneme composition.

### Security & AI Safety

`crates/well-llm` implements opt-in AI command translation and image generation:

- **OS credential storage**: Credentials (`WELL_GEMINI_KEY`, `WELL_OPENAI_KEY`, `WELL_HF_TOKEN`) are securely stored in the macOS Keychain (`security-framework`) via `crates/well-llm/src/keychain.rs`.
- **Destructive Command AST Gate**: The composer validates current text at submission. Commands classified as destructive by Pythia are routed through Mneme rather than executed directly. Mneme uses an explicit Shift+Enter override. These heuristics are warnings, not a shell sandbox.

### IPC, Server & Historical Replay

- `crates/well-ipc`: Typed blocks, lock-free ring-buffer primitives, and JSON-RPC dispatching (`surface.switch_tab`, `surface.split`).
- `crates/well-server`: Headless daemon streaming length-delimited `TypedBlock` chunks over standard streams.
- `crates/well-history`: Sandboxed process replay with stdin pipe forwarding and memory-mapped journals.
- `crates/well-ssh`: Native SSH client and server engine backed by `russh` and `SystemKeyringProvider`.
- `crates/well-wasm`: Sandboxed WebAssembly extension host for deterministic plugin execution via `wasmi`.

## Unified Rust Frontend & GPUI Architectural Evolution

### Retirement of the Flutter/Dart FFI Bridge

The experimental Flutter/Dart mobile FFI bridge (`app/` and FFI texture serialization) is formally retired from the primary desktop architecture:

1. **Zero-Copy Performance**: PTY output streams at gigabytes per second during heavy shell operations (e.g. `cat large.log`, `cmatrix`). Crossing cross-language FFI boundaries and copying RGBA texture buffers between Rust and Flutter introduced unacceptable latency, memory bandwidth pressure, and GC stutter.
2. **Single-Ecosystem Simplicity**: Consolidating the GUI entirely within native Rust eliminates double-toolchain overhead (Dart SDK, Flutter engine, Gradle/CocoaPods) and ensures unified type safety and memory profiling.

### Current GPUI pipeline and retained legacy code

The packaged app runs `well-gpui`, with GPUI rendering and independent `PtySession`
instances. Its binary imports the testable library modules. The root winit/wgpu/
egui frontend remains in the workspace for compatibility and regression coverage;
it is not the packaged production entry point. Parity must be verified before removal.

### AgentOS boot ownership

`boot/agentos` owns the x86_64 EFI loader migrated from AgentOS. AgentOS retains its
kernel and canonical `microvm/boot/protocol.zig`. Its kernel-only build target feeds
Well's loader; its full build also consumes Well's canonical loader source. The
`well-boot` companion embeds that kernel, verifies SHA-384, exports a FAT ESP volume,
and supervises QEMU or the existing pinned DHCP/TFTP engines. It serves only named
artifacts and status over HTTP. Hash checks establish payload integrity, not hardware
attestation. The GPUI boot panel dispatches visible commands into a terminal pane.

### Incoming SSH hosting

`well-ssh::server::SshHost` owns a dedicated worker/runtime and real listener. It
uses a persisted private host key, authorized public keys, optional memory-only
password verification, and PTY channel bridging. It runs as the local user; it is
not a container or privilege sandbox. The GUI reports actual listener state and
requires stopping the host before changing authentication settings.

## Execution Model Principles

1. `PtySession` is the source of truth for interactive terminal behavior.
2. Renderer state is derived directly from the PTY parser/screen state.
3. Zero placeholders in production paths: all features must compile, pass tests, and gracefully handle edge cases.
4. Telemetry and crash logs are strictly local-only to respect user privacy.
