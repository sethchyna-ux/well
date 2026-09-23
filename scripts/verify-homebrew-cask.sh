#!/usr/bin/env bash
# Verify the structural and style requirements of a generated Well Homebrew Cask.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CASK_PATH="${1:-${ROOT_DIR}/dist/Casks/well.rb}"

fail() {
    echo "error: $*" >&2
    exit 1
}

[ -f "${CASK_PATH}" ] || fail "Cask does not exist: ${CASK_PATH}"

for command in awk ruby; do
    command -v "${command}" >/dev/null 2>&1 || fail "Required command is unavailable: ${command}"
done

VERSION="$(awk -F'"' '/^[[:space:]]*version "/ { print $2; exit }' "${CASK_PATH}")"
SHA256="$(awk -F'"' '/^[[:space:]]*sha256 "/ { print $2; exit }' "${CASK_PATH}")"
URL="$(awk -F'"' '/^[[:space:]]*url "/ { print $2; exit }' "${CASK_PATH}")"

[[ "${VERSION}" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || fail "Cask has an invalid version"
[[ "${SHA256}" =~ ^[A-Fa-f0-9]{64}$ ]] || fail "Cask has an invalid SHA-256 checksum"
[[ "${URL}" =~ ^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/releases/download/[A-Za-z0-9._-]+/Well-${VERSION}-macOS-arm64\.zip$ ]] || \
    fail "Cask URL must target an immutable arm64 GitHub release asset for ${VERSION}"

grep -Fqx 'cask "well" do' "${CASK_PATH}" || fail "Cask token must be well"
grep -Fqx '  depends_on arch: :arm64' "${CASK_PATH}" || fail "Cask must declare arm64 support"
grep -Fqx '  depends_on macos: :monterey' "${CASK_PATH}" || fail "Cask must declare the macOS baseline"
grep -Fqx '  app "Well.app"' "${CASK_PATH}" || fail "Cask must install Well.app"
grep -Fqx '  binary "#{appdir}/Well.app/Contents/MacOS/Well", target: "well"' "${CASK_PATH}" || \
    fail "Cask must expose the well command"

ruby -c "${CASK_PATH}" >/dev/null

if command -v brew >/dev/null 2>&1; then
    brew ruby -e '
        require "cask/cask_loader"
        cask = Cask::CaskLoader::FromPathLoader.new(Pathname(ARGV.fetch(0))).load(config: nil)
        abort "Unexpected Cask token: #{cask.token}" unless cask.token == "well"
    ' "${CASK_PATH}" >/dev/null
fi

echo "Verified Homebrew Cask: ${CASK_PATH}"
