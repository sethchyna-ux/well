# Well — a local AgentOS workspace

Well is an experimental, Rust-based workspace for working with AI agents through a native terminal environment. Its goal is to bring the shell, terminal UI, editing tools, configuration, and local automation interfaces together so an agent and its user can work in the same environment.

Well is a desktop application and agent workspace, not an operating-system kernel or a distributed-computing platform. The project is under active development; the current code is a prototype and is not yet a release-ready terminal or AgentOS.

## Architecture

The Cargo workspace is organized into focused crates:

| Crate | Responsibility |
| --- | --- |
| `well-shell` | Shell execution, command history, keyboard input, and PTY support |
| `well-prompt` | Prompt generation |
| `well-editor` | Inline text editing |
| `well-render` | Standalone glyph-atlas renderer (not yet used by the app) |
| `well-config` | Application settings model and egui configuration panel |
| `well-ipc` | Typed terminal messages and state/queue primitives |
| `well` | Native window and application entry point |

The desktop application now starts a local PTY shell, renders its visible screen and cursor in the native window, forwards keyboard and IME text input, and resizes the PTY with the window. The display is currently monochrome and does not yet support mouse selection, clipboard integration, or all terminal protocols, so it is an early interactive terminal rather than a complete terminal emulator or agent interface. Other crates remain standalone and are not yet fully integrated.

The application uses egui for the current text UI. Its WGPU renderer dependency must stay compatible with the workspace's WGPU version.

The prompt crate currently compiles deterministic templates with `{cwd}`, `{exit_code}`, `{duration_ms}`, `{jobs}`, `{vim_mode}`, and `{character}` placeholders. It does not invoke shell commands, load Starship configuration, or use a memory-mapped state vector.

The configuration crate exposes an egui panel that edits in-memory terminal preferences. Saving and loading those preferences, and connecting the panel to the main application window, are not implemented yet.

## Build

Requirements: a Rust toolchain with Cargo and the platform dependencies required by `winit` and `wgpu`.

```sh
cargo check --workspace
cargo test --workspace
cargo run --bin well
```

## Project direction

The intended AgentOS workflow is local-first: keep the user's shell and workspace in their control while making reliable, explicit interfaces available to agents. Priorities include safe shell and IPC behavior, a responsive terminal UI, useful editing and prompt features, and local automation with clear capability boundaries.

The implementation should be treated as a work in progress. Consult the source and tests for current behavior; design notes and architectural sketches in this repository may describe capabilities that have not been implemented.
