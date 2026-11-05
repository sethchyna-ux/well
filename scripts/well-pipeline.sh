#!/usr/bin/env bash
# ==============================================================================
# well-pipeline.sh
#
# Unified Development, Testing, and Debugging Pipeline Orchestrator for Well.
# Supports granular re-runs, subsystem targeting, debug instrumentation, and CI parity.
# ==============================================================================

set -euo pipefail

# ------------------------------------------------------------------------------
# Terminal Aesthetics & Formatting
# ------------------------------------------------------------------------------
BOLD=$'\033[1m'
DIM=$'\033[2m'
RESET=$'\033[0m'
BLUE=$'\033[0;34m'
GREEN=$'\033[0;32m'
YELLOW=$'\033[0;33m'
RED=$'\033[0;31m'
MAGENTA=$'\033[0;35m'
CYAN=$'\033[0;36m'

INFO="${BLUE}[INFO]${RESET}"
SUCCESS="${GREEN}[SUCCESS]${RESET}"
WARN="${YELLOW}[WARN]${RESET}"
ERROR="${RED}[ERROR]${RESET}"
DEBUG="${MAGENTA}[DEBUG]${RESET}"
TRACE="${CYAN}[TRACE]${RESET}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

# ------------------------------------------------------------------------------
# Usage & Help
# ------------------------------------------------------------------------------
show_help() {
    cat << EOF
${BOLD}Well Subsystems Pipeline Orchestrator${RESET}

${BOLD}USAGE:${RESET}
  ./scripts/well-pipeline.sh <COMMAND> [OPTIONS]

${BOLD}CORE COMMANDS:${RESET}
  ${CYAN}run${RESET} <stage>              Run a pipeline stage (check, test, build, debug, bench, package, all)
  ${CYAN}rerun${RESET} <stage>            Clean stage artifacts and immediately re-run the stage
  ${CYAN}dev${RESET} [--watch] [--crate]  Fast incremental development loop (supports cargo-watch)
  ${CYAN}test${RESET} [options]           Run subsystem tests with granular targeting
  ${CYAN}debug${RESET} [options]          Run or test under debug harness (trace logs, lldb, mock LLM)
  ${CYAN}zig${RESET}                    Build vendor/libghostty C-ABI archive via Zig
  ${CYAN}package${RESET}                Package release artifacts (macOS .app bundle or Linux)
  ${CYAN}pipeline${RESET} <name>          Run specialized pipeline (core-metrics, wasm, graphics, mobile, cloud, all-stacks)
  ${CYAN}clean${RESET} [target|zig|all]   Clean build artifacts and temporary files
  ${CYAN}doctor${RESET}                 Inspect installed toolchains (Rust, Zig, LLDB, Cargo tools)
  ${CYAN}help${RESET}                   Display this help message

${BOLD}TEST OPTIONS:${RESET}
  --crate <name>        Target a specific workspace crate (e.g. well-llm, well-core, well-render)
  --filter <filter>     Run only tests matching the filter pattern
  --nocapture           Stream test stdout/stderr directly without capturing
  --threads <n>         Set test thread count (e.g. --threads 1 for deterministic PTY testing)

${BOLD}DEBUG OPTIONS:${RESET}
  --lldb                Launch under interactive LLDB debugger
  --trace               Set RUST_LOG=trace,well=trace for maximum diagnostic telemetry
  --mock-llm            Enable WELL_MOCK_LLM=1 to mock external LLM API endpoints
  --headless            Set WELL_HEADLESS=1 for headless testing without a display server
  --backtrace           Set RUST_BACKTRACE=full

${BOLD}EXAMPLES:${RESET}
  ./scripts/well-pipeline.sh rerun test              # Clean test cache and rerun all tests
  ./scripts/well-pipeline.sh test --crate well-llm   # Run tests specifically for well-llm
  ./scripts/well-pipeline.sh debug --trace --lldb    # Debug binary with full trace logs in LLDB
  ./scripts/well-pipeline.sh dev --watch             # Continuous compile loop on file save
EOF
}

# ------------------------------------------------------------------------------
# Subsystem: Zig C-ABI Libghostty Compilation
# ------------------------------------------------------------------------------
build_zig_libghostty() {
    echo -e "${INFO} Checking Zig toolchain..."
    if ! command -v zig &> /dev/null; then
        echo -e "${ERROR} Zig compiler not found on PATH. Required for libghostty C-ABI." >&2
        exit 1
    fi

    echo -e "${INFO} Compiling vendor/libghostty with Zig ($(zig version))..."
    mkdir -p target/zig-out
    zig build-lib vendor/libghostty/src/lib.zig \
        -O ReleaseFast \
        --name ghostty \
        -lc \
        -femit-bin=target/zig-out/libghostty.a

    echo -e "${SUCCESS} Compiled native archive: target/zig-out/libghostty.a"
}

