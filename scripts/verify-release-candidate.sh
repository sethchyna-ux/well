#!/usr/bin/env bash
# Verify that a packaged Well app can perform non-GUI release diagnostics from clean state.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_BUNDLE="${1:-${REPO_ROOT}/dist/Well.app}"
EXECUTABLE="${APP_BUNDLE}/Contents/MacOS/Well"
TEMP_HOME=""

fail() {
    echo "[ERROR] $*" >&2
    exit 1
}

cleanup() {
    case "${TEMP_HOME}" in
        "${TMPDIR:-/tmp}"/well-release-candidate.*)
            rm -rf "${TEMP_HOME}"
            ;;
    esac
}
trap cleanup EXIT HUP INT TERM

"${SCRIPT_DIR}/verify-macos-bundle.sh" "${APP_BUNDLE}"
[ -x "${EXECUTABLE}" ] || fail "Missing packaged executable: ${EXECUTABLE}"
command -v jq >/dev/null 2>&1 || fail "jq is required to validate diagnostics"

TEMP_HOME="$(mktemp -d "${TMPDIR:-/tmp}/well-release-candidate.XXXXXX")"
DIAGNOSTIC_REPORT="${TEMP_HOME}/well-diagnostics.json"

VERSION_OUTPUT="$(HOME="${TEMP_HOME}" "${EXECUTABLE}" --version)"
case "${VERSION_OUTPUT}" in
    "Well "*) ;;
    *) fail "Unexpected version output: ${VERSION_OUTPUT}" ;;
esac

HOME="${TEMP_HOME}" \
GEMINI_API_KEY= \
HF_TOKEN= \
"${EXECUTABLE}" --diagnose > "${DIAGNOSTIC_REPORT}"

jq -e '
    .schema_version == 1 and
    .application.macos_app_bundle == true and
    .application.executable == "Well" and
    .privacy.credential_values_included == false and
    .privacy.terminal_output_included == false and
    .privacy.environment_values_included == false and
    .credentials.gemini_environment_present == false and
    .credentials.hugging_face_environment_present == false and
    .configuration.status == "missing (defaults active)" and
    (.runtime | has("shell") | not)
' "${DIAGNOSTIC_REPORT}" >/dev/null || fail "Diagnostic report is invalid or failed privacy and clean-state assertions"

[ ! -e "${TEMP_HOME}/.config" ] || fail "Diagnostics unexpectedly created a config directory"
[ ! -e "${TEMP_HOME}/.well" ] || fail "Diagnostics unexpectedly created runtime state"

echo "[SUCCESS] Release candidate passed clean-state diagnostics: ${VERSION_OUTPUT}"
