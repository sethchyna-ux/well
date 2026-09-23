#!/usr/bin/env bash
# verify-macos-bundle.sh
#
# Validates the structure, metadata, binary, resources, and signature of a
# packaged Well macOS app bundle.

set -euo pipefail

INFO='\033[0;34m[INFO]\033[0m'
SUCCESS='\033[0;32m[SUCCESS]\033[0m'
ERROR='\033[0;31m[ERROR]\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_BUNDLE="${1:-dist/Well.app}"
CONTENTS="${APP_BUNDLE}/Contents"
MACOS_DIR="${CONTENTS}/MacOS"
RESOURCES_DIR="${CONTENTS}/Resources"
EXECUTABLE="${MACOS_DIR}/Well"
PLIST="${CONTENTS}/Info.plist"
CARGO_TOML="${REPO_ROOT}/Cargo.toml"

fail() {
    echo -e "${ERROR} $*" >&2
    exit 1
}

require_file() {
    local path="$1"
    [ -f "${path}" ] || fail "Missing file: ${path}"
}

require_dir() {
    local path="$1"
    [ -d "${path}" ] || fail "Missing directory: ${path}"
}

require_command() {
    local command_name="$1"
    command -v "${command_name}" >/dev/null 2>&1 || fail "Required command is unavailable: ${command_name}"
}

plist_value() {
    local key="$1"
    /usr/libexec/PlistBuddy -c "Print :${key}" "${PLIST}" 2>/dev/null || fail "Missing or unreadable Info.plist key: ${key}"
}

cargo_package_version() {
    awk '
        /^\[package\][[:space:]]*$/ {
            in_package = 1
            next
        }
        in_package && /^\[/ {
            exit
        }
        in_package && /^[[:space:]]*version[[:space:]]*=/ {
            line = $0
            sub(/^[^=]*=[[:space:]]*"/, "", line)
            sub(/".*$/, "", line)
            print line
            exit
        }
    ' "${CARGO_TOML}"
}

architecture_present() {
    local requested_architecture="$1"
    local available_architecture

    for available_architecture in ${EXECUTABLE_ARCHITECTURES}; do
        if [ "${available_architecture}" = "${requested_architecture}" ]; then
            return 0
        fi
    done

    return 1
}

normalize_architecture() {
    case "$1" in
        aarch64)
            echo "arm64"
            ;;
        amd64)
            echo "x86_64"
            ;;
        *)
            echo "$1"
            ;;
    esac
}

