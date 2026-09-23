#!/usr/bin/env bash
# well-benchmarks.sh
#
# Automated Subsystem and Latency Profiling Harness using Hyperfine.
# Profiles startup times, prompt rendering speeds, Hyprlang parse latency,
# and shell execution latencies for the Well (Phrear) terminal environment.

set -euo pipefail

INFO='\033[0;34m[INFO]\033[0m'
SUCCESS='\033[0;32m[SUCCESS]\033[0m'
WARN='\033[0;33m[WARN]\033[0m'
ERROR='\033[0;31m[ERROR]\033[0m'

echo -e "${INFO} Initializing Well Hyperfine Benchmarking Suite..."

if ! command -v hyperfine &> /dev/null; then
    echo -e "${ERROR} hyperfine is not installed. Run: brew install hyperfine" >&2
    exit 1
fi

BENCH_OUT_DIR="target/benchmarks"
mkdir -p "${BENCH_OUT_DIR}"

echo -e "${INFO} Output directory: ${BENCH_OUT_DIR}"

# 1. Benchmark Interactive Shell Startup Latencies
echo -e "${INFO} [Benchmark 1/3] Benchmarking Shell Startup Latencies (Fish vs Zsh vs Sh)..."
hyperfine \
    --warmup 3 \
    --runs 20 \
    --export-markdown "${BENCH_OUT_DIR}/shell_startup.md" \
    --export-json "${BENCH_OUT_DIR}/shell_startup.json" \
    'fish -c exit' \
    'zsh -c exit' \
    'sh -c exit'

# 2. Benchmark Prompt Rendering Latencies (Starship)
echo -e "${INFO} [Benchmark 2/3] Benchmarking Starship Prompt Evaluation Latency..."
hyperfine \
    --warmup 2 \
    --runs 15 \
    --export-markdown "${BENCH_OUT_DIR}/starship_prompt.md" \
    --export-json "${BENCH_OUT_DIR}/starship_prompt.json" \
    'starship prompt --status 0'

# 3. Benchmark Well Subsystem Test Latencies (Hyprlang + HyperShell)
echo -e "${INFO} [Benchmark 3/3] Benchmarking Well Unit & Subsystem Tests..."
hyperfine \
    --warmup 1 \
    --runs 5 \
    --export-markdown "${BENCH_OUT_DIR}/subsystem_tests.md" \
    --export-json "${BENCH_OUT_DIR}/subsystem_tests.json" \
    'cargo test -p well-config -- --quiet' \
    'cargo test -p well-shell -- --quiet'

echo -e "--------------------------------------------------------"
echo -e "${SUCCESS} Hyperfine Benchmarks Completed Successfully!"
echo -e "Generated Reports:"
echo -e "  • ${BENCH_OUT_DIR}/shell_startup.md"
echo -e "  • ${BENCH_OUT_DIR}/starship_prompt.md"
echo -e "  • ${BENCH_OUT_DIR}/subsystem_tests.md"
echo -e "--------------------------------------------------------"