# ------------------------------------------------------------------------------
# Subsystem: Code Quality, Lint & Syntax Checks
# ------------------------------------------------------------------------------
run_check() {
    local target_crate="${1:-}"
    echo -e "${INFO} Running workspace code formatting check..."
    cargo fmt --all -- --check || {
        echo -e "${WARN} Code formatting issues detected. Run 'cargo fmt' to fix."
    }

    echo -e "${INFO} Ensuring Zig native archive is compiled..."
    if [ ! -f "target/zig-out/libghostty.a" ]; then
        build_zig_libghostty
    fi

    if [ -n "${target_crate}" ]; then
        echo -e "${INFO} Running cargo check for crate: ${BOLD}${target_crate}${RESET}..."
        cargo check -p "${target_crate}"
    else
        echo -e "${INFO} Running cargo check across entire workspace..."
        cargo check --workspace --all-targets
    fi

    echo -e "${INFO} Running Clippy linter..."
    local clippy_flags=()
    if [ "${STRICT:-0}" = "1" ] || [ "${CI:-false}" = "true" ]; then
        clippy_flags+=("--" "-D" "warnings")
    fi

    if [ -n "${target_crate}" ]; then
        cargo clippy -p "${target_crate}" ${clippy_flags[@]+"${clippy_flags[@]}"}
    else
        cargo clippy --workspace --all-targets ${clippy_flags[@]+"${clippy_flags[@]}"}
    fi

    echo -e "${SUCCESS} Syntax, type-checking, and linter passed!"
}

# ------------------------------------------------------------------------------
# Subsystem: Test Orchestration
# ------------------------------------------------------------------------------
run_test() {
    local target_crate=""
    local filter=""
    local nocapture=false
    local threads=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --crate)
                target_crate="$2"
                shift 2
                ;;
            --filter)
                filter="$2"
                shift 2
                ;;
            --nocapture)
                nocapture=true
                shift
                ;;
            --threads)
                threads="$2"
                shift 2
                ;;
            *)
                shift
                ;;
        esac
    done

    echo -e "${INFO} Ensuring Zig native dependencies are ready..."
    if [ ! -f "target/zig-out/libghostty.a" ]; then
        build_zig_libghostty
    fi

    local cargo_args=()
    if [ -n "${target_crate}" ]; then
        echo -e "${INFO} Running tests for crate: ${BOLD}${target_crate}${RESET}..."
        cargo_args+=("-p" "${target_crate}")
    else
        echo -e "${INFO} Running tests across entire workspace..."
        cargo_args+=("--workspace")
    fi

    local test_args=()
    if [ -n "${filter}" ]; then
        test_args+=("${filter}")
    fi
    if [ "${nocapture}" = true ]; then
        test_args+=("--nocapture")
    fi
    if [ -n "${threads}" ]; then
        test_args+=("--test-threads" "${threads}")
    fi

    export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
    export WELL_MOCK_LLM="${WELL_MOCK_LLM:-1}"

    if [ ${#test_args[@]} -gt 0 ]; then
        cargo test "${cargo_args[@]}" -- "${test_args[@]}"
    else
        cargo test "${cargo_args[@]}"
    fi

    echo -e "${SUCCESS} Subsystem tests completed successfully!"
}

# ------------------------------------------------------------------------------
# Subsystem: Debugging Harness
# ------------------------------------------------------------------------------
run_debug() {
    local use_lldb=false
    local trace_mode=false
    local mock_llm=true
    local headless=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --lldb)
                use_lldb=true
                shift
                ;;
            --trace)
                trace_mode=true
                shift
                ;;
            --no-mock)
                mock_llm=false
                shift
                ;;
            --headless)
                headless=true
                shift
                ;;
            *)
                shift
                ;;
        esac
    done

    echo -e "${DEBUG} Initializing Well debugging harness..."

    export RUST_BACKTRACE=full
    if [ "${trace_mode}" = true ]; then
        export RUST_LOG="trace,well=trace,wgpu=info,naga=warn"
        echo -e "${TRACE} Enabled trace telemetry (RUST_LOG=${RUST_LOG})"
    else
        export RUST_LOG="debug,well=debug"
        echo -e "${DEBUG} Enabled debug logging (RUST_LOG=${RUST_LOG})"
    fi

    if [ "${mock_llm}" = true ]; then
        export WELL_MOCK_LLM=1
        echo -e "${DEBUG} Mock LLM layer active (WELL_MOCK_LLM=1)"
    fi

    if [ "${headless}" = true ]; then
        export WELL_HEADLESS=1
        echo -e "${DEBUG} Headless mode active (WELL_HEADLESS=1)"
    fi

    if [ ! -f "target/zig-out/libghostty.a" ]; then
        build_zig_libghostty
    fi

    echo -e "${INFO} Building debug binary..."
    cargo build --bin well

    local binary="target/debug/well"
    if [ ! -f "${binary}" ]; then
        echo -e "${ERROR} Debug binary not found at ${binary}" >&2
        exit 1
    fi

    if [ "${use_lldb}" = true ]; then
        echo -e "${DEBUG} Launching under LLDB..."
        lldb "${binary}"
    else
        echo -e "${DEBUG} Executing ${binary}..."
        "${binary}"
    fi
}

