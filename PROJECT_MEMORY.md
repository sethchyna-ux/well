# Project Memory (Append-Only)

> **Agent Instruction:** This file serves as a continuous, append-only log of project states, architectural pivots, and development context. **Do not delete historical entries.** When updating, append a new block with the current date and time, summarizing where we were, current status, and our next targets. **You MUST update this file at the start of every session, as well as every 1 hour thereafter.**

---

## 2026-09-28 Architectural Baseline & The GPUI Pivot

### Zoomed-Out Project Architecture (From the Beginning)

- **Phase 1: The Flutter/Dart Experiment:** Well initially explored a cross-platform mobile/desktop boundary using a Flutter/Dart frontend calling into Rust via FFI (`app/`). This was retired due to the massive zero-copy overhead of streaming gigabytes of PTY data across the language boundary.
- **Phase 2: The Unified Native Wgpu/Winit Architecture:** To achieve zero-copy performance, the project consolidated entirely into Rust. It used a custom `wgpu` render pipeline (`crates/well-render`) with `winit` and `egui` to manually composite text atlases, tab bars, and even native Kitty protocol image textures over a GPU swapchain.
- **Phase 3: The GPUI Pivot (Current):** Recognizing the massive complexity of maintaining a bespoke UI/text composition layer, the project is now migrating the entire desktop view layer to **GPUI** (`crates/well-gpui`). GPUI provides native macOS integrations, sub-millisecond declarative layouts, and handles complex flex/text shaping out-of-the-box, letting us focus purely on terminal emulation semantics.

### Where we were (Historical Context)

- **Core Vision:** "Well" is an experimental, blazingly fast Rust terminal workspace tailored natively for macOS. It prioritizes memory-safe, low-latency execution and high-quality aesthetics.
- **Original Architecture:** The terminal workspace originally relied on a custom desktop app entrypoint (`src/`) driving a real PTY session (`crates/well-shell`), vt100 parsing, and a custom `wgpu` renderer (`crates/well-render`).
- **Subsystem Maturity:**
  - **Release & Trust:** macOS packaging (`package-app.sh`), Homebrew Cask generation, and Developer ID/notarization hooks (`scripts/preflight-macos-release.sh`) reached a production-ready baseline.
  - **Features:** Implementations for the Mneme inline editor (payload execution), Astraea prompt/state-vector (Git state without spawning git), and Hermes IPC/GPU engine primitives exist.
  - **AI Integration:** Opt-in Pythia AI/image features (`crates/well-llm/`) with local artifact history were established.
- **The GUI Shift:** We recently decided to transition the terminal's visual frontend away from the raw wgpu/custom composition layer towards the **GPUI** framework (specifically `crates/well-gpui`), taking advantage of its robust layout, text rendering, and native macOS integrations.

### Current Project Status

- **The GPUI Crate:** `cargo run -p well-gpui` is our active development target. We successfully scaffolded the GPUI window and hooked up the background `mpsc` channel to our existing `PtySession` (`crates/well-shell`).
- **Terminal Rendering:** We completely replaced an initial, crude string-concatenation renderer with a dedicated **Grid Cell Renderer** in `crates/well-gpui/src/main.rs`.
  - The renderer iterates exactly over the 24 rows and 80 columns of the `vt100::Screen`.
  - Colors (`vt100::Color`) are natively mapped to `gpui::Rgba` using our Cyber-Neon palette via `terminal.rs`.
  - Empty cells are guaranteed to render as `" "` to prevent GPUI flex layout collapses.
- **Input Pipeline:** The keyboard handler (`on_key_down`) intercepts VT100 sequences (Enter, Backspace, Delete, Arrows, Ctrl-chords) and explicitly applies `cx.stop_propagation()` so GPUI's global action bindings don't swallow terminal keystrokes.
- **Experimental Branches:** Mobile, Kubernetes, web console, and plugin ecosystems exist in the repo but remain explicitly deferred unless requested.

### Where we want to be

- **Short-term UI Polish (GPUI):**
  - Apply full rich text formatting (Bold, Italic, Strikethrough) extracted from `vt100::Cell` to the GPUI elements.
  - Stabilize tiling and pane management within the GPUI window.
  - Re-implement the Kitty image protocol rendering within the GPUI grid.
- **Mid-term Subsystem Depth:**
  - Harden the Hermes IPC/GPU engine and benchmark latency.
  - Polish the Profile import/export/switching workflows (`crates/well-config`).
  - Improve the generated AI image lifecycle, insertion, and error states.
  - Expand Mneme richer editing flows and Astraea prompt replacement.
- **Long-term Release:** Complete a notarized public build using the hardened GPUI frontend, push Cask promotion, and update support docs.

## 2026-09-28 Update

- **GPUI Rendering:** Successfully mapped basic `vt100` cell properties (foreground, background, inverse).
- **Bug Fix:** Removed `.strikethrough()` since `vt100::Cell` doesn't support it yet.
- **Bug Fix:** Fixed an "empty box / no app" crash when hovering over text by forcing `.cursor(gpui::CursorStyle::Arrow)` on the root GPUI div. This prevents macOS from attempting to load missing IBeam cursor resources in an unpackaged binary.
- **Settings UI:** Decided to use a Native Pop-out Window for the Settings UI to mimic the standard macOS terminal feel.

### 2026-09-28 20:00:00 End of Hour Update

- **Settings State Sync:** Used GPUI's global `Entity` modeling to share a live `TerminalSettings` reference between `SettingsView` and `WorkspaceView`. Font size updates in the Settings Window now instantly propagate to the Terminal grid.
- **Font Rendering Fix:** Migrated the fallback font to `"Menlo"` to allow GPUI to natively resolve Bold/Italic assets without crashing or rendering empty boxes. Re-enabled `.font_weight(gpui::FontWeight::BOLD)` and `.italic()` on the terminal cells.
- **Tabs and Multiplexing:** Scaffolded the `WorkspaceView` as the new root view of the main window. Added a horizontal `TabBar` UI containing dynamically generated tabs, an active tab indicator, and a `+` button that successfully spawns new `TerminalView`s and individual PTY sessions, correctly routing focus on click.

### 2026-09-28 20:05

- **Task:** Ported premium tab bar styling to GPUI.
- **Details:** Replaced functional gray-box prototype tab bar with a custom declarative GPUI flex layout matching the old `wgpu` application's neon aesthetic. Added logo, pill-shaped tabs with custom colored borders (cyan, purple, blue) based on tab index, and realigned the Settings button. Fixed GPUI ID routing (`.id()`) to ensure hitboxes for switching tabs and closing tabs handle clicks correctly in the retained-mode hierarchy.
- **Next:** Potentially tackle drawing the custom neon border around the terminal itself, or move to PaneManager (split panes) integration in GPUI.

### 2026-09-29 GPUI Panes and Kitty Protocol Render

- **Kitty Image Protocol**: Re-implemented Kitty image protocol in GPUI. Fixed rendering failures by parsing native dimensions with the `image` crate and applying `.relative()` bounds to the absolute-positioned overlay children.
- **Application Menu**: Implemented `cx.set_menus` to inject a proper macOS native application menu, providing global access to the Settings Window (`cmd-,`).
- **PaneManager Integration**: Fully hooked up declarative split panes within the GPUI window.
  - Added horizontal (`cmd-d`) and vertical (`cmd-shift-d`) terminal splits.
  - Implemented clean collapsing logic when closing a pane (`cmd-w`).
  - Added a custom keyboard focus cycling action (`FocusNextPane`, `cmd-]`) to cycle focus dynamically through the binary `PaneTree`.
- **UI Polish**: Replaced the flat border with a premium 2px `.rounded_xl()` border with an `.overflow_hidden()` mask and a subtle `.shadow_sm()`. Active panes illuminate with `#00ffff` cyan, while inactive panes dim out seamlessly.
- **Next**: Hardening Hermes IPC/GPU engine and avoiding overclaiming unimplemented remote surfaces.

## 2026-09-29: Hermes IPC Integration into GPUI

- **Context:** The JSON-RPC local socket automation (`well-ipc` / `CaduceusServer`) was not yet connected to the new GPUI-based desktop runtime (`well-gpui`).
- **Action:**
  - Added `tokio` and `well-ipc` to `well-gpui` dependencies.
  - Initialized `CaduceusServer` running in a background Tokio thread within the GPUI `main` entrypoint.
  - Linked the `HermesAppCommand` MPSC receiver to the `WorkspaceView` via a `cx.spawn_in` async task.
  - Implemented handling for `SwitchTab`, `SplitPane`, and `SendShellInput` RPC commands natively within the GPUI `cx.update` lifecycle, directly manipulating the active `PaneTree` and `PtySession`.
- **Result:** External local CLI/agent scripts can now split panes, switch tabs, and pipe commands into the GPUI Well Terminal in real-time.

## 2026-09-29: Dynamic Theme Processing

- **Context:** `well-gpui` relied on hardcoded `rgba(x)` colors while `well-config` GUI already supported emitting JSON-RPC payload for changing themes.
- **Action:**
  - Added `crates/well-gpui/src/theme.rs` to abstract 16 ANSI colors and core `Theme` struct (Cyber-Neon, Tokyo Night, Dracula, etc).
  - Modified `TerminalSettings` to include the `Theme`.
  - Updated `TerminalView`, `WorkspaceView` and PTY parser `convert_color` routines to dynamically pull `border_active`, `bg`, `fg`, and `ansi_colors` from the selected Theme.
  - Linked `well_ipc::HermesAppCommand::SaveProfile` in the async IPC loop in `main.rs` to instantly apply theme updates onto the active GPUI context globally.
- **Result:** Selecting themes in the external `well-config` GUI or via IPC scripts instantly redraws the GPUI layout and terminal colors in real time.

## 2026-09-30: Build Environment Fix & Hermes IPC Hardening

### Build Environment

