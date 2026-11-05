#!/usr/bin/env bash
# ==============================================================================
# wasm-pipeline.sh
#
# Pipeline 2: WebAssembly Sandboxed Plugin Runtime Pipeline
# Validates Wasm bytecode compilation, deterministic sandboxed execution, and plugin hooks.
# ==============================================================================

set -euo pipefail

BOLD=$'\033[1m'
RESET=$'\033[0m'
BLUE=$'\033[0;34m'
GREEN=$'\033[0;32m'

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo -e "${BLUE}[INFO]${RESET} ${BOLD}Executing Pipeline 2: Wasm Sandboxed Plugin Runtime...${RESET}"

echo -e "${BLUE}[INFO]${RESET} 1. Running cargo check on well-wasm..."
cargo check -p well-wasm

echo -e "${BLUE}[INFO]${RESET} 2. Running sandboxed WebAssembly execution tests..."
cargo test -p well-wasm -- --nocapture

echo -e "${GREEN}[SUCCESS]${RESET} ${BOLD}Pipeline 2: Wasm Sandboxed Plugin Runtime passed all validations!${RESET}"
