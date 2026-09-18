# Well Subsystems Development, Testing & Debugging Workflows

A technical reference and operator guide for running, re-running, and debugging workflows across the **Well** terminal workspace.

---

## 1. Quick Start

### Check System Prerequisites

```bash
# Via script
./scripts/well-pipeline.sh doctor

# Or via Makefile
make doctor
```

### Local Dev Loop

```bash
# Run syntax and clippy check on all crates
make check

# Check a single crate (e.g., well-ipc)
make check CRATE=well-ipc

# Run tests on a single crate
make test-crate CRATE=well-ipc

# Continuous compilation / watch mode
./scripts/well-pipeline.sh dev --watch
```

---

## 2. Granular Re-run Orchestration

The pipeline includes a dedicated `rerun` engine that invalidates stage-specific fingerprints/caches and triggers a clean re-execution:

| Target Stage | Command | Action Taken Before Execution |
| :--- | :--- | :--- |
| **All** | `./scripts/well-pipeline.sh rerun all` | Scrubs `dist/` and `target/zig-out/`, compiles libghostty, checks, tests, packages |
| **Tests** | `./scripts/well-pipeline.sh rerun test` | Scrubs `target/debug/deps/*test*` cache and re-runs test binaries |
| **Check** | `./scripts/well-pipeline.sh rerun check` | Scrubs `.fingerprint/well*` and re-runs `cargo fmt`, `cargo check`, `clippy` |
| **Zig Archive** | `./scripts/well-pipeline.sh rerun zig` | Scrubs `target/zig-out/` and recompiles `libghostty.a` via Zig |
| **Debug Binary** | `./scripts/well-pipeline.sh rerun debug` | Deletes `target/debug/well` and triggers fresh debug compile |
| **Release Bundle** | `./scripts/well-pipeline.sh rerun package` | Cleans `dist/` and regenerates signed `Well.app` bundle |

You can also pass arguments through `rerun`:

```bash
./scripts/well-pipeline.sh rerun test --crate well-ipc --nocapture
```

---

## 3. Debugging Harness & Instrumentation

The debugging subsystem configures runtime environments for crash analysis, memory leaks, and GPU/PTY diagnostics:

### Trace Logging (`RUST_LOG=trace`)

```bash
./scripts/well-pipeline.sh debug --trace
# Sets RUST_LOG=trace,well=trace,wgpu=info,naga=warn
# Sets RUST_BACKTRACE=full
```

### LLDB Interactive Debugging

```bash
./scripts/well-pipeline.sh debug --trace --lldb
# Compiles with debug symbols and attaches LLDB directly
```

### Mock LLM / Offline Fallback Mode

```bash
./scripts/well-pipeline.sh debug --mock-llm
# Sets WELL_MOCK_LLM=1 to test command-line translation offline without API latency
```

### Headless Display Mode

```bash
./scripts/well-pipeline.sh debug --headless
# Sets WELL_HEADLESS=1 for offscreen headless validation
```

---

## 4. GitHub Actions CI/CD Re-runs

The repository includes [well-pipeline.yml](file:///Users/yocan/Desktop/well/.github/workflows/well-pipeline.yml) which supports GitHub Actions `workflow_dispatch`.

### Triggering Manual Re-runs via GitHub Web UI or CLI

```bash
# Re-run all stages
gh workflow run well-pipeline.yml -f target=all

# Re-run only subsystem tests
gh workflow run well-pipeline.yml -f target=test-workspace

# Re-run with clean cache bypass
gh workflow run well-pipeline.yml -f target=check-lint -f clean_cache=true

# Re-run with full trace logging
gh workflow run well-pipeline.yml -f target=test-workspace -f debug_logging=true
```

### Automatic Concurrency Cancellation

Whenever a developer pushes a new commit or triggers a manual re-run, existing in-progress runs for that branch or PR are terminated immediately via `concurrency.cancel-in-progress: true`, freeing CI resources.

---

## 5. VSCode Integration

Open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) and choose **Tasks: Run Task**:

- `Pipeline: Check Workspace`
- `Pipeline: Rerun Workspace Tests`
- `Pipeline: Debug (Trace Mode)`
- `Pipeline: Rerun All`
- `Pipeline: Toolchain Doctor`

Press `F5` to select launch configurations:

- **Debug Well (Trace & Backtrace)**: Launch under LLDB with trace logs and full backtraces.
- **Debug Well (Mock LLM)**: Launch under LLDB with offline mock translation layer.

---

## 6. Repeatable Pipelines (Placeholder Elimination & Feature Stacks)

Well provides 5 specialized workflow pipelines located in `scripts/pipelines/` to automate subsystem validation, remove placeholders, and verify production feature stacks:

| Pipeline | Script | Makefile Target | Scope & Resolved Subsystems |
| :--- | :--- | :--- | :--- |
| **Core & Telemetry** | `scripts/pipelines/core-metrics-pipeline.sh` | `make core-metrics` | `well-core` RingBuffer/SessionCoordinator & `well-metrics` Prometheus registry |
| **Wasm Extensions** | `scripts/pipelines/wasm-pipeline.sh` | `make wasm` | `well-wasm` deterministic sandboxed Wasm interpreter (`wasmi`) |
| **GPU & Graphics** | `scripts/pipelines/graphics-pipeline.sh` | `make graphics` | `well-render` CRT WGSL shader validation & `well-config` image textures |
| **Mobile Cross-Dev** | `scripts/pipelines/mobile-pipeline.sh` | `make mobile` | Flutter Dart analysis & `well-ffi` C-ABI dynamic library validation |
| **Cloud & K8s** | `scripts/pipelines/hermes-cloud-pipeline.sh` | `make cloud` | Hermes IPC, Dockerfile syntax, and Kubernetes deployment manifests |
| **All 5 Stacks** | `./scripts/well-pipeline.sh pipeline all-stacks` | `make pipelines` | Runs all 5 feature stack pipelines consecutively |

### Executing Specialized Pipelines

```bash
# Execute individual pipelines
make core-metrics
make wasm
make graphics
make mobile
make cloud

# Or execute all 5 consecutively
make pipelines
```
