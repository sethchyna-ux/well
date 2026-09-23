#!/usr/bin/env bash
# Launch a packaged Well app under an isolated HOME and verify it survives startup.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_BUNDLE="${1:-${REPO_ROOT}/dist/Well.app}"
EXECUTABLE="${APP_BUNDLE}/Contents/MacOS/Well"
SMOKE_SECONDS="${WELL_GUI_SMOKE_SECONDS:-8}"
TEMP_HOME=""
APP_PID=""

fail() {
    echo "[ERROR] $*" >&2
    exit 1
}

cleanup() {
    if [ -n "${APP_PID}" ] && kill -0 "${APP_PID}" 2>/dev/null; then
        kill "${APP_PID}" 2>/dev/null || true
        wait "${APP_PID}" 2>/dev/null || true
    fi
    case "${TEMP_HOME}" in
        "${TMPDIR:-/tmp}"/well-gui-smoke.*)
            /bin/rm -R "${TEMP_HOME}" 2>/dev/null || true
            ;;
    esac
}
trap cleanup EXIT HUP INT TERM

[ -x "${EXECUTABLE}" ] || fail "Missing packaged executable: ${EXECUTABLE}"
[[ "${SMOKE_SECONDS}" =~ ^[1-9][0-9]*$ ]] || fail "WELL_GUI_SMOKE_SECONDS must be a positive integer"

TEMP_HOME="$(mktemp -d "${TMPDIR:-/tmp}/well-gui-smoke.XXXXXX")"

HOME="${TEMP_HOME}" \
RUST_BACKTRACE=1 \
"${EXECUTABLE}" >"${TEMP_HOME}/stdout.log" 2>"${TEMP_HOME}/stderr.log" &
APP_PID="$!"

sleep "${SMOKE_SECONDS}"

if kill -0 "${APP_PID}" 2>/dev/null; then
    kill "${APP_PID}" 2>/dev/null || true
    wait "${APP_PID}" 2>/dev/null || true
    APP_PID=""
    echo "[SUCCESS] Packaged GUI survived ${SMOKE_SECONDS}s clean-home startup: ${APP_BUNDLE}"
else
    set +e
    wait "${APP_PID}"
    exit_code="$?"
    set -e
    APP_PID=""
    echo "[ERROR] Packaged GUI exited during ${SMOKE_SECONDS}s startup smoke test with status ${exit_code}" >&2
    echo "[ERROR] stdout:" >&2
    sed -n '1,120p' "${TEMP_HOME}/stdout.log" >&2 || true
    echo "[ERROR] stderr:" >&2
    sed -n '1,160p' "${TEMP_HOME}/stderr.log" >&2 || true
    exit "${exit_code}"
fi
