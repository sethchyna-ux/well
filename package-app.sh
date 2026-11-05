#!/usr/bin/env bash
# Build, sign, verify, and optionally archive/install the Well macOS app bundle.

set -euo pipefail

INFO='\033[0;34m[INFO]\033[0m'
SUCCESS='\033[0;32m[SUCCESS]\033[0m'
ERROR='\033[0;31m[ERROR]\033[0m'

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
APP_NAME="Well"
APP_BUNDLE="${DIST_DIR}/${APP_NAME}.app"
SIGN_IDENTITY="${WELL_SIGN_IDENTITY:--}"
BUILD_NUMBER="${WELL_BUILD_NUMBER:-1}"
MIN_MACOS_VERSION="${WELL_MIN_MACOS_VERSION:-12.0}"
INSTALL_APP=false
CREATE_ARCHIVE=false
CREATE_DMG=false
CREATE_CASK=false
NOTARIZE_APP=false
STAGING_ROOT=""

usage() {
    cat <<EOF
Usage: $0 [--archive] [--dmg] [--cask] [--notarize] [--install]

Builds a locked release binary and produces dist/Well.app.

Options:
  --archive   Create a versioned macOS zip and SHA-256 checksum in dist/.
  --dmg       Create a compressed UDZO disk image (.dmg) with /Applications link.
  --cask      Generate a Homebrew Cask in dist/Casks/well.rb from the archive.
  --notarize  Submit with notarytool, staple, validate, and create an archive.
  --install   Copy the verified app to /Applications/Well.app.

Environment:
  WELL_SIGN_IDENTITY    Developer ID Application identity; defaults to ad-hoc '-'.
  WELL_BUILD_NUMBER     Positive integer CFBundleVersion; defaults to 1.
  WELL_MIN_MACOS_VERSION
                        Deployment metadata version; defaults to 12.0.
  WELL_NOTARY_PROFILE   notarytool keychain profile required by --notarize.
  WELL_RELEASE_REPOSITORY
                        GitHub owner/repository for the generated Cask; defaults
                        to GitHub Actions metadata, a GitHub git remote, or the
                        Well release repository.
  WELL_RELEASE_TAG      GitHub release tag for the generated Cask; defaults to
                        v<version>.
  WELL_HOMEBREW_CASK_OUTPUT
                        Generated Cask path; defaults to dist/Casks/well.rb.
  WELL_VERSION          Override package version (normally read from Cargo.toml).
EOF
}

fail() {
    echo -e "${ERROR} $*" >&2
    exit 1
}

cleanup() {
    if [ -n "${STAGING_ROOT}" ] && [ -d "${STAGING_ROOT}" ]; then
        rm -rf "${STAGING_ROOT}"
    fi
}
trap cleanup EXIT HUP INT TERM

for arg in "$@"; do
    case "${arg}" in
        --archive)
            CREATE_ARCHIVE=true
            ;;
        --dmg)
            CREATE_DMG=true
            ;;
        --cask)
            CREATE_CASK=true
            CREATE_ARCHIVE=true
            ;;
        --notarize)
            NOTARIZE_APP=true
            CREATE_ARCHIVE=true
            ;;
        --install)
            INSTALL_APP=true
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

[ "$(uname -s)" = "Darwin" ] || fail "macOS packaging must run on macOS."

required_tools=(cargo codesign file install mktemp otool plutil sed xattr)
if [ "${CREATE_ARCHIVE}" = true ]; then
    required_tools+=(ditto shasum)
fi
if [ "${INSTALL_APP}" = true ]; then
    required_tools+=(ditto)
fi
if [ "${NOTARIZE_APP}" = true ]; then
    required_tools+=(xcrun)
fi
for tool in "${required_tools[@]}"; do
    command -v "${tool}" >/dev/null 2>&1 || fail "Required tool is unavailable: ${tool}"
done

for asset in \
    "assets/AppIcon.icns" \
    "assets/images/logo.png" \
    "assets/fonts/OpenDyslexicNerdFont-Regular.otf" \
    "packaging/macos/Info.plist.in"; do
    [ -s "${ROOT_DIR}/${asset}" ] || fail "Required packaging asset is missing or empty: ${asset}"
done