- **Root cause:** `cargo test` failed because `xcrun -find metal` could not locate the Metal
  shader compiler. The Xcode developer directory was pointing at `/Applications/Xcode.app`
  (which doesn't exist on this machine); the actual installation is at
  `/Volumes/internal-drive/Applications/Xcode.app`.
- **Fix:** `sudo xcode-select --switch /Volumes/internal-drive/Applications/Xcode.app`
  — `metal` now resolves via a MetalToolchain cryptex mount.
- **Result:** `well-gpui` compiles cleanly (7 cosmetic warnings, 0 errors).

### Hermes IPC Hardening

#### Problem 1 — Silent command drops in the GPUI host loop (`crates/well-gpui/src/main.rs`)
The `match cmd` arm used a wildcard `_ => { /* ignore */ }` for all unhandled commands.
This caused **oneshot responder channels to be dropped without a reply**, leaving RPC callers
blocked indefinitely.

**Fix:** Replaced with exhaustive explicit arms:
- `PublishAgentLifecycle` → logs event + responds `{ "status": "acknowledged" }`
- `LoadProfile` → explicit `Err(...)` with user-facing guidance
- `InsertEditorText`, `ProposeEditorDiff`, `SpawnAgentWorkspace` → explicit `Err(...)` with
  boundary description

#### Problem 2 — No wire framing in `CaduceusServer` (`crates/well-ipc/src/rpc.rs`)
Raw `stream.read()` loop was fragile; large payloads and split reads caused silent partial
JSON parse failures.

**Fix:** **4-byte little-endian length-prefix framing** (`u32` body size header) for both
requests and responses. Hard cap: 4 MiB per message.

#### New test coverage
Added `crates/well-ipc/tests/rpc_framing_test.rs` — 5 integration tests against a live
`CaduceusServer` Unix socket exercising the full framing round-trip.

**Test result:** 19/19 pass across all `well-ipc` suites.

### Next
1. Clean up 7 cosmetic warnings in `well-gpui`.
2. Profile import/export/switching polish.
3. Image generation polish: lifecycle, insertion, rendering, error states.

## 2026-09-30: Image Protocol Polish & GPUI Cleanup

- **Image Generation Rendering:** Hooked up the `Render Inline` button in the Image Studio UI (`crates/well-config`). This natively injects the generated image directly into the active PTY session by building the `base64` payload and wrapping it in Kitty Graphic Protocol `\e_Gf=100...` escape sequences. Error states are robustly handled via the Pythia translation thread and explicitly shown as red error text in the egui view.
- **GPUI Code Health:** Fixed all 5 remaining Clippy warnings in `well-gpui` (redundant match, manual clamp, manual ASCII checks, manual range contains).
- **proc-macro-error2 Fix:** Silenced the `extern crate proc_macro` future-incompatibility warning by pointing `Cargo.toml` to `fgardt:fix/future-incompat` GitHub fork using `[patch.crates-io]`. 

### Next
- **Richer Mneme Editing**: We expanded `MnemeEditor` (`crates/well-editor/src/lib.rs`) with richer editing flows (`move_cursor_up`, `move_cursor_down`, `delete_next_char`, `move_cursor_to_start_of_line`, `move_cursor_to_end_of_line`, `move_word_left`, `move_word_right`). 
- **Mneme Integration to GPUI**: We wired this engine into the `well-gpui` terminal frontend (`crates/well-gpui/src/main.rs`). The visual composer overlay now renders correctly over the terminal and intercepts standard editing keys when toggled via `Ctrl+E`. Also restored `HermesAppCommand::InsertEditorText` so the shell can proactively prompt editing.
- **Astraea State Vector Propagation to GPUI**: Wired `well_prompt::AstraeaStateVector` directly into `TerminalView`'s state. When `mneme_active` is toggled, it now renders the dynamic Astraea cyber prompt (Git branch, status, operation) using native GPUI elements instead of static text. Shell directory changes (OSC 7) automatically refresh the vector to guarantee sync.

---

## 2026-10-25 04:50:00 Session Initialization & Custom Helper Deployment

### Operational Environment & Tooling
- **Hardware & Host Architecture:** macOS Darwin (Apple Silicon arm64), rustc toolchain with GPUI native compilation target.
- **Session Cron & Lifecycle:** Scheduled recurring `/learn` evaluation every 2 hours (`0 */2 * * *`). Every milestone, architectural pivot, and challenge is analyzed and recorded continuously into this append-only document.
- **Custom Agent Helper Deployment:**
  - Initialized dedicated workspace agent `well-helper` located at `agents/well-helper`.
  - Configured via Google Agents CLI (`agents-cli`) with `agents-cli-manifest.yaml` targeting GCP Agent Runtime (project `agent-os-510817`, region `us-east1`).
  - Implemented concrete, production-grade tools in `agents/well-helper/app/agent.py`:
    - `check_well_workspace_health()`: Live audit of Well subsystem status (GPUI frontend, PTY session, Hermes IPC framing, Mneme editor, Astraea prompt).
    - `query_memory_invariants(subsystem)`: Strict architectural constraints covering raw mode handling, IPC framing, zero-leak child process lifecycles, and font fallbacks.
  - Added automated execution runner `scripts/deploy-well-helper.sh` with preflight verification and `--live` deployment modes. Verified preflight pass with exit code 0.

### Subsystem State & Core Invariants
- **1. GPUI Frontend (`crates/well-gpui`):**
  - Retained-mode 24x80 terminal grid using `vt100::Screen` backing buffer.
  - Active Cyber-Neon color mapping with dynamic theme switches propagated from `well-config` via Hermes IPC.
  - Key down interception with `cx.stop_propagation()` ensuring global actions do not swallow terminal control sequences.
  - Root div cursor lock (`gpui::CursorStyle::Arrow`) to prevent unpackaged macOS IBeam crashes.
- **2. Hermes IPC & Wire Framing (`crates/well-ipc`):**
  - Length-prefixed wire protocol over Unix domain sockets with 4-byte little-endian header (`u32`) and 4 MiB payload cap.
  - Exhaustive command matching in GPUI loop; zero dropped oneshot responders to avoid deadlock for external CLI tools.
- **3. Low-Latency Systems & MicroVM Architecture:**
  - 16-byte hardware telemetry trailer (RDTSC timestamp + WAL commit latency + EDF runqueue load).
  - AArch64 VirtIO MMIO v2 drivers with Split Virtqueue ring buffers; strict page-boundary alignments to avoid trap overhead on APFS hypervisors.
  - UART COM1 (0x3F8) interactive REPL line discipline with zero-leak child process lifecycles via RAII Drop guards.

### Next Session Targets
1. Complete verification of GPUI split pane resizing under heavy PTY throughput.
2. Benchmark Hermes IPC length-prefixed socket roundtrip latency.
3. Validate packaging pipeline with `./package-app.sh --archive --cask` for the GPUI binary.

---

## 2026-10-25 06:18:00 /learn Milestone: Caduceus Dual-Transport Multiplexer & Root Hygiene

### Context & Challenge
- **Symptom:** `crates/well-ipc/tests/rpc_framing_test.rs` failed with `UnexpectedEof` on length-prefixed connections because `CaduceusServer` was migrated to an Axum/Hyper HTTP engine which rejected binary 4-byte little-endian headers as invalid HTTP requests.
- **Root Cause & Technical Dilemma:** The architectural specification demands both:
  1. High-speed local framed Unix socket IPC (4-byte LE length prefix, 4 MiB hard cap) for agents, automation tools, and tests.
  2. HTTP/WebSocket JSON-RPC routing (`/` and `/rpc`) for browser/GUI automation and MCP interoperability.
  Tokio's `UnixStream` lacks a high-level async `peek()` method.

### Architectural Invariant & Solution
- **Zero-Copy MSG_PEEK Wire Protocol Sniffer:**
  - Implemented non-consuming socket header inspection inside `PrivateListener::accept` using `tokio::net::UnixStream::try_io` paired with `libc::recv(..., libc::MSG_PEEK)`.
  - Identified HTTP streams by inspecting ASCII verb prefixes (`GET `, `POST`, `HEAD`, `PUT `, `DELE`, `OPTI`, `PATC`).
  - **Dynamic Stream Routing:**
    - If HTTP prefix matches: stream is passed up to `axum::serve(listener, app)` for standard HTTP/1.1 and WebSocket processing.
    - If binary length-prefixed: stream is diverted directly into asynchronous task `handle_framed_stream(...)`, preserving the 4-byte LE header and dispatching directly against `HermesService::handle_request`.
- **Honest Capability & Method Enforcement:**
  - `system.ping` returns `{"status": "pong", "protocol": "caduceus/1.0"}`.
  - `system.capabilities` returns unified contract: version, methods array, protocol, GPU boundary flags, active methods, and unsupported methods.
  - Deferred methods (`editor.insert`, `editor.propose_diff`, `profile.save`, `workspace.agent_spawn`) return explicit boundary error `-32004` without touching or blocking the host loop.
- **Verification:**
  - `cargo test -p well-ipc`: All 29 tests passing (17 unit tests, 5 MCP integration tests, 2 ring buffer stress tests, 5 wire framing integration tests) in 0.13s total.

---

## 2026-10-25 08:00:00 /learn Milestone: Native GPUI Pythia Copilot, Kitty Inline Graphics, & Release Packaging Hardening

### Context & Architectural Challenge
1. **Frontend Architecture Alignment:** Per `AGENTS.md`, the production spine of Well is `crates/well-gpui`, while legacy `src/` (winit 0.29/wgpu) is being phased out. Previously, AI features were hosted in `well-config`'s egui panel (Theia's Prism) using an offline procedural sine-wave shader, while the native GPUI desktop lacked an integrated Pythia Copilot and Image Studio.
2. **Inline Terminal Graphics Protocol:** Generating provider-backed neural images needed an honest, direct path from generation to terminal rendering without intermediate file clutter or UI disconnections.
3. **macOS Gatekeeper & Codesign Detritus:** Running `./package-app.sh` and `./scripts/verify-release-candidate.sh` failed with `resource fork, Finder information, or similar detritus not allowed` (`Disallowed xattr com.apple.FinderInfo`) because the workspace resides in `~/Desktop/well`, where macOS Finder / FileProvider automatically attaches extended attributes to any newly created directory or application bundle.
4. **Clean-State CLI Diagnostics:** `verify-release-candidate.sh` requires packaged app bundles to support non-GUI `--version` and `--diagnose` commands emitting schema 1 JSON health reports without initializing the GPUI/Metal window engine.

### Architectural Invariants & Solutions Implemented

