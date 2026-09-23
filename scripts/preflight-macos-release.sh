#!/usr/bin/env bash
# Validate macOS packaging, signing, and notarization prerequisites before a
# release build spends time compiling or submitting artifacts.

set -euo pipefail

INFO='\033[0;34m[INFO]\033[0m'
SUCCESS='\033[0;32m[SUCCESS]\033[0m'
ERROR='\033[0;31m[ERROR]\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

CHECK_NOTARIZE=false
CHECK_NOTARY_SERVICE=true

usage() {
    cat <<EOF
Usage: $0 [--notarize] [--skip-notary-service-check]

Checks that the local macOS host has the tools, assets, metadata, signing
identity, and notarization profile needed to build Well release artifacts.

Options:
  --notarize                  Also require a Developer ID Application identity
                              and validate WELL_NOTARY_PROFILE with notarytool.
  --skip-notary-service-check Validate local inputs only; do not contact Apple's
                              notary service to test the keychain profile.
  --help, -h                  Show this help text.

Environment:
  WELL_SIGN_IDENTITY          Developer ID Application identity required by
                              --notarize.
  WELL_NOTARY_PROFILE         notarytool keychain profile required by --notarize.
  WELL_BUILD_NUMBER           Positive integer CFBundleVersion; defaults to 1.
  WELL_MIN_MACOS_VERSION      Deployment metadata version; defaults to 12.0.
  WELL_VERSION                Override package version; otherwise Cargo.toml.
EOF
}

fail() {
    echo -e "${ERROR} $*" >&2
    exit 1
}

info() {
    echo -e "${INFO} $*"
}

require_command() {
    local command_name="$1"
    command -v "${command_name}" >/dev/null 2>&1 || fail "Required command is unavailable: ${command_name}"
}

require_file() {
    local relative_path="$1"
    [ -s "${ROOT_DIR}/${relative_path}" ] || fail "Required packaging asset is missing or empty: ${relative_path}"
}

cargo_package_version() {
    sed -n '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n 1
}

for arg in "$@"; do
    case "${arg}" in
        --notarize)
            CHECK_NOTARIZE=true
            ;;
        --skip-notary-service-check)
            CHECK_NOTARY_SERVICE=false
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            usage >&2
            fail "Unknown argument: ${arg}"
            ;;
    esac
done

[ "$(uname -s)" = "Darwin" ] || fail "macOS release preflight must run on macOS."

required_tools=(cargo codesign ditto file install mktemp otool plutil sed shasum xattr)
if [ "${CHECK_NOTARIZE}" = true ]; then
    required_tools+=(security spctl xcrun)
fi

for tool in "${required_tools[@]}"; do
    require_command "${tool}"
done

require_file "Cargo.toml"
require_file "assets/AppIcon.icns"
require_file "assets/images/logo.png"
require_file "assets/fonts/OpenDyslexicNerdFont-Regular.otf"
require_file "packaging/macos/Info.plist.in"

APP_VERSION="${WELL_VERSION:-$(cargo_package_version)}"
BUILD_NUMBER="${WELL_BUILD_NUMBER:-1}"
MIN_MACOS_VERSION="${WELL_MIN_MACOS_VERSION:-12.0}"

[[ "${APP_VERSION}" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || fail "Invalid app version: ${APP_VERSION}"
[[ "${BUILD_NUMBER}" =~ ^[1-9][0-9]*$ ]] || fail "WELL_BUILD_NUMBER must be a positive integer."
[[ "${MIN_MACOS_VERSION}" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || fail "Invalid WELL_MIN_MACOS_VERSION: ${MIN_MACOS_VERSION}"

if [ "${CHECK_NOTARIZE}" = true ]; then
    SIGN_IDENTITY="${WELL_SIGN_IDENTITY:-}"
    [ -n "${SIGN_IDENTITY}" ] && [ "${SIGN_IDENTITY}" != "-" ] || fail "--notarize requires WELL_SIGN_IDENTITY with a Developer ID Application identity."
    case "${SIGN_IDENTITY}" in
        Developer\ ID\ Application:*) ;;
        *) fail "WELL_SIGN_IDENTITY must be a Developer ID Application identity for public releases." ;;
    esac

    IDENTITY_LIST="$(security find-identity -v -p codesigning 2>&1)" || fail "Could not inspect local code-signing identities."
    printf '%s\n' "${IDENTITY_LIST}" | grep -F -- "${SIGN_IDENTITY}" >/dev/null \
        || fail "Developer ID identity is not installed or valid: ${SIGN_IDENTITY}"

    [ -n "${WELL_NOTARY_PROFILE:-}" ] || fail "--notarize requires WELL_NOTARY_PROFILE."
    if [ "${CHECK_NOTARY_SERVICE}" = true ]; then
        info "Checking notarization keychain profile '${WELL_NOTARY_PROFILE}'..."
        xcrun notarytool history --keychain-profile "${WELL_NOTARY_PROFILE}" >/dev/null \
            || fail "Could not validate WELL_NOTARY_PROFILE with Apple's notary service: ${WELL_NOTARY_PROFILE}"
    else
        info "Skipping live notary service check by request."
    fi
fi

if [ "${CHECK_NOTARIZE}" = true ]; then
    echo -e "${SUCCESS} macOS release preflight passed for notarized Well ${APP_VERSION} (${BUILD_NUMBER})."
else
    echo -e "${SUCCESS} macOS release preflight passed for local Well ${APP_VERSION} (${BUILD_NUMBER})."
fi