APP_VERSION="${WELL_VERSION:-$(sed -n '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n 1)}"
[[ "${APP_VERSION}" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || fail "Invalid app version: ${APP_VERSION}"
[[ "${BUILD_NUMBER}" =~ ^[1-9][0-9]*$ ]] || fail "WELL_BUILD_NUMBER must be a positive integer."
[[ "${MIN_MACOS_VERSION}" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || fail "Invalid WELL_MIN_MACOS_VERSION: ${MIN_MACOS_VERSION}"

if [ "${NOTARIZE_APP}" = true ]; then
    "${ROOT_DIR}/scripts/preflight-macos-release.sh" --notarize
fi

ARCH="$(uname -m)"
ARCHIVE_PATH="${DIST_DIR}/${APP_NAME}-${APP_VERSION}-macOS-${ARCH}.zip"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"
DMG_PATH="${DIST_DIR}/${APP_NAME}-${APP_VERSION}-macOS-${ARCH}.dmg"
DMG_CHECKSUM_PATH="${DMG_PATH}.sha256"

strip_finder_xattrs() {
    local target="$1"
    xattr -dr com.apple.FinderInfo "${target}" 2>/dev/null || true
    xattr -dr com.apple.ResourceFork "${target}" 2>/dev/null || true
    xattr -cr "${target}" 2>/dev/null || true
}

sign_path() {
    local path="$1"
    strip_finder_xattrs "${path}"

    if [ "${SIGN_IDENTITY}" = "-" ]; then
        codesign --force --sign - "${path}"
    else
        codesign --force --options runtime --timestamp --sign "${SIGN_IDENTITY}" "${path}"
    fi
}

verify_bundle() {
    "${ROOT_DIR}/scripts/verify-macos-bundle.sh" "$1"
}

create_dmg() {
    rm -f "${DMG_PATH}" "${DMG_CHECKSUM_PATH}"
    local dmg_stage
    dmg_stage="$(mktemp -d "${TMPDIR:-/tmp}/.well-dmg.XXXXXX")"
    ditto "${APP_BUNDLE}" "${dmg_stage}/${APP_NAME}.app"
    strip_finder_xattrs "${dmg_stage}/${APP_NAME}.app"
    ln -s /Applications "${dmg_stage}/Applications"
    hdiutil create \
        -volname "${APP_NAME} Terminal" \
        -srcfolder "${dmg_stage}" \
        -ov \
        -format UDZO \
        "${DMG_PATH}" >/dev/null
    rm -rf "${dmg_stage}"
    (
        cd "${DIST_DIR}"
        shasum -a 256 "$(basename "${DMG_PATH}")" > "$(basename "${DMG_CHECKSUM_PATH}")"
    )
    [ -s "${DMG_PATH}" ] || fail "Release DMG was not created."
    [ -s "${DMG_CHECKSUM_PATH}" ] || fail "Release DMG checksum was not created."
    echo -e "${SUCCESS} Created ${DMG_PATH}"
    echo -e "${SUCCESS} Created ${DMG_CHECKSUM_PATH}"
}

create_archive() {
    rm -f "${ARCHIVE_PATH}" "${CHECKSUM_PATH}"
    ditto -c -k --sequesterRsrc --keepParent "${APP_BUNDLE}" "${ARCHIVE_PATH}"
    (
        cd "${DIST_DIR}"
        shasum -a 256 "$(basename "${ARCHIVE_PATH}")" > "$(basename "${CHECKSUM_PATH}")"
    )
    [ -s "${ARCHIVE_PATH}" ] || fail "Release archive was not created."
    [ -s "${CHECKSUM_PATH}" ] || fail "Release checksum was not created."
    echo -e "${SUCCESS} Created ${ARCHIVE_PATH}"
    echo -e "${SUCCESS} Created ${CHECKSUM_PATH}"
}

generate_homebrew_cask() {
    local cask_output="${WELL_HOMEBREW_CASK_OUTPUT:-${DIST_DIR}/Casks/well.rb}"

    "${ROOT_DIR}/scripts/generate-homebrew-cask.sh" \
        --archive "${ARCHIVE_PATH}" \
        --checksum "${CHECKSUM_PATH}" \
        --version "${APP_VERSION}" \
        --output "${cask_output}"
    "${ROOT_DIR}/scripts/verify-homebrew-cask.sh" "${cask_output}"

    if [ "${NOTARIZE_APP}" != true ]; then
        echo -e "${INFO} The Cask reflects an ad-hoc build; publish only a Developer ID-signed and notarized Cask archive."
    fi
}

echo -e "${INFO} Building ${APP_NAME} ${APP_VERSION} (${BUILD_NUMBER}) for ${ARCH}..."
cd "${ROOT_DIR}"
BINARY_NAME="${WELL_BINARY_NAME:-well-gpui}"
MACOSX_DEPLOYMENT_TARGET="${MIN_MACOS_VERSION}" cargo build --release --locked -p "${BINARY_NAME}"

mkdir -p "${DIST_DIR}"
STAGING_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/.well-package.XXXXXX")"
STAGED_APP="${STAGING_ROOT}/${APP_NAME}.app"
CONTENTS="${STAGED_APP}/Contents"
MACOS_DIR="${CONTENTS}/MacOS"
RESOURCES_DIR="${CONTENTS}/Resources"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}/fonts"

install -m 755 "${ROOT_DIR}/target/release/${BINARY_NAME}" "${MACOS_DIR}/${APP_NAME}"
install -m 644 "${ROOT_DIR}/assets/AppIcon.icns" "${RESOURCES_DIR}/AppIcon.icns"
install -m 644 "${ROOT_DIR}/assets/images/logo.png" "${RESOURCES_DIR}/logo.png"
install -m 644 \
    "${ROOT_DIR}/assets/fonts/OpenDyslexicNerdFont-Regular.otf" \
    "${RESOURCES_DIR}/fonts/OpenDyslexicNerdFont-Regular.otf"

sed \
    -e "s/@VERSION@/${APP_VERSION}/g" \
    -e "s/@BUILD_NUMBER@/${BUILD_NUMBER}/g" \
    -e "s/@MIN_MACOS_VERSION@/${MIN_MACOS_VERSION}/g" \
    "${ROOT_DIR}/packaging/macos/Info.plist.in" > "${CONTENTS}/Info.plist"
plutil -lint "${CONTENTS}/Info.plist" >/dev/null

strip_finder_xattrs "${STAGED_APP}"
echo -e "${INFO} Signing with $([ "${SIGN_IDENTITY}" = "-" ] && echo 'an ad-hoc identity' || echo "${SIGN_IDENTITY}")..."
sign_path "${MACOS_DIR}/${APP_NAME}"
sign_path "${STAGED_APP}"
verify_bundle "${STAGED_APP}"

if [ -e "${APP_BUNDLE}" ]; then
    rm -rf "${APP_BUNDLE}"
fi
ditto "${STAGED_APP}" "${APP_BUNDLE}"
rm -rf "${STAGED_APP}"
strip_finder_xattrs "${APP_BUNDLE}"

if [ "${NOTARIZE_APP}" = true ]; then
    echo -e "${INFO} Submitting ${APP_NAME} to Apple's notary service..."
    create_archive
    xcrun notarytool submit "${ARCHIVE_PATH}" \
        --keychain-profile "${WELL_NOTARY_PROFILE}" \
        --wait
    xcrun stapler staple "${APP_BUNDLE}"
    xcrun stapler validate "${APP_BUNDLE}"
    WELL_REQUIRE_DEVELOPER_ID=1 WELL_REQUIRE_NOTARIZED=1 verify_bundle "${APP_BUNDLE}"
    create_archive
elif [ "${CREATE_ARCHIVE}" = true ]; then
    create_archive
fi

if [ "${CREATE_DMG}" = true ]; then
    create_dmg
fi

if [ "${CREATE_CASK}" = true ]; then
    generate_homebrew_cask
fi

if [ "${INSTALL_APP}" = true ]; then
    echo -e "${INFO} Installing the verified bundle to /Applications/Well.app..."
    if [ -e "/Applications/${APP_NAME}.app" ]; then
        rm -rf "/Applications/${APP_NAME}.app"
    fi
    ditto "${APP_BUNDLE}" "/Applications/${APP_NAME}.app"
    verify_bundle "/Applications/${APP_NAME}.app"
    touch "/Applications/${APP_NAME}.app"
    echo -e "${SUCCESS} Installed /Applications/${APP_NAME}.app"
fi

echo -e "${SUCCESS} Generated and verified ${APP_BUNDLE}"
echo "Launch locally with: open '${APP_BUNDLE}'"
echo "Runtime locations: docs/RUNTIME_LOCATIONS.md"
echo "Release signing/notarization: docs/RELEASE_SIGNING.md"
if [ "${CREATE_CASK}" = true ]; then
    echo "Homebrew distribution: docs/HOMEBREW.md"
fi
