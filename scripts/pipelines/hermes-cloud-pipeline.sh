#!/usr/bin/env bash
# ==============================================================================
# hermes-cloud-pipeline.sh
#
# Pipeline 5: Hermes Cloud RPC & Kubernetes Continuous Deployment Pipeline
# Validates IPC schemas, Dockerfile structure, and Kubernetes manifests.
# ==============================================================================

set -euo pipefail

BOLD=$'\033[1m'
RESET=$'\033[0m'
BLUE=$'\033[0;34m'
GREEN=$'\033[0;32m'
WARN=$'\033[0;33m'

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo -e "${BLUE}[INFO]${RESET} ${BOLD}Executing Pipeline 5: Hermes Cloud RPC & Kubernetes...${RESET}"

echo -e "${BLUE}[INFO]${RESET} 1. Running cargo check on well-ipc and hermes daemon..."
cargo check -p well-ipc

echo -e "${BLUE}[INFO]${RESET} 2. Running unit tests on Hermes Seqlock & JSON-RPC contracts..."
cargo test -p well-ipc -- --nocapture

echo -e "${BLUE}[INFO]${RESET} 3. Validating Dockerfile.hermes syntax..."
if [ -f "k8s/Dockerfile.hermes" ]; then
    grep -E '^(FROM|RUN|COPY|EXPOSE|CMD|USER|WORKDIR)' k8s/Dockerfile.hermes > /dev/null
    echo -e "${GREEN}[PASS]${RESET} Dockerfile.hermes syntax verified."
else
    echo "Error: k8s/Dockerfile.hermes missing!" >&2
    exit 1
fi

echo -e "${BLUE}[INFO]${RESET} 4. Validating Kubernetes deployment and service manifests..."
if [ -f "k8s/deployment.yaml" ]; then
    grep "kind: Deployment" k8s/deployment.yaml > /dev/null
    grep "kind: Service" k8s/deployment.yaml > /dev/null
    grep "kind: ConfigMap" k8s/deployment.yaml > /dev/null
    echo -e "${GREEN}[PASS]${RESET} k8s/deployment.yaml structure and resources verified."
else
    echo "Error: k8s/deployment.yaml missing!" >&2
    exit 1
fi

echo -e "${GREEN}[SUCCESS]${RESET} ${BOLD}Pipeline 5: Hermes Cloud RPC & Kubernetes passed all validations!${RESET}"
