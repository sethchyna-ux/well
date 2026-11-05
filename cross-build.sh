#!/usr/bin/env bash
set -euo pipefail

# Well Terminal ("Phrear") Fast Cross-Compilation Engine
# Uses Zig Toolchain for zero-Docker, native Apple Silicon speed cross-compilation

RUSTUP_BIN="$(rustup which rustc 2>/dev/null | xargs dirname || echo "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin")"
export PATH="${RUSTUP_BIN}:$HOME/.cargo/bin:$PATH"

TARGET="${1:-x86_64-unknown-linux-gnu}"
PROFILE="${2:---release}"

echo "⚡ [Well Cross-Compiler] Compiling for target '${TARGET}' (${PROFILE})..."
cargo zigbuild --target "${TARGET}" ${PROFILE}

OUTPUT_DIR="target/${TARGET}/$(echo "${PROFILE}" | sed 's/--//')"
echo "✅ [Well Cross-Compiler] Build succeeded!"
echo "📁 Artifacts located in: ${OUTPUT_DIR}/"