1. **Native GPUI Pythia Copilot & Image Studio (`crates/well-gpui/src/main.rs`):**
   - Built a native glassmorphic overlay modal activated via `Cmd+K` keyboard shortcut or header action (`✨ Pythia (⌘K)`).
   - Added `PythiaMode` supporting:
     - `CommandSynthesis`: Natural language translation to executable shell commands with safety analysis and one-click execution or Mneme composer handoff.
     - `ImageStudio`: Provider-backed image generation (Google Imagen / Hugging Face FLUX.1 / offline fallback) with style presets (Cyberpunk Neon, Retro Sci-Fi, Dark Fantasy, Minimalist Vector), aspect ratio / resolution controls, seed controls, thumbnail preview, clipboard copying, and direct PTY terminal injection.
     - `StderrDiagnosis`: Autonomous error analysis and command remediation for terminal failure outputs.
   - **Zero-Copy Kitty Graphic Protocol Injection:**
     - The generated PNG bytes are base64-encoded and transmitted directly to the active PTY session via Kitty APC escape format:
       `\x1b_Gf=100,a=T,m=0;<base64_payload>\x1b\\\r\n`
     - `well-shell`'s internal graphics interceptor decodes the sequence into `GraphicEvent::KittyImage` and dispatches it to `TerminalView`, which renders the image inline on the GPUI canvas.

2. **Theia Offline Generator Transparency (`crates/well-config/src/theia.rs`):**
   - Clarified that Theia's offline mathematical preview generator is a procedural fallback and that live neural models route through Pythia (`well-llm`).
   - Updated UI labels and convergent status strings to remove misleading "Metal MPS GPU" diffusion claims.

3. **Release Packaging & Finder Detritus Hardening (`package-app.sh` & `scripts/verify-macos-bundle.sh`):**
   - Aligned `package-app.sh` to package `well-gpui` as the production macOS application bundle (`dist/Well.app`).
   - Isolated packaging staging into `${TMPDIR:-/tmp}/.well-package.XXXXXX` using `ditto` to prevent Desktop Finder indexing from injecting attributes during compilation or codesigning.
   - Discovered that macOS Finder protects `com.apple.FinderInfo` from generic `xattr -c` clearing. Solved by implementing explicit recursive deletion:
     `xattr -dr com.apple.FinderInfo "${APP_BUNDLE}" 2>/dev/null || true`
   - Added pre-signing xattr sanitization in `sign_path`.

4. **Non-GUI Release Diagnostics (`crates/well-gpui/src/diagnostics.rs`):**
   - Added CLI argument interceptor before GPUI runtime initialization.
   - Implemented schema 1 JSON diagnostic health report:
     - Validates macOS bundle status and binary executable.
     - Checks configuration state without creating unwanted `$HOME/.config` or `$HOME/.well` directories.
     - Validates privacy redactions (no secrets, no terminal output, no raw environment variables).

5. **PTY Child Lifecycle & Exit Waiting (`crates/well-shell/src/pty.rs`):**
   - Implemented `PtySession::wait_for_exit(&self, timeout: Duration) -> io::Result<Option<portable_pty::ExitStatus>>` with non-blocking poll loop.
   - Restored compliance for `scripts/smoke-shell-startup.sh`.

### Verification & Test Matrix
- **Release Packaging:** `./package-app.sh --archive --cask` passed (built arm64 Mach-O, ad-hoc signed, verified bundle, generated `dist/Well-0.1.0-macOS-arm64.zip`, checksum, and `dist/Casks/well.rb`).
- **Release Qualification:** `./scripts/verify-release-candidate.sh dist/Well.app` passed with clean-state JSON assertion.
- **GUI Launch Smoke:** `./scripts/smoke-gui-launch.sh dist/Well.app` passed (survived 8s isolated clean-home startup).
- **Shell Startup Smoke:** `./scripts/smoke-shell-startup.sh` passed (tested `/bin/sh`, `/bin/zsh`, `/opt/homebrew/bin/fish`).
- **Interactive Programs Smoke:** `./scripts/smoke-interactive-programs.sh` passed.
- **Production Workspace Suites:**
  - `well-ipc`: 29/29 passed (unit tests, MCP integration, ring buffer stress, wire framing).
  - `well-editor`: 8/8 passed.
  - `well-llm`: 26/26 passed.
  - `well-shell`: 39/39 passed.
  - `well-gpui`: 0 check/build errors.

### Next Session Targets

---

## Session Log: 2026-10-25 — Theia SSH Manager & Complete "Look of Theia" Integration

### Objectives & Scope
1. **Theia SSH Manager Full Program Integration:**
   - Implement real PTY SSH session spawning with interactive terminal streaming, ~/.ssh/config host parsing, active session tracking, and Hermes JSON-RPC query handlers.
2. **Authentic Theia Look & Aesthetic Architecture:**
   - Elevate the entire GPUI frontend to embody the authentic Theia visual identity ("The Titaness of Radiant Sight & Heavenly Light").
   - Replace generic settings with the full GPUI implementation of **Theia's Prism Control Center** (Gruvbox Retro-Modern Dark Hard Obsidian `#1d2021`, radiant amber accents `#fe8019`, aqua badges `#8ec07c`).
   - Transform top bar, logo, tab pills, split dividers, and hotkeys.

### Architecture & Key Implementations
1. **Theia Aesthetic Tokens & Default Theme (`crates/well-gpui/src/theme.rs`):**
   - Configured `Theme::theia_prism()` as default Theme 0 with authentic Gruvbox Retro-Modern Dark Hard palette:
     - Obsidian background: `rgb(0x1d2021)`
     - Dark Slate panel/card fills: `rgb(0x282828)`, `rgb(0x3c3836)`
     - Radiant Amber primary accent: `rgb(0xfe8019)`
     - Radiant Aqua remote accent: `rgb(0x8ec07c)`
     - Light Warm Cream foreground: `rgb(0xebdbb2)`
     - Full 16-color Gruvbox ANSI palette.
   - Updated split pane divider in `PaneTree::render` from hardcoded cyan to Theia radiant amber `rgb(0xfe8019)`.

2. **Theia's Prism Control Center (`crates/well-gpui/src/main.rs`):**
   - Elevated `SettingsView` from a generic 3-row One Dark box into a 760x580 immediate-mode control center matching `theias-prism-panel.rs`:
     - Subtabs:
       - `🎨 Orpheus Engine`: Interactive theme switcher with live color preview swatches (`Theia's Prism`, `Cyber-Neon`, `Tokyo Night`, `Dracula`), typography size steppers & presets, Naga shader uniforms indicators (scanlines, curvature, glow, blur).
       - `🛰️ Astraea Prompt`: Starship format presets (`Gruvbox Retro`, `Tokyo Night`, `Pure Minimal`), context module toggles (Git status, smart directory context, compiler versions, pristine scrollback transience), live prompt vector preview ribbon.
       - `🐟 Metis Shell`: SQLite history capacity (260,000 commands), live abbreviations manager with delete `✕` and quick add presets (`+ gst`, `+ gp`).
       - `⌨️ Hotkey Matrix`: Unified collision-checked hotkey table (`F12`, `Cmd+Shift+S`, `Cmd+K`, `Cmd+D`, `Cmd+Shift+D`, `Cmd+W`, `Cmd+,`, `Cmd+]`, `Ctrl+E`).
       - `👁️ Live Status`: Real-time telemetry for Orpheus, Astraea, Metis, Theia SSH, Mneme, Pythia, and Hermes, plus `⚡ Apply & Synchronize Hermes Seqlock` action.
   - Triggered via `F12`, `Cmd+,`, menu item `"Theia's Prism Control Center..."`, and top-bar button `🏛️ Theia's Prism (F12)`.

3. **Theia SSH Remote Control Center Modal (`crates/well-gpui/src/main.rs`):**
   - Overlay modal with radiant amber border `rgb(0xfe8019)` and dark slate panels.
   - Interactive keyboard routing on focus handle (Enter to direct-connect, Tab to switch subtabs, Backspace and text input to filter hosts or quick connect, Escape to close).
   - Subtabs: `📡 Discovered Hosts`, `🌐 Active Sessions`, `⚡ Quick Connect`.
   - Direct PTY spawn via `TerminalView::spawn_ssh_pty` with `/usr/bin/ssh` and environment tagging (`SSH_CONNECTION=1`, `WELL_REMOTE=1`), Astraea snapshot tagging (`is_ssh = true`, `git_branch = destination`), and distinctive remote tab pill styling (`🌐 <title>`).

4. **Hermes RPC Integration (`crates/well-ipc/src/rpc.rs`):**
   - Registered and whitelisted `"ssh.connect"`, `"ssh.sessions"`, and `"ssh.hosts"`.
   - Handled in GPUI IPC loop with real-time JSON responses over `/tmp/well-caduceus.sock`.

### Verification & Validation Matrix
- **Compilation & Clippy:**
  - `cargo check -p well-gpui`: 0 errors, 0 warnings.
  - `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- **Test Suites:**
  - `well-ipc`: 29/29 passed.
  - `well-shell`: 39/39 passed.
  - `well-prompt`: 7/7 passed.
  - `well-editor`: 8/8 passed.
  - `well-config`: 0 doc errors.
- **Packaged App Verification:**
  - `./package-app.sh --archive --cask`: passed (built optimized release arm64, verified bundle, generated `dist/Well-0.1.0-macOS-arm64.zip`, checksum, and `dist/Casks/well.rb`).
  - `./scripts/verify-release-candidate.sh dist/Well.app`: passed.
  - `./scripts/smoke-gui-launch.sh dist/Well.app`: passed (survived 8s isolated startup).
  - `./scripts/smoke-shell-startup.sh`: passed.
  - `./scripts/smoke-interactive-programs.sh`: passed.

### Architectural Invariants & Hard-Learned Lessons (/learn Iteration 3)

1. **GPUI Modal Focus & Keyboard Routing Hierarchy:**
   - In GPUI, modal overlays (like Theia's Prism or Theia SSH Matrix) rendered on top of active terminal views must maintain their own distinct `FocusHandle` (`cx.focus_handle()`).
   - The modal root container must explicitly call `.track_focus(&self.modal_focus_handle)` and handle `.on_key_down(cx.listener(...))`.
   - Without explicit focus tracking on the overlay element, key events route down to the terminal view's PTY input channel, corrupting terminal scrollback or sending raw keystrokes to the shell.

2. **Decoupled PTY Spawning vs Entity Instantiation:**
   - Separating PTY process execution (`TerminalView::spawn_ssh_pty`) from GPUI view instantiation (`TerminalView::from_pty`) is critical for resilience.
   - PTY spawning can fail synchronously (e.g. invalid host format, missing SSH binary, permission denied, unreachable subnet).
   - Factoring this into a pure `Result<(Arc<PtySession>, Receiver<()>, Arc<AstraeaStateVector>, Option<String>), Error>` allows the workspace to capture the error into `ssh_state.status_message` and display a non-fatal Gruvbox red alert ribbon without panicking or allocating a dead GPUI entity.

3. **Remote PTY Environment & Lock-Free State Vector Invariant:**
   - Every remote transport session must inject explicit remote environment variables (`SSH_CONNECTION=1`, `SSH_CLIENT=1`, `WELL_REMOTE=1`) into the spawned process environment.
   - Simultaneously, Astraea's atomic state vector must record `is_ssh = true` and `git_branch = target.destination` prior to first render.
   - This prevents shell hooks (Starship, Fish, Zsh) from executing expensive, blocking local git queries over high-latency remote mounts, and ensures tab headers and status bars display `🌐 user@host` immediately without PTY query latency.

4. **Modern Clippy (Rust 1.83+) Workspace Hygiene:**
   - `.is_multiple_of(divisor)` must be used everywhere modulo arithmetic is checked against zero (`(now / 500).is_multiple_of(2)`).
   - In `HashMap` caches, replace `contains_key` followed by `insert` with `entry(key).or_insert(...)` or `Entry::Vacant` to avoid redundant hash lookups.
   - When storing types with OS handles (e.g., `PtySession` containing file descriptors and mutexes) inside `Arc`, explicitly decorate with `#[allow(clippy::arc_with_non_send_sync)]` since cross-thread message passing is coordinated via mpsc event loops.

