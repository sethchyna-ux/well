#!/usr/bin/env bash
# ==============================================================================
# graphics-pipeline.sh
#
# Pipeline 3: Orpheus GPU Shaders & Native Texture Rendering Pipeline
# Validates WGSL CRT shader syntax, GPU text shaper, and UI artifact rendering.
# ==============================================================================

set -euo pipefail

BOLD=$'\033[1m'
RESET=$'\033[0m'
BLUE=$'\033[0;34m'
GREEN=$'\033[0;32m'

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo -e "${BLUE}[INFO]${RESET} ${BOLD}Executing Pipeline 3: Orpheus GPU Shaders & Graphics...${RESET}"

echo -e "${BLUE}[INFO]${RESET} 1. Verifying orpheus-crt-shader.wgsl integrity..."
if [ ! -f "orpheus-crt-shader.wgsl" ]; then
    echo "Error: orpheus-crt-shader.wgsl missing!" >&2
    exit 1
fi

echo -e "${BLUE}[INFO]${RESET} 2. Running cargo check on well-render and well-config..."
cargo check -p well-render -p well-config

echo -e "${BLUE}[INFO]${RESET} 3. Executing Orpheus GPU render and shader tests..."
cargo test -p well-render -- --nocapture

echo -e "${GREEN}[SUCCESS]${RESET} ${BOLD}Pipeline 3: Orpheus GPU Shaders & Graphics passed all validations!${RESET}"