# ------------------------------------------------------------------------------
# Subsystem: Incremental Dev Loop
# ------------------------------------------------------------------------------
run_dev() {
    local watch_mode=false
    local target_crate=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --watch)
                watch_mode=true
                shift
                ;;
            --crate)
                target_crate="$2"
                shift 2
                ;;
            *)
                shift
                ;;
        esac
    done

    if [ ! -f "target/zig-out/libghostty.a" ]; then
        build_zig_libghostty
    fi

    if [ "${watch_mode}" = true ]; then
        if command -v cargo-watch &> /dev/null; then
            echo -e "${INFO} Starting cargo-watch loop on workspace..."
            if [ -n "${target_crate}" ]; then
                cargo watch -x "check -p ${target_crate}" -x "test -p ${target_crate} -- --nocapture"
            else
                cargo watch -x "check --workspace" -x "test --workspace -- --nocapture"
            fi
        else
            echo -e "${WARN} 'cargo-watch' is not installed. You can install it with: cargo install cargo-watch"
            echo -e "${INFO} Falling back to single incremental check..."
            run_check "${target_crate}"
        fi
    else
        echo -e "${INFO} Running incremental dev check..."
        run_check "${target_crate}"
    fi
}

# ------------------------------------------------------------------------------
# Subsystem: Re-run Workflow Orchestrator
# ------------------------------------------------------------------------------
run_rerun() {
    local stage="${1:-all}"
    shift || true
    local stage_upper
    stage_upper="$(echo "${stage}" | tr '[:lower:]' '[:upper:]')"
    echo -e "${YELLOW}${BOLD}=== RE-RUN TRIGGERED FOR STAGE: ${stage_upper} ===${RESET}"

    case "${stage}" in
        check|lint)
            echo -e "${INFO} Scrubbing cargo check fingerprint caches..."
            rm -rf target/debug/.fingerprint/well* 2>/dev/null || true
            run_check "$@"
            ;;
        test)
            echo -e "${INFO} Scrubbing test artifacts and caching..."
            rm -rf target/debug/deps/*test* 2>/dev/null || true
            run_test "$@"
            ;;
        zig)
            echo -e "${INFO} Cleaning Zig build directory..."
            rm -rf target/zig-out/
            build_zig_libghostty
            ;;
        debug)
            echo -e "${INFO} Re-compiling debug binary..."
            rm -f target/debug/well
            run_debug "$@"
            ;;
        package)
            echo -e "${INFO} Cleaning dist/ directory..."
            rm -rf dist/
            ./package-app.sh
            ;;
        all)
            echo -e "${INFO} Performing full rebuild & re-run sequence..."
            rm -rf target/zig-out/ dist/
            build_zig_libghostty
            run_check "$@"
            run_test "$@"
            ./package-app.sh
            ;;
        *)
            echo -e "${ERROR} Unknown re-run stage: '${stage}'. Available: check, test, zig, debug, package, all" >&2
            exit 1
            ;;
    esac

    echo -e "${SUCCESS}${BOLD}=== RE-RUN COMPLETED: ${stage_upper} ===${RESET}"
}

# ------------------------------------------------------------------------------
# Subsystem: Doctor & Diagnostic Inspection
# ------------------------------------------------------------------------------
run_doctor() {
    echo -e "${BOLD}Well Subsystems Environment Doctor:${RESET}"
    echo -e "----------------------------------------------------"

    # Rust
    if command -v rustc &> /dev/null; then
        echo -e "  [x] Rust Compiler:   ${GREEN}$(rustc --version)${RESET}"
    else
        echo -e "  [ ] Rust Compiler:   ${RED}Not found${RESET}"
    fi

    # Cargo
    if command -v cargo &> /dev/null; then
        echo -e "  [x] Cargo:           ${GREEN}$(cargo --version)${RESET}"
    else
        echo -e "  [ ] Cargo:           ${RED}Not found${RESET}"
    fi

    # Zig
    if command -v zig &> /dev/null; then
        echo -e "  [x] Zig Compiler:    ${GREEN}v$(zig version)${RESET}"
    else
        echo -e "  [ ] Zig Compiler:    ${RED}Not found (Required for libghostty)${RESET}"
    fi

    # LLDB
    if command -v lldb &> /dev/null; then
        echo -e "  [x] LLDB Debugger:   ${GREEN}$(lldb --version | head -n 1)${RESET}"
    else
        echo -e "  [ ] LLDB Debugger:   ${YELLOW}Optional (needed for --lldb)${RESET}"
    fi

    # cargo-watch
    if command -v cargo-watch &> /dev/null; then
        echo -e "  [x] Cargo Watch:     ${GREEN}Installed${RESET}"
    else
        echo -e "  [ ] Cargo Watch:     ${DIM}Optional (cargo install cargo-watch)${RESET}"
    fi

    # Hyperfine
    if command -v hyperfine &> /dev/null; then
        echo -e "  [x] Hyperfine:       ${GREEN}$(hyperfine --version)${RESET}"
    else
        echo -e "  [ ] Hyperfine:       ${DIM}Optional (needed for ./well-benchmarks.sh)${RESET}"
    fi

    echo -e "----------------------------------------------------"
    echo -e "Workspace Crates:"
    grep -E '^\s*"crates/' Cargo.toml | tr -d '", ' | sed 's/^/  • /'
    echo -e "----------------------------------------------------"
}

# ------------------------------------------------------------------------------
# Dispatcher
# ------------------------------------------------------------------------------
COMMAND="${1:-help}"
shift || true

case "${COMMAND}" in
    run)
        STAGE="${1:-all}"
        shift || true
        case "${STAGE}" in
            check) run_check "$@" ;;
            test) run_test "$@" ;;
            debug) run_debug "$@" ;;
            zig) build_zig_libghostty ;;
            package) ./package-app.sh ;;
            bench) ./well-benchmarks.sh ;;
            all)
                build_zig_libghostty
                run_check
                run_test
                ./package-app.sh
                ;;
            *)
                echo -e "${ERROR} Unknown stage '${STAGE}'. Available: check, test, debug, zig, package, bench, all" >&2
                exit 1
                ;;
        esac
        ;;
    rerun)
        run_rerun "$@"
        ;;
    dev)
        run_dev "$@"
        ;;
    check)
        run_check "$@"
        ;;
    test)
        run_test "$@"
        ;;
    debug)
        run_debug "$@"
        ;;
    zig)
        build_zig_libghostty
        ;;
    bench)
        ./well-benchmarks.sh
        ;;
    package)
        ./package-app.sh
        ;;
    pipeline|pipe)
        SUBPIPELINE="${1:-all-stacks}"
        case "${SUBPIPELINE}" in
            core-metrics|core)
                ./scripts/pipelines/core-metrics-pipeline.sh
                ;;
            wasm)
                ./scripts/pipelines/wasm-pipeline.sh
                ;;
            graphics)
                ./scripts/pipelines/graphics-pipeline.sh
                ;;
            mobile)
                ./scripts/pipelines/mobile-pipeline.sh
                ;;
            cloud|hermes)
                ./scripts/pipelines/hermes-cloud-pipeline.sh
                ;;
            all-stacks|all)
                echo -e "${INFO} Running all 5 repeatable workflow pipelines..."
                ./scripts/pipelines/core-metrics-pipeline.sh
                ./scripts/pipelines/wasm-pipeline.sh
                ./scripts/pipelines/graphics-pipeline.sh
                ./scripts/pipelines/mobile-pipeline.sh
                ./scripts/pipelines/hermes-cloud-pipeline.sh
                echo -e "${SUCCESS} All 5 feature stack pipelines passed successfully!"
                ;;
            *)
                echo -e "${ERROR} Unknown pipeline: '${SUBPIPELINE}'. Options: core-metrics, wasm, graphics, mobile, cloud, all-stacks" >&2
                exit 1
                ;;
        esac
        ;;
    clean)
        TARGET_CLEAN="${1:-all}"
        case "${TARGET_CLEAN}" in
            target)
                echo -e "${INFO} Cleaning target directory..."
                cargo clean
                ;;
            zig)
                echo -e "${INFO} Cleaning Zig artifacts..."
                rm -rf target/zig-out/
                ;;
            dist)
                echo -e "${INFO} Cleaning dist directory..."
                rm -rf dist/
                ;;
            all)
                echo -e "${INFO} Cleaning all build, dist, and zig outputs..."
                cargo clean
                rm -rf dist/ target/zig-out/
                ;;
        esac
        echo -e "${SUCCESS} Clean complete."
        ;;
    doctor)
        run_doctor
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo -e "${ERROR} Unknown command: '${COMMAND}'" >&2
        echo "Run './scripts/well-pipeline.sh help' for usage instructions."
        exit 1
        ;;
esac