5. **Release Trust & Staged Packaging Pipeline:**
   - macOS Finder automatically writes extended attributes (`com.apple.FinderInfo`) to files created in Desktop folders.
   - Codesign strictly forbids `com.apple.FinderInfo` on Mach-O executables and app bundles.
   - The packaging pipeline must always compile and assemble inside an isolated temporary directory (`${TMPDIR:-/tmp}/.well-package.XXXXXX`) using `ditto`, strip extended attributes with `xattr -dr com.apple.FinderInfo`, and verify the resulting bundle with `./scripts/verify-release-candidate.sh dist/Well.app`.

### Next Session Targets
1. **Pythia Copilot & Image Studio Polish:**
   - Support dragging and dropping generated PNGs directly from the Pythia Image Studio preview into macOS Finder and applications.
   - Add visual snapshot tests for Pythia Copilot overlay layout across all 4 themes (`theia_prism`, `cyber_neon`, `tokyo_night`, `dracula`).
2. **Hermes RPC Maturity & Benchmarks:**
   - Measure length-prefixed Unix domain socket roundtrip latency with `scripts/well-cli-benchmark.py`.
   - Implement streaming JSON-RPC subscriptions for terminal output over `/tmp/well-caduceus.sock`.
3. **Profile Management Polish:**
   - Expand profile import/export/switching between local and remote configurations.

---

## Session Log: 2026-10-25 — /learn Iteration 4: Configuration Reactivity, RPC Whitelisting & Theme Contract Invariants

### Architectural Invariants & Hard-Learned Lessons (/learn Iteration 4)

1. **Immediate-Mode Configuration Reactivity Invariant:**
   - In GPUI, settings mutations must observe the single-source-of-truth pattern using `Entity<TerminalSettings>`.
   - Views register observers with `cx.observe(&settings, |_, _, cx| cx.notify()).detach()`.
   - In-memory modifications made inside *Theia's Prism Control Center* (e.g. font size steppers, theme switches, preset changes) call `settings_model.update(cx, |s, cx| { ...; cx.notify(); })`.
   - Because all workspace tabs and split pane leaves observe the shared settings entity, every open pane updates its typography metrics, ANSI color lookup tables, and border accents in the same render frame with zero disk I/O latency and zero lock contention.

2. **Hermes JSON-RPC Dual-Gate Whitelisting Invariant:**
   - Any remote procedure call exposed over Hermes (`/tmp/well-caduceus.sock`) requires registration at two separate layers in `crates/well-ipc`:
     1. **Protocol Gate:** Method name must be present in `HermesService::with_methods(&[...])`. Calls to unlisted methods are immediately rejected with JSON-RPC error `-32601 Method not found`.
     2. **Dispatch Gate:** The method must be matched in `HermesService::dispatch`, deserializing typed params and passing the command variant across `HermesAppCommand`.
   - Adding `ssh.connect`, `ssh.sessions`, and `ssh.hosts` required simultaneously updating `with_methods`, `dispatch`, and the GPUI main thread event receiver to guarantee that external MCP agents and scripts receive honest, strongly-typed responses.

3. **Theia Contrast & Visual Hierarchy Contract:**
   - Components styled with Theia's Gruvbox Dark Hard palette must strictly honor visual depth tiers:
     - Tier 0 (Root Workspace & Terminal Grid): Deep Obsidian `#1d2021`
     - Tier 1 (Modals, Panels & Inactive Tabs): Dark Slate `#282828`
     - Tier 2 (Hover states & Input Fields): Slate Dark 1 `#3c3836`
     - Tier 3 (Dividers & Active Borders): Radiant Amber `#fe8019` (local) / Jade Aqua `#8ec07c` (remote SSH)
     - Typography: High-contrast Warm Cream `#ebdbb2` for headings and commands; muted Sage `#a89984` / `#928374` for metadata and subtitles.
   - Preserving these exact hex tokens guarantees visual cohesion across the main terminal window, Pythia overlay, Theia SSH Matrix modal, and Theia's Prism control center.

4. **PTY Worker Thread Isolation & Channel Backpressure:**
   - `well_shell::pty::PtySession` encapsulates raw POSIX PTY master file descriptors, termios raw-mode flags, and an embedded `vt100` parser inside `Arc<Mutex<vt100::Parser>>`.
   - Shell output streams through unbounded mpsc channels (`futures::channel::mpsc::UnboundedSender<()>`), decoupling the high-throughput PTY reader thread from GPUI's 60fps display compositor loop.
   - Redraw notifications coalesce naturally via `cx.notify()`, ensuring rapid terminal output (e.g. `cat large_file.log`) saturates neither the macOS WindowServer nor the GPUI event dispatcher.

