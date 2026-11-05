#!/usr/bin/env bash
# Generate a release-specific Homebrew Cask from a Well macOS archive.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TEMPLATE_PATH="${ROOT_DIR}/packaging/homebrew/Casks/well.rb.in"
DEFAULT_RELEASE_REPOSITORY="sethchyna-ux/well"

ARCHIVE_PATH=""
CHECKSUM_PATH=""
OUTPUT_PATH="${ROOT_DIR}/dist/Casks/well.rb"
VERSION="${WELL_VERSION:-}"
REPOSITORY="${WELL_RELEASE_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
RELEASE_TAG="${WELL_RELEASE_TAG:-}"

usage() {
    cat <<EOF
Usage: $0 --archive <path> [options]

Generate a Homebrew Cask for a versioned Well macOS archive. The archive and
checksum are verified before the cask is written.

Options:
  --archive <path>       Versioned Well ZIP to publish (required).
  --checksum <path>      SHA-256 checksum file; defaults to <archive>.sha256.
  --output <path>        Cask output path; defaults to dist/Casks/well.rb.
  --version <version>    Well version; defaults to WELL_VERSION or Cargo.toml.
  --repository <owner/repo>
                         GitHub repository; defaults to WELL_RELEASE_REPOSITORY
                         or GITHUB_REPOSITORY, then the first GitHub git remote.
  --tag <tag>            GitHub release tag; defaults to WELL_RELEASE_TAG or
                         v<version>.

Environment:
  WELL_RELEASE_REPOSITORY  GitHub owner/repository for release assets.
  GITHUB_REPOSITORY        GitHub Actions owner/repository fallback.
  WELL_RELEASE_TAG         GitHub release tag for release assets.
  WELL_VERSION             Override the version read from Cargo.toml.
EOF
}

fail() {
    echo "error: $*" >&2
    exit 1
}

cargo_package_version() {
    sed -n '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n 1
}

github_repository_from_remotes() {
    local remote remote_url repository

    while IFS= read -r remote; do
        remote_url="$(git -C "${ROOT_DIR}" remote get-url "${remote}" 2>/dev/null || true)"
        case "${remote_url}" in
            https://github.com/*)
                repository="${remote_url#https://github.com/}"
                ;;
            git@github.com:*)
                repository="${remote_url#git@github.com:}"
                ;;
            *)
                continue
                ;;
        esac
        repository="${repository%.git}"
        if [ -n "${repository}" ]; then
            if [[ "${repository}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
                printf '%s\n' "${repository}"
                return 0
            fi
        fi
    done < <(git -C "${ROOT_DIR}" remote 2>/dev/null)

    return 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --archive)
            [ "$#" -ge 2 ] || fail "--archive requires a path"
            ARCHIVE_PATH="$2"
            shift 2
            ;;
        --checksum)
            [ "$#" -ge 2 ] || fail "--checksum requires a path"
            CHECKSUM_PATH="$2"
            shift 2
            ;;
        --output)
            [ "$#" -ge 2 ] || fail "--output requires a path"
            OUTPUT_PATH="$2"
            shift 2
            ;;
        --version)
            [ "$#" -ge 2 ] || fail "--version requires a value"
            VERSION="$2"
            shift 2
            ;;
        --repository)
            [ "$#" -ge 2 ] || fail "--repository requires owner/repository"
            REPOSITORY="$2"
            shift 2
            ;;
        --tag)
            [ "$#" -ge 2 ] || fail "--tag requires a value"
            RELEASE_TAG="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            usage >&2
            fail "Unknown argument: $1"
            ;;
    esac
done

[ -n "${ARCHIVE_PATH}" ] || fail "--archive is required"
[ -f "${ARCHIVE_PATH}" ] || fail "Archive does not exist: ${ARCHIVE_PATH}"
[ -f "${TEMPLATE_PATH}" ] || fail "Cask template is missing: ${TEMPLATE_PATH}"

for command in awk git mktemp sed shasum; do
    command -v "${command}" >/dev/null 2>&1 || fail "Required command is unavailable: ${command}"
done

if [ -z "${CHECKSUM_PATH}" ]; then
    CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"
fi
[ -f "${CHECKSUM_PATH}" ] || fail "Checksum does not exist: ${CHECKSUM_PATH}"

if [ -z "${VERSION}" ]; then
    VERSION="$(cargo_package_version)"
fi
[[ "${VERSION}" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || fail "Invalid Well version: ${VERSION}"

if [ -z "${REPOSITORY}" ]; then
    REPOSITORY="$(github_repository_from_remotes || true)"
fi
if [ -z "${REPOSITORY}" ]; then
    REPOSITORY="${DEFAULT_RELEASE_REPOSITORY}"
fi
[[ "${REPOSITORY}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || \
    fail "A GitHub owner/repository is required; set WELL_RELEASE_REPOSITORY or pass --repository"

if [ -z "${RELEASE_TAG}" ]; then
    RELEASE_TAG="v${VERSION}"
fi
[[ "${RELEASE_TAG}" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || fail "Invalid release tag: ${RELEASE_TAG}"

ARCHIVE_NAME="$(basename "${ARCHIVE_PATH}")"
EXPECTED_ARCHIVE_NAME="Well-${VERSION}-macOS-arm64.zip"
[ "${ARCHIVE_NAME}" = "${EXPECTED_ARCHIVE_NAME}" ] || \
    fail "Expected an arm64 release archive named ${EXPECTED_ARCHIVE_NAME}, got ${ARCHIVE_NAME}"

EXPECTED_SHA256="$(awk 'NF { print $1; exit }' "${CHECKSUM_PATH}")"
[[ "${EXPECTED_SHA256}" =~ ^[A-Fa-f0-9]{64}$ ]] || fail "Invalid SHA-256 checksum: ${CHECKSUM_PATH}"
ACTUAL_SHA256="$(shasum -a 256 "${ARCHIVE_PATH}" | awk '{print $1}')"
[ "${ACTUAL_SHA256}" = "${EXPECTED_SHA256}" ] || \
    fail "Checksum does not match archive: ${ARCHIVE_PATH}"

OUTPUT_DIR="$(dirname "${OUTPUT_PATH}")"
mkdir -p "${OUTPUT_DIR}"
TEMP_OUTPUT="$(mktemp "${OUTPUT_DIR}/.well-cask.XXXXXX")"
trap 'rm -f "${TEMP_OUTPUT}"' EXIT HUP INT TERM

sed \
    -e "s|@VERSION@|${VERSION}|g" \
    -e "s|@SHA256@|${ACTUAL_SHA256}|g" \
    -e "s|@REPOSITORY@|${REPOSITORY}|g" \
    -e "s|@TAG@|${RELEASE_TAG}|g" \
    -e "s|@ARCHIVE@|${ARCHIVE_NAME}|g" \
    "${TEMPLATE_PATH}" > "${TEMP_OUTPUT}"

mv "${TEMP_OUTPUT}" "${OUTPUT_PATH}"
trap - EXIT HUP INT TERM

echo "Generated Homebrew Cask: ${OUTPUT_PATH}"
echo "Release asset: https://github.com/${REPOSITORY}/releases/download/${RELEASE_TAG}/${ARCHIVE_NAME}"
