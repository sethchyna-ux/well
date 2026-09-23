# Well architecture

This document describes the current implementation boundaries and architectural evolution of Well.

## Production spine

### Desktop host & Windowing
The root `well` binary in `src/` owns the desktop event loop, windowing (`winit`), keyboard routing, clipboard integration, and native rendering. It coordinates:
- **`TabBar` & `PaneManager`** (`src/tab_bar.rs`, `src/pane_manager.rs`): Production-grade binary-tree tiling layout engine supporting arbitrary horizontal and vertical splits (`Cmd+D`, `Cmd+Shift+D`), multi-session tab lifecycle (`Cmd+T`, `Cmd+W`), tab navigation (`Cmd+[` / `Cmd+]`), pane hit-testing, and dynamic tree consolidation upon closing panes.
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
- **OS Keychain Integration**: Credentials (`WELL_GEMINI_KEY`, `WELL_OPENAI_KEY`, `WELL_HF_TOKEN`) are securely stored in the macOS Keychain (`security-framework`) via `crates/well-llm/src/keychain.rs`.
- **Destructive Command AST Gate**: All suggested shell commands undergo AST validation; destructive operations (`rm -rf`, `dd`, `mkfs`, fork bombs) are blocked behind mandatory typed confirmation modals.

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

### Current Pipeline & Migration Path to GPUI
- **Current Pipeline**: Unified single-binary architecture using `winit` + `wgpu` + `egui` + custom shaders. All UI chrome (tab bars, status lines, split dividers, settings modal, Mneme overlay) and the terminal grid share the same GPU device and swapchain.
- **GPUI Roadmap**: For future major versions, Well is architected to transition its UI chrome to **GPUI** (the declarative GPU-accelerated UI framework developed by Zed). This provides native macOS look-and-feel, retained-mode declarative views, sub-millisecond layout passes, and seamless integration with our existing `wgpu`/PTY core.

## Execution Model Principles

1. `PtySession` is the source of truth for interactive terminal behavior.
2. Renderer state is derived directly from the PTY parser/screen state.
3. Zero placeholders in production paths: all features must compile, pass tests, and gracefully handle edge cases.
4. Telemetry and crash logs are strictly local-only to respect user privacy.

