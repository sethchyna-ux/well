#!/usr/bin/env bash
# Deploy or dry-run the custom well-helper agent using google-agents-cli
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
AGENT_DIR="${ROOT_DIR}/agents/well-helper"

PROJECT_ID="${GCP_PROJECT:-$(gcloud config get-value project 2>/dev/null || echo "agent-os-510817")}"
REGION="${GCP_REGION:-us-east1}"

echo "=== Well Custom Agent Deployment ==="
echo "Target: ${AGENT_DIR}"
echo "Project: ${PROJECT_ID}"
echo "Region: ${REGION}"

if [[ ! -d "${AGENT_DIR}" ]]; then
  echo "Error: Agent directory not found: ${AGENT_DIR}" >&2
  exit 1
fi

MODE="${1:---dry-run}"

cd "${AGENT_DIR}"

if [[ "${MODE}" == "--live" ]]; then
  echo "Executing live deployment to Agent Runtime..."
  agents-cli deploy --project "${PROJECT_ID}" --region "${REGION}" --no-confirm-project
else
  echo "Executing deployment preflight verification (dry-run)..."
  agents-cli deploy --dry-run --project "${PROJECT_ID}" --region "${REGION}" --no-confirm-project
  echo "Preflight check passed. To deploy live, run: ./scripts/deploy-well-helper.sh --live"
fi
