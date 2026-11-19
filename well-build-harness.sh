#!/usr/bin/env bash
# Locked GPUI + companion verification and native artifact packaging.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${WELL_BUILD_TARGET_DIR:-${ROOT_DIR}/target}"
OUTPUT_DIR="${ROOT_DIR}/dist"
SKIP_TESTS=false
PACKAGE=false
BENCHMARK=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --skip-tests) SKIP_TESTS=true ;;
        --package) PACKAGE=true ;;
        --output) shift; OUTPUT_DIR="${1:?--output requires a directory}" ;;
        --benchmark|-b) BENCHMARK=true ;;
        --help|-h) echo 'Usage: well-build-harness.sh [--skip-tests] [--package] [--output DIR] [--benchmark]'; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
    shift
done
cd "${ROOT_DIR}"
cargo check --workspace --all-targets --locked --target-dir "${BUILD_DIR}"
cargo check --manifest-path crates/well-boot/Cargo.toml --all-targets --locked --target-dir "${BUILD_DIR}"
if [ "${SKIP_TESTS}" != true ]; then
    cargo test --workspace --locked --target-dir "${BUILD_DIR}"
    cargo test --manifest-path crates/well-boot/Cargo.toml --locked --target-dir "${BUILD_DIR}"
fi
if [ "${BENCHMARK}" = true ]; then ./well-benchmarks.sh; fi
if [ "${PACKAGE}" = true ]; then
    mkdir -p "${OUTPUT_DIR}"
    case "$(uname -s)" in
        Darwin)
            WELL_BUILD_TARGET_DIR="${BUILD_DIR}" ./package-app.sh --archive
            if [ "$(cd "${OUTPUT_DIR}" && pwd)" != "${ROOT_DIR}/dist" ]; then
                cp "${ROOT_DIR}"/dist/Well-*.zip "${ROOT_DIR}"/dist/Well-*.zip.sha256 "${OUTPUT_DIR}/"
            fi
            ;;
        Linux)
            cargo build --release --locked -p well-gpui --target-dir "${BUILD_DIR}"
            cargo build --release --locked --manifest-path crates/well-boot/Cargo.toml --target-dir "${BUILD_DIR}"
            STAGING="$(mktemp -d "${TMPDIR:-/tmp}/well-package.XXXXXX")"
            trap 'rm -rf "${STAGING}"' EXIT
            mkdir -p "${STAGING}/bin" "${STAGING}/share/well/agentos-loader/src"
            install -m 755 "${BUILD_DIR}/release/well-gpui" "${BUILD_DIR}/release/well-boot" "${STAGING}/bin/"
            install -m 644 boot/agentos/build.zig "${STAGING}/share/well/agentos-loader/"
            install -m 644 boot/agentos/src/main.zig "${STAGING}/share/well/agentos-loader/src/"
            ARCHIVE="${OUTPUT_DIR}/Well-linux-$(uname -m).tar.gz"
            tar -czf "${ARCHIVE}" -C "${STAGING}" bin share
            sha256sum "${ARCHIVE}" > "${ARCHIVE}.sha256"
            ;;
        *) echo 'Native packaging supports macOS and Linux' >&2; exit 1 ;;
    esac
fi
echo 'Well GPUI and boot companion checks completed.'
