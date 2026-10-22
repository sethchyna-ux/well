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
| `well-render` | GPU-backed terminal rendering |
| `well-config` | Application configuration |
| `well-ipc` | Typed terminal messages and state/queue primitives |
| `well` | Native window and application entry point |

These crates are at different stages of implementation and are not all connected to the desktop application yet. In particular, the current application entry point opens a window and clears it; it does not yet provide a complete interactive terminal or agent interface. The subsystems and AgentOS capabilities described above are project goals, not claims of finished functionality.

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
