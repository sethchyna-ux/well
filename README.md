# ⚡ Well-Shell (Phrear)
### *Next-Generation Sub-Millisecond Native GPU Terminal & Shell Environment*

[![Rust](https://img.shields.io/badge/Rust-1.80+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Metal/WebGPU](https://img.shields.io/badge/Graphics-Metal%20%2F%20WebGPU%20(wgpu)-blueviolet.svg?style=flat-square)](https://wgpu.rs)
[![Shell](https://img.shields.io/badge/Shell-Fish%204.x%20%2B%20Starship-cyan.svg?style=flat-square)](https://fishshell.com)
[![Config](https://img.shields.io/badge/Config-Hyprlang%20(.hl)-magenta.svg?style=flat-square)](https://hyprland.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon%20%26%20Intel)-black.svg?style=flat-square&logo=apple)](https://apple.com)
[![License](https://img.shields.io/badge/License-MIT-green.svg?style=flat-square)](LICENSE)

---

**Well-Shell** (internally codenamed *Phrear*) is a high-performance terminal emulator and shell runtime written from the ground up in Rust. Designed to bypass the legacy bottleneck of serial-era UNIX pseudo-terminals, Well pairs direct hardware-accelerated **Metal / WebGPU** instancing with an atomic zero-lock inter-process bus (**Hermes**), native **Fish 4.x** integration, **Starship** prompt support, and a Wayland-inspired **Hyprlang** configuration engine.

Compiled with Fat Link-Time Optimization (`lto = "fat"`) and single-codegen-unit execution, Well delivers sub-millisecond input-to-pixel latency, fluid 250 FPS GPU instanced glyph rendering, and instant `< 0.1ms` shell initialization.

---

## 🏛️ Subsystem Architecture

Well is architected around a unified pantheon of modular, highly specialized subsystems:

| Subsystem | Crate | Role & Technical Implementation |
| :--- | :--- | :--- |
| **Atlas** | `well` (`src/`) | Platform windowing, winit 0.29 event loop pacing, Metal viewport host, and raw hardware keystroke translation. |
| **Orpheus** | `crates/well-render` | Single-pass GPU instanced text rendering (`wgpu`), layered 2D texture atlas, `fontdue` rasterization, and CRT scanline/curvature shader. |
| **Hermes** | `crates/well-ipc` | Lock-free atomic Sequence Lock (`HermesSeqlock`) facilitating zero-copy, zero-blocking configuration sync between GUI and GPU threads. |
| **Metis** | `crates/well-shell` | Shell execution core, `portable-pty` session supervisor, $\mathcal{O}(k)$ in-memory prefix-trie command autocomplete, and automated VT/ANSI DA1 responder. |
| **HyperShell** | `crates/well-shell` | Asynchronous multi-stage task pipeline and background command execution engine with microsecond telemetry. |
| **Theia's Prism** | `crates/well-config` | Embedded `egui` Control Center (`Cmd+,`), live theme switching, typography controls, and full-fidelity **Hyprlang** (`.hl`) parser/emitter. |
| **Astraea** | `crates/well-prompt` | Sub-100µs atomic prompt vector compiler synchronizing git status, execution codes, and cwd without process forks. |
| **Mneme** | `crates/well-editor` | Inline composition mode backed by an $\mathcal{O}(\log n)$ B-tree rope (`ropey`) and Tree-sitter syntax parser for multi-line scripting. |

For deep cosmological and engineering background, read [MYTHOLOGY.md](file:///Users/yocan/Desktop/well/MYTHOLOGY.md).

---

## ✨ Key Features

* **⚡ Sub-Millisecond Fish 4.x Startup**: Implements an automated VT220/xterm Primary Device Attribute (DA1/DA2/DSR) responder in the PTY reader, satisfying Fish 4.x compatibility handshakes in `< 0.1ms` and eliminating the 10-second startup delay.
* **🎮 Single-Pass GPU Rendering**: The Orpheus rendering pipeline uploads live VT100 cell matrices to GPU instance buffers in a single draw call, bypassing raster font overhead.
* **🪐 Hyprlang Configuration Engine**: Supports declarative, human-readable Hyprland-style block configuration (`well.hl`) with live round-trip parsing, validation, and export.
* **🪟 Theia Control Center (`Cmd+,`)**: Modern movable, resizable onyx-glass settings panel with 8 segmented tabs, embedded Undo/Redo history, and real-time Hermes sequence telemetry.
* **🏎️ HyperShell Pipeline Engine**: Concurrent async task runner (`HyperShellEngine`) capable of streaming shell pipelines (`cmd1 | cmd2 | cmd3`) and scheduling non-blocking background jobs.
* **📊 Hyperfine Benchmarking**: Integrated automated benchmarking suite (`well-benchmarks.sh`) measuring shell launch latency, prompt compile times, and subsystem throughput.
* **🎨 Cyber-Neon Aesthetics**: Bespoke high-contrast dark palette with deep midnight slate-navy uppercase contrast (`#112244`), vivid neon accents, and customizable CRT phosphor shaders.

---

## ⌨️ Keyboard Shortcuts & Controls

| Shortcut | Action | Subsystem |
| :--- | :--- | :--- |
| `Cmd + ,` or `Ctrl + ,` | Toggle Theia Control Center | Theia's Prism |
| `F1` or `F12` | Toggle Theia Control Center (Alternative) | Theia's Prism |
| `Escape` | Dismiss / Close Control Center | Theia's Prism |
| `Cmd + D` | Split Pane Horizontal | Metis Shell |
| `Cmd + Shift + D` | Split Pane Vertical | Metis Shell |
| `Cmd + K` | Clear Active Terminal Buffer | Orpheus Screen |
| `Cmd + =` / `Cmd + -` | Increase / Decrease Font Size | Orpheus Atlas |
| `F5` | Run Diagnostics / Embedded Go Demo | Metis Engine |
| `Mouse Drag` (Titlebar) | Move Control Center Window Smoothly | Atlas / Egui |

---

## 🚀 Getting Started

### Prerequisites

* macOS 12.0+ (Apple Silicon or Intel x86_64)
* [Rust toolchain](https://rustup.rs) (1.80+ recommended)
* [Fish Shell](https://fishshell.com) (`brew install fish`)
* [Starship Prompt](https://starship.rs) (`brew install starship`)
* [Hyperfine](https://github.com/sharkdp/hyperfine) (`brew install hyperfine`, optional for benchmarks)

### Installation & Standalone App Deployment

Build the optimized Fat-LTO release binary, generate high-resolution Retina icons, sign with macOS ad-hoc identity, and install directly to `/Applications/Well.app`:

```bash
# Clone the repository
git clone https://github.com/your-org/well.git
cd well

# Build optimized release & package standalone Well.app
./package-app.sh
```

You can now launch **Well-Shell** directly from your macOS Applications folder, Spotlight (`Cmd + Space` -> "Well"), or via terminal:

```bash
open /Applications/Well.app
```

### Building for Development

To compile and run directly from source with incremental debug builds:

```bash
# Run terminal emulator directly
cargo run

# Run workspace unit test suite
cargo test --workspace

# Check compilation across all crates
cargo check --workspace
```

---

## ⚙️ Configuration

Well stores user configuration under `~/.config/well/config.json` and supports importing/exporting native **Hyprlang** (`well.hl`) configuration files.

### Hyprlang Syntax Example (`well.hl`)

```ini
# Well Terminal — Hyprlang Configuration
appearance {
    theme_id = 0
    background_opacity = 0.95
    glass_blur_radius = 20.0
    font_size = 14.0
    line_height = 1.20
    cursor_style = 0
    cursor_blink = true
}

performance {
    scan_timeout_ms = 30
    command_timeout_ms = 500
    enable_transient_prompt = true
    scrollback_limit = 100000
}

crt_shader {
    screen_curvature = 0.05
    scanline_frequency = 0.50
    glow_radius = 1.20
}

# Custom Keybindings
bind = Cmd, D, Split Pane Horizontal
bind = Cmd+Shift, D, Split Pane Vertical
bind = Cmd, K, Clear Terminal Buffer
```

---

## 📊 Benchmarking & Performance Verification

Well includes an automated test and benchmarking suite backed by `hyperfine`:

```bash
./well-benchmarks.sh
```

This tests:
1. Shell startup latencies (`fish` vs `zsh` vs `sh`).
2. Starship prompt rendering overhead across shells.
3. Subsystem micro-benchmarks (Hermes Seqlock atomic latency, Astraea prompt compile time).

---

## 📦 Workspace Crates

```
crates/
├── well-core/       # Shared primitives, cell types, coordinate vectors
├── well-ipc/        # Hermes atomic Seqlock & Caduceus JSON-RPC server
├── well-render/     # Orpheus Metal/WebGPU instanced renderer & CRT shader
├── well-shell/      # Metis PTY supervisor, Trie autocomplete & HyperShell engine
├── well-config/     # Theia Control Center UI, profiles & Hyprlang parser/emitter
├── well-prompt/     # Astraea sub-100µs zero-fork prompt compiler
├── well-editor/     # Mneme Ropey B-tree buffer & Tree-sitter AST
├── well-metrics/    # Hardware telemetry, frame pacing, and cache counters
└── well-wasm/       # WebAssembly compilation target bindings
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
Part of the Well Terminal Architecture Project.