validate_runtime_path() {
    local runtime_path="$1"
    local path_kind="$2"

    case "${runtime_path}" in
        *"${REPO_ROOT}"*) fail "Executable ${path_kind} points into the build worktree: ${runtime_path}" ;;
    esac
    case "${runtime_path}" in
        */target/*|*/target) fail "Executable ${path_kind} points into a Cargo target directory: ${runtime_path}" ;;
    esac
    case "${runtime_path}" in
        /opt/homebrew/*|/usr/local/Cellar/*|/usr/local/opt/*)
            fail "Executable ${path_kind} points into Homebrew: ${runtime_path}"
            ;;
    esac
    case "${runtime_path}" in
        /System/Library/*|/usr/lib/*|@rpath/*|@loader_path/*|@executable_path/*) ;;
        /*) fail "Executable contains a non-system absolute ${path_kind} that may be build-machine-specific: ${runtime_path}" ;;
    esac
}

echo -e "${INFO} Verifying macOS app bundle: ${APP_BUNDLE}"

case "${WELL_REQUIRE_DEVELOPER_ID:-0}" in
    0|1) ;;
    *) fail "WELL_REQUIRE_DEVELOPER_ID must be 0 or 1" ;;
esac

case "${WELL_REQUIRE_NOTARIZED:-0}" in
    0|1) ;;
    *) fail "WELL_REQUIRE_NOTARIZED must be 0 or 1" ;;
esac

require_command plutil
require_command codesign
require_command file
require_command lipo
require_command otool
require_file "${CARGO_TOML}"

require_dir "${APP_BUNDLE}"
require_dir "${CONTENTS}"
require_dir "${MACOS_DIR}"
require_dir "${RESOURCES_DIR}"
require_file "${PLIST}"
require_file "${EXECUTABLE}"
require_file "${RESOURCES_DIR}/AppIcon.icns"
require_file "${RESOURCES_DIR}/logo.png"
require_file "${RESOURCES_DIR}/fonts/OpenDyslexicNerdFont-Regular.otf"

[ -x "${EXECUTABLE}" ] || fail "Executable bit is not set: ${EXECUTABLE}"
[ -s "${RESOURCES_DIR}/AppIcon.icns" ] || fail "Required resource is empty: ${RESOURCES_DIR}/AppIcon.icns"
[ -s "${RESOURCES_DIR}/logo.png" ] || fail "Required resource is empty: ${RESOURCES_DIR}/logo.png"
[ -s "${RESOURCES_DIR}/fonts/OpenDyslexicNerdFont-Regular.otf" ] || fail "Required resource is empty: ${RESOURCES_DIR}/fonts/OpenDyslexicNerdFont-Regular.otf"

plutil -lint "${PLIST}" >/dev/null

[ "$(plist_value CFBundleExecutable)" = "Well" ] || fail "CFBundleExecutable must be Well"
[ "$(plist_value CFBundleIdentifier)" = "org.well.terminal" ] || fail "CFBundleIdentifier must be org.well.terminal"
[ "$(plist_value CFBundlePackageType)" = "APPL" ] || fail "CFBundlePackageType must be APPL"
EXPECTED_MIN_MACOS_VERSION="${WELL_MIN_MACOS_VERSION:-12.0}"
[ "$(plist_value LSMinimumSystemVersion)" = "${EXPECTED_MIN_MACOS_VERSION}" ] || fail "LSMinimumSystemVersion must be ${EXPECTED_MIN_MACOS_VERSION}"

EXPECTED_VERSION="${WELL_VERSION:-$(cargo_package_version)}"
[ -n "${EXPECTED_VERSION}" ] || fail "Could not determine the root Cargo package version from ${CARGO_TOML}"
BUNDLE_VERSION="$(plist_value CFBundleShortVersionString)"
[ "${BUNDLE_VERSION}" = "${EXPECTED_VERSION}" ] || fail "CFBundleShortVersionString is ${BUNDLE_VERSION}; expected ${EXPECTED_VERSION}"

BUNDLE_BUILD_NUMBER="$(plist_value CFBundleVersion)"
case "${BUNDLE_BUILD_NUMBER}" in
    ''|*[!0-9]*)
        fail "CFBundleVersion must contain only decimal digits; found: ${BUNDLE_BUILD_NUMBER}"
        ;;
esac

EXECUTABLE_FILE_TYPE="$(file -b "${EXECUTABLE}")" || fail "Could not inspect executable type: ${EXECUTABLE}"
case "${EXECUTABLE_FILE_TYPE}" in
    *Mach-O*) ;;
    *) fail "Bundle executable is not a Mach-O binary: ${EXECUTABLE_FILE_TYPE}" ;;
esac

EXECUTABLE_ARCHITECTURES="$(lipo -archs "${EXECUTABLE}" 2>/dev/null)" || fail "Could not read executable architectures: ${EXECUTABLE}"
[ -n "${EXECUTABLE_ARCHITECTURES}" ] || fail "Bundle executable declares no architectures"

HOST_ARCHITECTURE="$(normalize_architecture "$(uname -m)")"
architecture_present "${HOST_ARCHITECTURE}" || fail "Bundle architectures (${EXECUTABLE_ARCHITECTURES}) do not include host architecture ${HOST_ARCHITECTURE}"

DECLARED_ARCHITECTURES="$(/usr/libexec/PlistBuddy -c 'Print :LSArchitecturePriority' "${PLIST}" 2>/dev/null || true)"
if [ -n "${DECLARED_ARCHITECTURES}" ]; then
    case "${DECLARED_ARCHITECTURES}" in
        'Array {'*) ;;
        *) fail "LSArchitecturePriority must be an array when present" ;;
    esac

    while IFS= read -r declared_architecture; do
        declared_architecture="$(echo "${declared_architecture}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
        case "${declared_architecture}" in
            ''|'Array {'|'}') continue ;;
        esac
        declared_architecture="$(normalize_architecture "${declared_architecture}")"
        architecture_present "${declared_architecture}" || fail "LSArchitecturePriority declares ${declared_architecture}, but the executable contains only: ${EXECUTABLE_ARCHITECTURES}"
    done <<EOF
${DECLARED_ARCHITECTURES}
EOF
fi

OTOOL_LIBRARIES="$(otool -L "${EXECUTABLE}" 2>&1)" || fail "Could not inspect executable dependencies: ${OTOOL_LIBRARIES}"
OTOOL_LOAD_COMMANDS="$(otool -l "${EXECUTABLE}" 2>&1)" || fail "Could not inspect executable load commands: ${OTOOL_LOAD_COMMANDS}"

while IFS= read -r dependency; do
    dependency="$(echo "${dependency}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*(compatibility version.*$//')"
    [ -n "${dependency}" ] || continue
    validate_runtime_path "${dependency}" "dependency"
done <<EOF
$(echo "${OTOOL_LIBRARIES}" | sed '1d')
EOF

while IFS= read -r load_path; do
    load_path="$(echo "${load_path}" | sed -e 's/[[:space:]]*(offset [0-9][0-9]*)[[:space:]]*$//')"
    [ -n "${load_path}" ] || continue
    validate_runtime_path "${load_path}" "runtime search path"
done <<EOF
$(echo "${OTOOL_LOAD_COMMANDS}" | sed -n 's/^[[:space:]]*path //p')
EOF

codesign --verify --deep --strict --verbose=2 "${APP_BUNDLE}"

if [ "${WELL_REQUIRE_DEVELOPER_ID:-0}" = "1" ]; then
    SIGNATURE_DETAILS="$(codesign --display --verbose=4 "${APP_BUNDLE}" 2>&1)" || fail "Could not inspect the app bundle signature"
    case "${SIGNATURE_DETAILS}" in
        *'Authority=Developer ID Application:'*) ;;
        *) fail "App bundle is not signed with a Developer ID Application certificate" ;;
    esac
fi

if [ "${WELL_REQUIRE_NOTARIZED:-0}" = "1" ]; then
    require_command xcrun
    require_command spctl
    xcrun stapler validate "${APP_BUNDLE}" >/dev/null || fail "App bundle does not contain a valid notarization ticket"
    spctl --assess --type execute --verbose=4 "${APP_BUNDLE}" || fail "Gatekeeper assessment failed for the notarized app bundle"
fi

echo -e "${SUCCESS} Bundle verification passed: ${APP_BUNDLE}"