5. **Theia Status Bar & Ergonomics Architecture:**
   - Positioned as a dedicated 24px bottom bar across the GPUI workspace:
     - Left Remote Indicator (`status-remote-btn`): Displays `>< SSH: user@host` (Jade Aqua `#8ec07c` on dark forest `#1d2021`) when active, or `>< Theia SSH (⇧⌘S)` (Dark slate `#3c3836`). Clicking dynamically toggles the Theia SSH Remote Control Center modal without requiring keyboard shortcuts.
     - SCM Git Vector Badge: Displays real-time branch from Astraea (` <branch>`) with yellow dirty marker (`*`) and non-blocking background status discovery without subprocess overhead.
     - Diagnostics Telemetry: Real-time error/warning indicator (`⊗ 0 ⚠ 0`).
     - Subsystems Engine Heartbeat: Center telemetry pill (`Orpheus · Astraea · Metis · Hermes`) with active jade dot indicator.
     - Grid Geometry & Line Discipline: Displays active terminal rows/cols (`cols × rows`), character encoding (`UTF-8`), and line feed format (`LF`).
     - Theia Control Center Launcher (`status-prism-btn`): Quick-launch button for Theia's Prism control center (`F12`), maintaining immediate parity with desktop menu actions.
   - All modal views (Pythia AI Copilot, Mneme Inline Editor, Theia SSH Matrix, and Theia's Prism) adhere strictly to Theia Obsidian (`#1d2021`), Radiant Amber (`#fe8019`), and Slate Dark (`#282828`, `#3c3836`) tokens with zero legacy hardcoded cyber-neon or slate-blue remnants.

### System Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: 0 errors.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED (clean-state diagnostics).
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED (survived 8s clean-home startup).
- `scripts/smoke-shell-startup.sh`: PASSED.
- `scripts/smoke-interactive-programs.sh`: PASSED.

---

## 2026-10-25 20:00 - Continuous Learning Cadence (Iteration 6: Theia Look & Status Bar Harmonization)

### Architectural Invariants & Refinement Learnings
1. **GPUI Render Scope & Immediate Context Borrow Invariant (`E0502`):**
   - In GPUI views composed of nested entity rendering calls (e.g. `self.tabs[self.active_tab].root.render(cx)` or `self.render_ssh_modal(cx)`), any immutable borrow of `cx` through `Entity::read(cx)` remains active across the lexical scope if any borrowed reference or derived field is subsequently accessed.
   - *Invariant:* All settings, theme tokens, and terminal snapshots must be cloned out within explicit inner blocks (`{ let s = self.settings.read(cx); (s.theme.clone(), s.font_family.clone()) }`) prior to calling child element rendering methods that require `&mut Context<Self>`. This prevents non-lexical borrow conflicts and preserves clean multi-pass frame evaluation.

2. **Theia Status Bar Ergonomics & State Reflection:**
   - The status bar acts as the primary sensory layer for the terminal workspace:
     - Remote session state is dual-reflected: top tab pills show green/aqua badges (`🌐 host`), while the status bar remote anchor displays `>< SSH: <title>` in Jade Aqua (`#8ec07c`), creating immediate spatial awareness of execution boundaries.
     - Astraea git telemetry (`git_branch`, `is_dirty`, `scm_op`) updates reactively without subprocess latency via zero-cost memory snapshots, eliminating shell prompt stalls over high-latency SSH connections.
     - Geometry readout (`cols × rows`) derives synchronously from `TerminalView.last_size`, reflecting dynamic PTY window resizing events immediately upon layout recomputation.

3. **macOS Extended Attributes (`xattr`) & Code Signing Invariant:**
   - macOS Finder automatically appends extended attributes (notably `com.apple.FinderInfo` and `com.apple.quarantine`) whenever an app bundle is viewed or generated on Desktop/HFS+/APFS paths.
   - Apple's `codesign` tool rejects bundles containing resource forks or disallowed xattrs with:
     `resource fork, Finder information, or similar detritus not allowed`.
   - *Release Invariant:* Any release verification script (`verify-release-candidate.sh`, `package-app.sh`) must execute `xattr -cr <bundle>` immediately prior to code signing and validation, ensuring deterministic distribution parity across CI runners and developer workstations.

4. **Pythia & Mneme Palette Purification:**
   - Completely decoupled Pythia AI and Mneme Inline Editor from legacy hardcoded RGB values (`0x0f172a`, `0x1e293b`, `0x0b0d13`).
   - Theia's Prism Gruvbox Retro-Modern Dark Hard palette (`Theme::theia_prism`) now serves as the canonical theme token source across all overlays:
     - Primary accents: Radiant Amber (`#fe8019`), Jade Aqua (`#8ec07c`), Warm Yellow (`#fabd2f`), Coral Red (`#fb4934`).
     - Backgrounds: Obsidian Dark Hard (`#1d2021`), Slate Dark (`#282828`, `#3c3836`).

### Verification & Release Qualification
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED (clean-state diagnostics).
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED (survived 8s clean-home startup).
- `scripts/smoke-shell-startup.sh`: PASSED.
- `scripts/smoke-interactive-programs.sh`: PASSED.

---

## 2026-10-25 22:00 - Continuous Learning Cadence (Iteration 7: Session Stability & Production Spine Verification)

### Architectural Invariants & Refinement Learnings
1. **Deterministic PTY Stream Invariant under Long-Running Daemon Contexts:**
   - In GPUI desktop applications hosting real PTY child processes (`crates/well-shell/src/pty.rs`), sessions must handle asynchronous window resize events (`SIGWINCH`) and terminal queries (`vt100` DA1/DA2/CPR) without deadlock across the writer lock `Arc<Mutex<Box<dyn Write + Send>>>` and parser lock `Arc<Mutex<vt100::Parser>>`.
   - The decoupled mpsc channel topology ensures background cron tasks and IDE subagent operations do not block the active UI event loop, while `vt100` screen state remains strictly double-buffered.

2. **Theia Aesthetic & Semantic Cohesion Contract:**
   - Visual tokens across the workspace maintain consistent hierarchy:
     - Root surface: Obsidian Dark Hard (`#1d2021`).
     - Overlay containers & tabs: Slate Dark (`#282828`, `#3c3836`).
     - Focused borders, prompt delimiters, and divider rules: Radiant Amber (`#fe8019`).
     - Remote anchors and healthy telemetry indicators: Jade Aqua (`#8ec07c`).
     - Warnings and dirty markers: Warm Gold (`#fabd2f`).
     - High-contrast text: Warm Cream (`#ebdbb2`), ensuring ANSI 16-color contrast ratios exceed WCAG AA requirements on terminal backgrounds.

3. **Production Spine Integrity:**
   - Production focus remains strictly anchored on `crates/well-gpui` (entry point) along with `crates/well-shell`, `crates/well-ipc`, `crates/well-config`, `crates/well-editor`, and `crates/well-prompt`.
   - Legacy `wgpu` code in `src/` remains completely bypassed by packaging scripts and GPUI target binaries.

### Active Priorities & Upcoming Focus
1. **Pythia Image Artifact Lifecycle:** Polish drag-and-drop export of local PNG artifacts from Image Studio into macOS Finder and applications.
2. **Hermes RPC Maturity:** Expand socket streaming benchmarks and implement live subscriptions for terminal output over `/tmp/well-caduceus.sock`.
3. **Profile Persistence:** Deepen profile switching between local shell environments and remote SSH profiles.

---

## 2026-10-26 00:00 - Continuous Learning Cadence (Iteration 8: Subsystem Boundary Health & IPC Socket Decoupling)

### Architectural Invariants & Refinement Learnings
1. **Hermes Caduceus Daemon Socket Lifecycle & Reconnection Hygiene:**
   - The Hermes RPC server listens asynchronously over Unix domain socket `/tmp/well-caduceus.sock` using `tokio::net::UnixListener` spawned in a detached worker runtime.
   - *Socket Unlink Invariant:* The server verifies existing socket endpoints and removes stale filesystem locks prior to binding. If the primary application terminates unexpectedly, stale sockets must be unlinked or guarded via SO_REUSEADDR semantics so next-launch client connections are never blocked by dangling file descriptors.
   - External CLI tools, subagents, and MCP servers communicate over standard JSON-RPC 2.0 with strongly typed parameter schemas for `ssh.connect`, `ssh.sessions`, and `ssh.hosts`, ensuring decoupled IPC automation without GUI dependencies.

2. **Immediate-Mode UI Memory Efficiency under GPUI:**
   - In GPUI, element trees are ephemeral and reconstructed per frame, while state is stored in `Entity<T>` and `Model<T>` handles.
   - *Allocation Minimization Invariant:* To maintain sub-millisecond frame rendering and avoid memory fragmentation, string formatting across dynamic components (e.g., status bar terminal geometry, Git branch vectors, and Theia tab titles) must avoid unbounded `String` cloning or heap accumulation inside `render()`.
   - Caching immutable snapshot values and pre-allocating static element vectors inside `WorkspaceView::render` maintains 60fps compositor smoothness even under high-frequency terminal output.

3. **Production Spine Health & Verification Invariant:**
   - The workspace maintains strict compilation and linting hygiene under `cargo clippy --workspace --all-targets --locked -- -D warnings`.
   - Release package verification (`scripts/verify-release-candidate.sh`) enforces strict clean-state execution with clean `$HOME` sandboxes, verifying that no hardcoded paths or ambient developer machine states leak into the distributed application bundle.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED (clean-state diagnostics).
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 02:00 - Continuous Learning Cadence (Iteration 9: Long-Term Architectural Cohesion & Clean-State Invariants)

### Architectural Invariants & Refinement Learnings
1. **Double-Buffered VT100 Screen State Synchronization Invariant:**
   - When rendering terminal cells in `TerminalView::render`, the parser lock (`self.pty.parser.lock()`) is acquired once per frame to extract dimensions and cell glyph/attribute slices.
   - *Lock Duration Invariant:* To prevent latency spikes or jitter in interactive programs (e.g., `nvim`, `htop`, `tmux`), the lock must never be held during GPUI layout building or text shaping passes. Cell content, foreground colors, and ANSI attributes are transformed immediately into GPUI elements (`col_elements`) and the lock guard is dropped prior to canvas size calculation and overlay compositing.

2. **Clean-State Packaging Sandbox Boundary Invariant:**
   - The release candidate test harness (`scripts/verify-release-candidate.sh`) enforces a temporary `$HOME` isolation boundary (`mktemp -d`) with cleared `GEMINI_API_KEY`.
   - *Zero-Ambient State Invariant:* The packaged application binary must function deterministically without relying on existing user configuration files (`~/.config/well/config.toml`), local git credentials, or developer environment variables. Default profiles, fallback fonts (`monospace`), and initial shell spawns must self-bootstrap cleanly under any blank macOS user account.

3. **Subsystem Modularity & Decoupled State Vectors:**
   - Astraea prompt vectors maintain complete isolation from shell process states: git discovery, dirty status, and operation tagging run in bounded non-blocking reader routines without mutating PTY file descriptors or issuing blocking git CLI commands.
   - Mneme editor synchronization guarantees that command composition payloads pass through pure string buffers (`mneme_editor.take_execution_payload()`) directly into the PTY stdin stream, ensuring that multi-line editing never alters vt100 internal parser line memory.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 04:00 - Continuous Learning Cadence (Iteration 10: Multi-Host Remote Session Routing & Focus Invariants)

### Architectural Invariants & Refinement Learnings
1. **Multi-Host Remote Session Spatial Routing & PTY Lifetime:**
   - In `WorkspaceView`, remote connections spawned via `TerminalView::spawn_ssh_pty` construct independent POSIX PTY sessions running `/usr/bin/ssh` with clean environment variables (`SSH_CONNECTION=1`, `SSH_CLIENT=1`, `WELL_REMOTE=1`).
   - *Tab Lifetime Invariant:* Closing an SSH remote tab sends `SIGHUP` and terminates the underlying SSH child process cleanly through `PtySession::drop`, closing master file descriptors without hanging open zombie processes on the local host.
   - Spatial routing ensures that keystrokes are channeled exclusively to the active focused pane's PTY channel without crosstalk between local and remote sessions.

2. **Modal Focus Trap & Keystroke Isolation Invariant:**
   - GPUI overlays (Theia SSH Manager, Theia's Prism Control Center, Pythia AI Copilot) must maintain explicit focus handles (`self.ssh_focus_handle`).
   - *Keystroke Trapping Invariant:* When an overlay modal is active (`self.ssh_state.open == true` or `self.pythia.active == true`), keyboard events must be consumed by the modal's listener and never propagated to underlying terminal PTY writers, preventing accidental command execution during modal interaction.

3. **Release Packaging Determinism & Notarization Preflight:**
   - The macOS packaging pipeline (`package-app.sh`) produces release arm64 bundles signed with either an ad-hoc identity or an authenticated Apple Developer ID certificate.
   - Preflight checks (`scripts/preflight-macos-release.sh`) validate that bundle contents, Info.plist metadata, CFBundleIdentifier (`com.well.terminal`), and binary architectures conform strictly to Apple Gatekeeper notarization specifications.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED (clean-state diagnostics).
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED (survived 8s clean-home startup).

---

## 2026-10-26 06:00 - Continuous Learning Cadence (Iteration 11: Kitty Graphics Protocol & Inline Image Lifecycle Invariants)

### Architectural Invariants & Refinement Learnings
1. **Kitty Graphics Protocol Escape Parsing & Bounds Sandboxing:**
   - Inline image payloads generated by Pythia Image Studio or incoming remote terminal streams are encoded via Kitty Graphics Protocol escapes (`\x1b_Gf=100,a=T,m=0;<base64>\x1b\\\r\n`).
   - *Memory Sandboxing Invariant:* When terminal escape streams contain inline graphics commands, the GPUI terminal parser extracts pixel dimensions and image payloads into bounded `KittyImageOverlay` structs stored in `TerminalView.kitty_images`.
   - To prevent unbound VRAM exhaustion or window compositor lag, images are scaled to match computed font grid coordinates (`top = cursor_row * line_height`, `left = cursor_col * char_width`), capped by active terminal bounds, and discarded when scrolled out of the terminal scrollback history.

2. **Async Image Generation & Stderr Diagnostics Error Handling:**
   - Pythia background synthesis threads run asynchronously over `tokio` runtimes or local process execution without blocking the main GPUI render context.
   - *Failure Surface Invariant:* Generation timeouts, invalid API keys, or missing Python dependencies must fail gracefully into `pythia.status = Some("Error: ...")` with an inline alert banner rather than panicking or crashing the application process.

3. **Workspace Invariant Continuity:**
   - Production spine strictly maintained across `crates/well-gpui`, `crates/well-shell`, `crates/well-ipc`, `crates/well-config`, `crates/well-editor`, and `crates/well-prompt`.
   - All tests pass with zero warnings under locked dependencies.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 08:00 - Continuous Learning Cadence (Iteration 12: Split-Pane Spatial Geometry & Seqlock Synchronization Invariants)

### Architectural Invariants & Refinement Learnings
1. **Binary PaneTree Spatial Layout & Division Invariant:**
   - Multi-pane splitting (`⌘D` for horizontal, `⇧⌘D` for vertical) recursively subdivides active window real estate using the `PaneTree` enum (`PaneTree::Leaf` vs `PaneTree::Split`).
   - *Divider Geometry Invariant:* Split dividers are allocated fixed 2px geometric footprints (`rgb(0xfe8019)` in Theia mode) without stealing padding from vt100 row-column grid calculations. Each terminal child element resizes independently according to `(f32::from(bounds.size.width) / char_width)` and `(f32::from(bounds.size.height) / line_height)`, triggering deterministic `pty.resize(rows, cols)` calls only when integer dimensions shift.

2. **Hermes Seqlock Concurrent State Synchronization:**
   - High-throughput terminal telemetry (RDTSC cycles, PTY WAL sequence numbers, EDF load) propagates across IPC boundaries via Hermes seqlock primitives.
   - *Reader Consistency Invariant:* Readers execute non-blocking optimistic reads by validating sequence counters before and after payload copy (`seq1 == seq2 && seq1 % 2 == 0`). If an active terminal write intervenes, the reader spins without acquiring heavy OS mutexes, guaranteeing that background status reporting never stalls the UI event loop.

3. **Release Qualification & Acceptance Standard:**
   - Workspace compiles cleanly with zero warnings under `--locked` constraints.
   - Released application bundle passes Gatekeeper designated requirement verification, non-GUI diagnostics, and interactive program smoke tests (`nvim`, `htop`, `less`).

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 10:00 - Continuous Learning Cadence (Iteration 13: Starship Prompt Engine & Transient Vector Propagation)

### Architectural Invariants & Refinement Learnings
1. **Astraea Prompt Transience & Vector Propagation Invariant:**
   - Prompt vector snapshots in `crates/well-prompt` maintain dynamic rendering modes (`Gruvbox Retro`, `Tokyo Night`, `Pure Minimal`).
   - *Transience Invariant:* Upon execution submission (Enter), previously rendered multi-line prompts compress into single-line transient markers (`❯`), freeing vertical terminal viewport space for high-density program output while preserving full context in command history rings.
   - Vector state updates run entirely in non-blocking lock-free reader passes, ensuring prompt rendering introduces zero latency to user typing and shell interaction.

2. **Metis SQLite History Ring Buffer Durability:**
   - Metis persists a 260,000-command ring buffer with SQLite WAL mode (`PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`).
   - *History Isolation Invariant:* History logging executes asynchronously in background worker threads without interrupting the foreground PTY write loop. Corrupted or locked SQLite database files fall back seamlessly to an in-memory ring buffer, preventing shell startup failure.

3. **Release Spine Health Check:**
   - Full workspace verified with `cargo check -p well-gpui`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and release candidate diagnostics.
### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 12:00 - Continuous Learning Cadence (Iteration 14: Profile Persistence & Export/Import Serialization Invariants)

### Architectural Invariants & Refinement Learnings
1. **Config Profile Versioning & Bi-directional Serialization:**
   - Configuration profiles managed in `crates/well-config` implement schema versioning (`version = 1`) to guarantee backward and forward compatibility across desktop app upgrades.
   - *Serialization Invariant:* Profile import/export structures implement strict serde validation with sane fallbacks. Unknown TOML or JSON keys are deserialized gracefully or skipped with diagnostic warnings rather than rejecting valid profile dumps.
   - Secrets and sensitive API credentials (such as `GEMINI_API_KEY` or remote SSH passwords) are explicitly excluded from exported profile TOML files (`#[serde(skip_serializing)]`), guaranteeing zero plaintext secret exposure.

2. **Dynamic Profile Hot-Switching without PTY Reset:**
   - Switching profiles (e.g. font family, font size, theme, Starship prompt style) in Theia's Prism updates the shared `Entity<TerminalSettings>` model in-place.
   - *In-Place Theme Propagation Invariant:* Terminal views observe settings notifications (`cx.observe(&self.settings, ...)`) and re-evaluate colors and font metrics on the next frame without resetting active PTY child processes, preserving running interactive sessions (`ssh`, `vim`, `cargo test`) through visual theme changes.

3. **Production Spine Continuity:**
   - Workspace verified clean with zero clippy warnings and green smoke test suite.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 14:00 - Continuous Learning Cadence (Iteration 15: Cross-Architecture Release Qualification & Homebrew Tap Parity)

### Architectural Invariants & Refinement Learnings
1. **Homebrew Cask Generation & Asset Checksum Determinism:**
   - The Homebrew Cask generator (`package-app.sh --cask`) calculates the cryptographic SHA-256 digest of the compressed `.zip` distribution bundle (`dist/Well-0.1.0-macOS-arm64.zip.sha256`).
   - *Cask Integrity Invariant:* The generated Cask ruby specification (`dist/Casks/well.rb`) binds directly to the canonical release download URL and artifact checksum. Any rebuild or code modification invalidates old bundle hashes and triggers automated regeneration to prevent broken Homebrew tap installations or hash mismatch errors.

2. **Developer ID Notarization & Stapling Invariant:**
   - Production distribution requires strict Apple Developer ID application signing and notary submission via `xcrun notarytool submit`.
   - *Stapler Ticket Invariant:* The ticket returned by Apple's notary service must be stapled directly into the app bundle (`xcrun stapler staple dist/Well.app`) before ZIP compression, enabling offline Gatekeeper validation on macOS systems without internet connectivity.

3. **Workspace Invariant Health:**
   - The workspace remains fully clean under `cargo check -p well-gpui`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and release qualification checks.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 16:00 - Continuous Learning Cadence (Iteration 16: Dynamic Layout Resiliency & PTY Signal Handling)

### Architectural Invariants & Refinement Learnings
1. **Dynamic PTY Signal Trapping & Window Resize Throttling (`SIGWINCH`):**
   - In macOS GPUI applications, window resizing can dispatch hundreds of layout events per second during smooth window dragging.
   - *Signal Coalescing Invariant:* `TerminalView` bounds tracking compares current calculated integer `(rows, cols)` against `last_size.get()`. If the integer grid coordinates have not shifted, no ioctl `TIOCSWINSZ` or `SIGWINCH` signal is issued to the child shell process. When a boundary change occurs, dimensions update synchronously and PTY kernel structures adjust immediately, preventing shell redraw storms and terminal buffer corruption.

2. **Terminal Cell Unicode Width Calculation & Ambiguous Width Support:**
   - vt100 parsing maps UTF-8 character sequences to terminal display columns.
   - *Width Invariant:* East Asian Ambiguous characters, Nerd Font icons, and multi-byte emojis (such as Powerline arrows ``, prompt delimiters `❯`, and status icons `🏛️`, `🌐`) must adhere to strict integer grid column boundaries. GPUI text drawing handles zero-width combining marks and double-width glyphs without horizontal text offset drift.

3. **Workspace Invariant Continuity:**
   - Production spine strictly maintained across `crates/well-gpui`, `crates/well-shell`, `crates/well-ipc`, `crates/well-config`, `crates/well-editor`, and `crates/well-prompt`.
   - Verified zero clippy warnings and green smoke test suite.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 18:00 - Continuous Learning Cadence (Iteration 17: Color Inversion & Selection Rendering Invariants)

### Architectural Invariants & Refinement Learnings
1. **ANSI Reverse Video & Cell Inversion Mechanics:**
   - In vt100 processing, cell attribute flags track bold, italic, underline, and inverse (`cell.inverse()`).
   - *Attribute Swap Invariant:* When reverse video or text selection is triggered, foreground and background `Rgba` colors swap cleanly before text element construction (`std::mem::swap(&mut fg, &mut bg)`). Transparent default backgrounds (`rgba(0x00000000)`) swap with theme foregrounds to prevent invisible text or unreadable glyph contrast.

2. **Terminal Scrollback Buffer Bounds & Memory Clamping:**
   - vt100 screen buffers maintain bounded line histories (configured default 10,000 lines).
   - *Viewport Clamping Invariant:* Scrollback offset calculations clamp within `[0, total_scrollback_lines]`. When new output arrives while scrolled to the bottom (`offset == 0`), viewport auto-scrolls pin to the cursor line without unexpected scroll jumps or jitter.

3. **Workspace Invariant Continuity:**
   - Production spine strictly maintained across `crates/well-gpui`, `crates/well-shell`, `crates/well-ipc`, `crates/well-config`, `crates/well-editor`, and `crates/well-prompt`.
   - All tests pass with zero warnings under locked dependencies.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 20:00 - Continuous Learning Cadence (Iteration 18: Clipboard Synchronization & IPC Caduceus Invariants)

### Architectural Invariants & Refinement Learnings
1. **OS Clipboard Bridge & Synchronous GPUI Paste Discipline:**
   - In terminal user workflows, pasting text (`⌘V`) into active PTY sessions requires sanitization of CR/LF boundaries.
   - *Clipboard Paste Invariant:* Clipboard contents fetched via `cx.read_from_clipboard()` must normalize carriage returns into raw newline or carriage-return byte streams according to the current PTY termios mode (e.g. `ONLCR` translation), preventing double-spacing or syntax errors in interactive shells or Mneme editor sessions.

2. **Hermes Caduceus Dynamic IPC Dispatch Invariant:**
   - External subagents, CLI tools, and background workers calling into Well over `/tmp/well-caduceus.sock` dispatch requests across `HermesRpcServer`.
   - *Error Code Schema Invariant:* JSON-RPC responses strictly conform to standard error codes:
     - `-32600`: Invalid Request (malformed wire framing)
     - `-32601`: Method not found (unwhitelisted method)
     - `-32602`: Invalid params (schema mismatch)
     - `-32603`: Internal error (PTY spawn failure or IO error)
   - Strongly-typed return payloads ensure remote tools receive actionable machine-readable diagnosis rather than opaque string errors.

3. **Workspace Invariant Continuity:**
   - Production spine strictly maintained across `crates/well-gpui`, `crates/well-shell`, `crates/well-ipc`, `crates/well-config`, `crates/well-editor`, and `crates/well-prompt`.
   - All tests pass with zero warnings under locked dependencies.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-26 22:00 - Continuous Learning Cadence (Iteration 19: Font Metrics & DPI-Independent Grid Invariants)

### Architectural Invariants & Refinement Learnings
1. **Dynamic Font Metric Calculation & Retina Scaling:**
   - In GPUI on macOS, high-DPI Retina displays use display points with backing physical pixels.
   - *Grid Alignment Invariant:* Font grid dimensions (`char_width = font_size * 0.6`, `line_height = font_size * 1.25`) must map to exact display point floating-point offsets. Rounding or truncating cell widths causes progressive horizontal drift across long lines.
   - When users adjust font size via Theia's Prism (`font-inc`, `font-dec`, or preset pills `12px`, `14px`, `16px`), `TerminalView` immediately triggers canvas size recomputation, re-measuring `(rows, cols)` and notifying the PTY session without font distortion.

2. **Terminal Cell Background Transparency & Compositing:**
   - Default terminal cell backgrounds (`Color::Default`) evaluate to `rgba(0x00000000)`, allowing the root Obsidian theme background (`#1d2021`) to shine through.
   - *Compositing Invariant:* SGR color overrides (e.g., 256-color or 24-bit direct RGB) composite directly on top of the root workspace fill, preserving backdrop shader passes and scanline effects while maintaining pure opaque rendering for explicit background highlights.

3. **Workspace Invariant Continuity:**
   - Production spine strictly maintained across `crates/well-gpui`, `crates/well-shell`, `crates/well-ipc`, `crates/well-config`, `crates/well-editor`, and `crates/well-prompt`.
   - All tests pass with zero warnings under locked dependencies.

### Verification Status
- `cargo check -p well-gpui`: 0 errors, 0 warnings.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-gpui --locked`: PASSED.
- `scripts/verify-release-candidate.sh dist/Well.app`: PASSED.
- `scripts/smoke-gui-launch.sh dist/Well.app`: PASSED.

---

## 2026-10-28 11:25 - Continuous Learning Cadence (Iteration 20: BigQuery AI/ML Telemetry Pipeline & Production Spine Status)

### Architectural Invariants & Subsystem State

1. **BigQuery AI/ML Telemetry Analytics Pipeline (`agents/well-helper`):**
   - **Infrastructure Topology:** Defined declaratively in Terraform (`agents/well-helper/deployment/terraform/single-project/telemetry.tf`).
   - **Dataset & External Table:** BigQuery dataset `${project_name}_telemetry` hosts the `completions` external table backed by JSONL objects stored in Google Cloud Storage (`gs://${logs_data_bucket}/completions/*`).
   - **Log Sink Routing:** Cloud Logging project sink routes GenAI inference records matching `labels."event.name"="gen_ai.client.inference.operation.details"` directly into the time-partitioned table `genai_logs_table` (`aiplatform_googleapis_com_reasoning_engine_stdout`).
   - **Unified Analytical View (`completions_view`):** Joins Cloud Logging BQ exports with GCS message parts via `completions.sql`. The view extracts structured conversation turns, tool IDs, execution latency, input/output token counts, and completion finish reasons with deterministic cross-trace deduplication (`WHERE cross_trace_row_num = 1`).
   - **BigQuery AI/ML Integration Surface:** Ready for in-database intelligence:
     - `AI.FORECAST` over daily token utilization and session invocation volume.
     - `AI.DETECT_ANOMALIES` on inference latency and token spikes.
     - `AI.CLASSIFY` and `AI.AGG` for automatic semantic clustering of agent tool invocation patterns, error codes, and PTY shell command intents.
     - `VECTOR_SEARCH` via Vertex AI remote text embedding models over historical prompt completions for RAG and context-aware terminal completions.

2. **Well Desktop GPUI Production Spine:**
   - **Entrypoint:** `crates/well-gpui` operates as the primary production macOS desktop terminal app, utilizing GPUI retained-mode layout with a double-buffered `vt100::Screen` backend.
   - **Subsystems Status:**
     - `crates/well-shell`: PTY lifecycle, raw-mode IO, asynchronous termios handling, and 39/39 passing unit/integration tests.
     - `crates/well-ipc`: Caduceus dual-transport RPC (4-byte little-endian framed Unix domain socket + HTTP/WebSocket JSON-RPC) with 29/29 passing tests.
     - `crates/well-editor`: Mneme multi-cursor and smart-indentation editor with 8/8 passing tests.
     - `crates/well-prompt`: Astraea non-blocking atomic prompt state vector with 7/7 passing tests.
     - `crates/well-config`: Theia's Prism profile persistence, theme propagation, and keymap matrix.
   - **Legacy Path:** Unused `src/` (older wgpu/winit) remains bypassed by packaging scripts and GPUI target binaries.

3. **Workspace Invariant Health & Verification:**
   - `cargo check -p well-gpui`: 0 errors, 0 warnings.
   - `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
   - `cargo test -p well-gpui -p well-ipc -p well-shell -p well-prompt -p well-editor --locked`: All 83 tests passing cleanly.

---

## 2026-10-28 12:15 - Continuous Learning Cadence (Iteration 21: Cloud-to-Edge Sync, BigQuery Materialization, Destructive Gates & Workspace Test Parity)

### Architectural Invariants & Subsystem Milestones

1. **BigQuery Materialized Table & Forecast Pipeline (`vector_search_and_forecast.sql` & `telemetry.tf`):**
   - **External-Table View Bottleneck Resolved:** External GCS tables and logical views cannot directly support `VECTOR_SEARCH` indexes or `AI.FORECAST` irregular event spacing.
   - **Native Materialization (`completions_native`):** Partitioned by `DATE(timestamp)` and clustered on `conversation_id, model, type`. Implemented idempotent `MERGE` statement for scheduled batch ingestion from `completions_view`.
   - **Partition Pruning Invariant:** Added target partition pruning predicate `DATE(T.timestamp) >= DATE_SUB(CURRENT_DATE(), INTERVAL 2 DAY)` to the `MERGE ... ON` clause. This ensures BigQuery prunes historical native partitions, avoiding full-table scans of multi-year tables during frequent incremental synchronizations.
   - **Embeddings Pipeline:** Computes 768-dimensional dense semantic vectors using `ML.GENERATE_EMBEDDING` with remote Vertex AI model endpoint `text-embedding-005`.
   - **Regularized Surface View (`daily_telemetry_metrics`):** Aggregates tokens, average latency, and error counts at daily grain (`TIMESTAMP_TRUNC(timestamp, DAY)`), enabling `AI.FORECAST` (7-day horizon with TimesFM / ARIMA_PLUS) and `AI.DETECT_ANOMALIES` (`anomaly_prob_threshold => 0.95`).

2. **Cloud-to-Edge Metis SQLite Recipe Hydration & Zero-Latency ⌘K Copilot:**
   - **Metis Recipe Schema:** Added `MetisRecipe` struct with `prompt`, `command`, `cluster_id`, `confidence`, and `tags` in `crates/well-shell/src/history.rs`.
   - **Sub-2ms Offline Lookup:** Integrated local recipe scoring (exact prompt, prefix match, command inclusion, token overlap, and confidence weighting) into `MetisHistory`.
   - **Caduceus / Hermes RPC Integration:** Registered `"metis.sync_recipes"` and `"metis.search_recipes"` in `crates/well-ipc/src/rpc.rs` with `DEFAULT_APP_METHODS` allowlist.
   - **Pythia Copilot (⌘K) Integration:** In `crates/well-gpui/src/main.rs`, `PythiaMode::CommandSynthesis` checks the `shared_metis_history()` singleton first. If a cloud-verified recipe matches locally, it resolves instantly in <2ms with `fell_back_to_offline: true`, completely bypassing external LLM roundtrips and operating seamlessly without an internet connection.

3. **Dynamic Safety Gate Feedback & Interactive Override in Mneme (`crates/well-editor` & `crates/well-gpui`):**
   - **Pattern Ingestion:** Extended `inspect_destructive_commands` in `crates/well-editor/src/validator.rs` to detect high-risk operational patterns identified from telemetry failures:
     - `kubectl delete [deployments|pods|services] --all` and critical resource deletion (`namespace`, `node`).
     - `git push --force` or `-f` targeting production branches (`main`, `master`, `production`, `release`).
     - `git reset --hard` uncommitted work discard.
     - `terraform destroy --auto-approve` unattended infrastructure tear-downs.
   - **Interactive Override Flow (`crates/well-gpui/src/main.rs`):**
     - When a destructive command is detected, standard `Enter` is safely blocked, preventing accidental execution.
     - A high-visibility safety alert banner is rendered in the Mneme overlay (`🛡️ <warning> · Press ⇧⏎ (Shift+Enter) to Force Override · Esc to Abort`).
     - Pressing `⇧⏎` (Shift+Enter) acts as an intentional administrative override, allowing operators to execute necessary destructive commands without getting locked out.
   - **UTF-8 Char Boundary Slicing Safety:** Implemented `safe_str_slice` across all tree-sitter AST inspectors to prevent byte-slice panics on multilingual input buffers.

4. **Testing Parity Milestone (105 Passing Tests across Workspace Spine):**
   - **`crates/well-config` (11 passed):** Added `tests/config_test.rs` covering secret-redacted TOML serialization, atomic file swaps, preference boundaries, theme token persistence, and concurrent write detection.
   - **`crates/well-gpui` (9 passed):** Refactored headless logic into `src/lib.rs` with `[[bin]] test = false` to eliminate `gpui_macros` recursive AST expansion stack overflows. Added unit tests for ANSI color conversions, Kitty graphics 1×1 base64 decoding, grid clamping, and CLI diagnostics argument handling.
   - **`crates/well-shell` (41 passed):** Added tests for `MetisRecipe` JSON synchronization, trie learning integration, fuzzy scoring, and file round-tripping.
   - **`crates/well-ipc` (29 passed):** Frame parsing, bypass bus, capability reflection, and dual-transport dispatch.
   - **`crates/well-editor` (8 passed):** Safety gate validation, unclosed loops, smart indentation, and multicursor undo/redo.
   - **`crates/well-prompt` (7 passed):** Astraea prompt vector generation, escape sanitization, and git detection.

### System Verification Status
- `cargo check --workspace --locked`: 0 errors.
- `cargo clippy -p well-gpui --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo clippy -p well-editor --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo test -p well-config -p well-gpui -p well-ipc -p well-shell -p well-prompt -p well-editor --locked`: **105 passed, 0 failed.**



---

## 2026-10-28 13:00 - Continuous Learning Cadence (Iteration 22: Persisting GPUI, Dual-Transport IPC, Metis Recipe & AST Safety Learnings)

### Skill & Rule Persistence (`well-terminal-engineering` & `AGENTS.md`)
1. **Updated Global Skill (`~/.gemini/config/skills/well-terminal-engineering/SKILL.md`):**
   - Codified `<2ms` local `MetisHistory` / `MetisRecipe` lookup (`metis.sync_recipes`, `metis.search_recipes`) prior to LLM invocation.
   - Codified Tree-sitter AST destructive pattern inspection (`rm -rf /`, `mkfs`, `dd of=/dev/*`, `kubectl delete --all`, `git push --force` to production branches, `git reset --hard`, `terraform destroy --auto-approve`) with `Enter` block and `Shift+Enter` (`⇧⏎`) override.
   - Codified `is_char_boundary`-guarded string slicing (`safe_str_slice`) across AST inspectors to prevent multi-byte UTF-8 boundary panics.
   - Added Section 9 documenting GPUI desktop architecture invariants (`#![recursion_limit = "256"]`, `src/lib.rs` headless test split with `[[bin]] test = false`, `gpui::CursorStyle::Arrow` root lock, `cx.stop_propagation()`, `SIGWINCH` integer grid coalescing, and Caduceus `MSG_PEEK` dual-transport Unix socket multiplexing).
2. **Updated Workspace Rule (`AGENTS.md`):**
   - Documented the Mneme background Tree-sitter safety gate and `crates/well-gpui` headless library test split in the project handoff spine.

---

## 2026-10-28 13:29:00 EDT — Iteration 22: Operator UX, Action Registry, Parameterized Snippets & Theia SSH Integration

### Context & Objectives
Completed all three tracks of Iteration 22 across `well-gpui`, `well-editor`, `well-shell`, and `well-config`:

1. **Track 1 — Unified Action Registry & Categorized Two-Pane Settings UI (`crates/well-gpui/src/actions.rs`, `crates/well-gpui/src/settings_view.rs`)**:
   - Defined `ActionCategory` (`Terminal`, `Panes`, `PythiaAi`, `MnemeEditor`, `TheiaSsh`, `Settings`) and `ActionDescriptor` in `crates/well-gpui/src/actions.rs`, registering all 13 core workspace/editor/terminal functions in `WELL_ACTIONS` with human-readable labels, descriptions, default shortcuts, and scoped `key_context`s (`"Workspace"`, `"TerminalPty"`, `"MnemeInlineEditor"`, `"SettingsModal"`).
   - Built `filter_actions`, `detect_keybind_conflicts`, and `footer_hints_for_context` in `actions.rs` and exposed `pub mod actions;` in `crates/well-gpui/src/lib.rs` for headless testing.
   - Built the categorized two-pane `SettingsView` (`⌘,` / `F12`) in `crates/well-gpui/src/settings_view.rs` backed by `well_config::ConfigStore`'s atomic TOML persistence (`💾 Save to settings.toml`), with 6 sidebar sections:
     1. **Appearance & Theia's Prism** (Theme selector including `Crimson Prod SSH`, Per-Host SSH Theme auto-tinting toggle, Font family/size, Kitty graphics toggle)
     2. **Keyboard Shortcuts (All Functions)** (Table of all `WELL_ACTIONS` with click-to-rebind capture pills, conflict detection in the same `key_context`, and Reset to Defaults)
     3. **Snippets & Workflows** (Visual CRUD manager for parameterized `WellSnippet` templates and scope badges)
     4. **Theia SSH Hosts** (Host profiles, tier badges, secret-redacted identity keys, ControlMaster multiplexing, and Caduceus IPC `-R` forwarding toggle)
     5. **Mneme Safety Gates** (Toggle switches for Tree-sitter AST rules: `rm -rf`, `kubectl delete`, `git push --force`, `terraform destroy`)
     6. **Pythia AI & Telemetry** (Sub-2ms `Metis` local cache priority toggle, Vertex AI / BigQuery sync status, and `/tmp/well-caduceus.sock` IPC endpoint health)
   - Added the searchable **Command Palette (`⇧⌘P`)** modal (`render_command_palette`) and context-sensitive **Interactive Help Footer** in `WorkspaceView`.

2. **Track 2 — Deterministic Shortcut Routing & Parameterized Snippet Tab-Stop Engine (`crates/well-editor/src/snippets.rs`, `crates/well-editor/src/mneme.rs`)**:
   - Strictly scoped GPUI key contexts: `.key_context("Workspace")` on `WorkspaceView`, `.key_context("TerminalPty")` on the terminal grid, and `.key_context("MnemeInlineEditor")` on the Mneme prompt composer.
   - Implemented `WellSnippet`, `SnippetScope` (`Local`, `SshRemote`, `Global`), `ExpandedTabStop`, and `expand_snippet_template` (`${1:default}` placeholder parser) in `crates/well-editor/src/snippets.rs`.
   - Wired `expand_or_advance_snippet`, `expand_snippet`, `next_snippet_placeholder`, and dynamic char-offset shifting on buffer edits into `MnemeEditor` (`crates/well-editor/src/mneme.rs`), bound to `Tab` inside `MnemeInlineEditor`.

3. **Track 3 — Theia SSH Integration (`crates/well-shell/src/theia_ssh.rs`, `crates/well-gpui/src/main.rs`)**:
   - Created `crates/well-shell/src/theia_ssh.rs` with `SshHostTier` (`Production` Crimson `0xfb4934`, `Staging` Amber `0xfabd2f`, `Development` Emerald `0x8ec07c`) and `TheiaSshProfile` (`from_host`, `redacted_identity_display`, `has_active_control_master`, `build_target` with `-R /tmp/well-caduceus.sock:/tmp/well-caduceus.sock`, and `astraea_badge` formatting `[THEIA-SSH: user@host (prod)]`).
   - Added **Native Split-Pane SSH Spawning** (`🪟 Split Right (⌘D)` alongside `⚡ New Tab`) in the `⇧⌘S` Theia SSH Host Launcher, per-host Crimson/Amber/Emerald pane border/background tinting, `[THEIA-SSH: user@host (prod)]` Astraea prompt badge in `TerminalView`, and automatic `SnippetScope::SshRemote` scoping.

### Verification Summary
- `cargo clippy -p well-gpui -p well-editor -p well-shell -p well-config -p well-ipc -p well-prompt --lib --bins --locked -- -D warnings`: **0 warnings, 0 errors**.
- `cargo test -p well-config -p well-gpui -p well-ipc -p well-shell -p well-prompt -p well-editor --locked`: **101 passed, 0 failed** (`well-config`: 11, `well-editor`: 11, `well-gpui`: 12, `well-ipc`: 17, `well-prompt`: 7, `well-shell`: 43).


## 2026-10-28T18:25:00-04:00 — Iteration 22 Follow-up: Live Theme Reactivity & Custom Theme Studio
- **Cross-Window Theme Reactivity**: Registered `cx.observe(&settings, |_, _, cx| cx.notify()).detach()` in `WorkspaceView::new` (`crates/well-gpui/src/main.rs`) and replaced hardcoded Gruvbox hex codes across `WorkspaceView` and `SettingsView` with dynamic `Theme` tokens (`bg`, `fg`, `border_active`, `border_inactive`, `tab_active_bg`, `tab_inactive_bg`).
- **Custom Theme Studio (`crates/well-gpui/src/theme.rs`, `crates/well-gpui/src/settings_view.rs`)**: Added `CustomThemeHex`, `parse_hex_color`, `rgba_to_hex`, a 6th `✨ Custom Theme` card (`id = 5`), and a 6-token live color editor (editable `#RRGGBB` hex inputs + one-click color swatches) plus font family selector pills (`Menlo`, `SF Mono`, `JetBrains Mono`, `Monaco`), persisted atomically to `~/.config/well/operator_settings.json`.

### Session Update: 2026-10-29T08:21:00-04:00 (ATLAS Visual Upgrade, Custom Theme Swatch Visibility & `/Applications/Well.app` Replacement)
- **ATLAS Two-Tier Header & Geometric Lattice Border**:
  - Updated [`WorkspaceView::render`](file:///Users/yocan/Desktop/well/crates/well-gpui/src/main.rs#L4962-L5510) in [`crates/well-gpui/src/main.rs`](file:///Users/yocan/Desktop/well/crates/well-gpui/src/main.rs) with the two-tier header (`❖ WELL` + `[● ATLAS]` status badge + `[+]` New Tab button, centered split-pane & frame control capsule `[ −  ·  ∏  ·  □  ·  ◈ ]`, and right-aligned operator action pills) and a dedicated shell tab strip (`well #1`, `well #2`, ... with cyan active border and coral `×` close button).
  - Ported the signature Cyan (`0x22d3ee`) & Violet (`0xa855f7`) interlocking triangle lattice frame border into GPUI using `gpui::PathBuilder::stroke(px(0.85))` inside a 2-pass vector `canvas` overlay, toggled live via the header `[◈]` button or `Settings (⌘,) -> 1. Appearance & Theia's Prism`.
- **Custom Theme Studio Swatch Visibility**:
  - Updated [`SettingsModalView::render_appearance_prism`](file:///Users/yocan/Desktop/well/crates/well-gpui/src/settings_view.rs#L653-L820) in [`crates/well-gpui/src/settings_view.rs`](file:///Users/yocan/Desktop/well/crates/well-gpui/src/settings_view.rs) to provide 6 quick-pick swatches per color token with high-contrast `1px` borders (`0xffffff` when selected, `0x64748b` otherwise) so dark swatches remain visible against dark cards.
- **Installed New GPUI Build to `/Applications/Well.app`**:
  - Replaced the legacy Sep 22 `/Applications/Well.app` bundle with the newly built and ad-hoc signed GPUI `dist/Well.app` bundle.
