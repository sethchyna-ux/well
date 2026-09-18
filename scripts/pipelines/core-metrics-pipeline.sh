#!/usr/bin/env bash
# ==============================================================================
# core-metrics-pipeline.sh
#
# Pipeline 1: Core Engine & Prometheus Telemetry Pipeline
# Validates Session Coordinator, RingBuffer scrollback, and Prometheus metrics export.
# ==============================================================================

set -euo pipefail

BOLD=$'\033[1m'
RESET=$'\033[0m'
BLUE=$'\033[0;34m'
GREEN=$'\033[0;32m'
ERROR=$'\033[0;31m'

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo -e "${BLUE}[INFO]${RESET} ${BOLD}Executing Pipeline 1: Core Engine & Telemetry...${RESET}"

echo -e "${BLUE}[INFO]${RESET} 1. Running cargo check on well-core and well-metrics..."
cargo check -p well-core -p well-metrics

echo -e "${BLUE}[INFO]${RESET} 2. Running unit tests on well-core (RingBuffer, SessionCoordinator)..."
cargo test -p well-core -- --nocapture

echo -e "${BLUE}[INFO]${RESET} 3. Running unit tests on well-metrics (Prometheus registries, histograms)..."
cargo test -p well-metrics -- --nocapture

echo -e "${GREEN}[SUCCESS]${RESET} ${BOLD}Pipeline 1: Core Engine & Telemetry passed all validations!${RESET}"
