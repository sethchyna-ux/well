#!/usr/bin/env bash
set -euo pipefail

# package-android.sh – Automated Android Cross‑Compilation for Well.
# Builds CLI and FFI for arm64‑v8a and x86_64, copies assets, generates a debug keystore, and assembles APKs.

WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$WORKSPACE_ROOT"

# Ensure Rust & cargo‑ndk are on PATH
export PATH="/Users/yocan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH"

# Locate Android NDK
if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    if [[ -d "/opt/homebrew/Caskroom/android-ndk/29/AndroidNDK14206865.app/Contents/NDK" ]]; then
        export ANDROID_NDK_HOME="/opt/homebrew/Caskroom/android-ndk/29/AndroidNDK14206865.app/Contents/NDK"
    elif [[ -d "$HOME/Library/Android/sdk/ndk" ]]; then
        export ANDROID_NDK_HOME="$(ls -d "$HOME/Library/Android/sdk/ndk/"* 2>/dev/null | sort -V | tail -n1)"
    fi
fi

if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "[-] Error: Android NDK not found. Set ANDROID_NDK_HOME." >&2
    exit 1
fi

echo "[*] Using Android NDK: $ANDROID_NDK_HOME"
export NDK_HOME="$ANDROID_NDK_HOME"

TARGET_ABIS=("arm64-v8a" "x86_64")
API_LEVEL="26"
DIST_DIR="$WORKSPACE_ROOT/dist/android"
mkdir -p "$DIST_DIR"

# Map ABI to Rust triple using a case statement (compatible with Bash 3)
build_target() {
    local abi="$1"
    local triple=""
    case "$abi" in
        arm64-v8a) triple="aarch64-linux-android" ;;
        x86_64) triple="x86_64-linux-android" ;;
        *) echo "[!] Unsupported ABI: $abi"; exit 1 ;;
    esac
    echo "========================================================"
    echo "[*] Building well-cli for $abi..."

    cargo ndk -t "$abi" -P "$API_LEVEL" build -p well-cli --release
    cp "$WORKSPACE_ROOT/target/$triple/release/well-cli" "$DIST_DIR/well-cli-$abi"
    chmod +x "$DIST_DIR/well-cli-$abi"
    echo "[+] CLI for $abi built at $DIST_DIR/well-cli-$abi"

    echo "========================================================"
    echo "[*] Building well-ffi for $abi..."
    local jni_dir="$WORKSPACE_ROOT/app/android/app/src/main/jniLibs/$abi"
    mkdir -p "$jni_dir"
    cargo ndk -t "$abi" -P "$API_LEVEL" -o "$jni_dir" build -p well-ffi --release
    # Map Android ABI to the corresponding Rust target triple
    local triple
    case "$abi" in
        arm64-v8a) triple="aarch64-linux-android" ;;
        armeabi-v7a) triple="armv7-linux-androideabi" ;;
        x86_64) triple="x86_64-linux-android" ;;
        x86) triple="i686-linux-android" ;;
        *) triple="$abi" ;;
    esac
    cp "$WORKSPACE_ROOT/target/$triple/release/libwell_ffi.so" "$jni_dir/libwell_ffi.so"
    cp "$jni_dir/libwell_ffi.so" "$DIST_DIR/libwell_ffi-$abi.so"
    echo "[+] FFI for $abi built at $DIST_DIR/libwell_ffi-$abi.so"
}

case "${1:-all}" in
    cli)
        for abi in "${TARGET_ABIS[@]}"; do build_target "$abi"; done
        ;;
    ffi)
        for abi in "${TARGET_ABIS[@]}"; do build_target "$abi"; done
        ;;
    all)
        for abi in "${TARGET_ABIS[@]}"; do build_target "$abi"; done
        echo "========================================================"
        echo "[+] Android Cross‑Compilation Complete!"
        # Copy CLI binaries into Android assets per ABI
        for abi in "${TARGET_ABIS[@]}"; do
            asset_dir="$WORKSPACE_ROOT/app/android/app/src/main/assets/$abi"
            mkdir -p "$asset_dir"
            cp "$DIST_DIR/well-cli-$abi" "$asset_dir/well-cli"
            chmod +x "$asset_dir/well-cli"
            echo "[+] Copied well-cli for $abi to $asset_dir"
        done
        # Ensure a debug keystore exists for signing release builds
        keystore_path="$WORKSPACE_ROOT/app/android/release.keystore"
        if [[ ! -f "$keystore_path" ]]; then
            echo "[+] Generating debug keystore for signing"
            keytool -genkeypair -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
                -keystore "$keystore_path" -storepass android -keypass android \
                -dname "CN=Android Debug,O=Android,C=US" >/dev/null 2>&1
            echo "[+] Keystore generated at $keystore_path"
        fi
        # Ensure Gradle wrapper is executable
        # Ensure Gradle wrapper is executable and run from Android project root
        ANDROID_PROJECT_ROOT="$WORKSPACE_ROOT/app/android"
        GRADLEW_PATH="$ANDROID_PROJECT_ROOT/gradlew"
        if [[ ! -x "$GRADLEW_PATH" ]]; then
            chmod +x "$GRADLEW_PATH"
        fi
        # Build APKs (debug then release)
        echo "[+] Building debug APK..."
        (cd "$ANDROID_PROJECT_ROOT" && "$GRADLEW_PATH" :app:assembleDebug)
        echo "[+] Building release APK..."
        (cd "$ANDROID_PROJECT_ROOT" && "$GRADLEW_PATH" :app:assembleRelease)
        echo "[+] APKs generated in app/build/outputs/apk/"
        echo "    Artifacts stored in $DIST_DIR:"
        ls -lh "$DIST_DIR"
        echo "========================================================"
        ;;
    *)
        echo "Usage: $0 [cli|ffi|all]"
        exit 1
        ;;
esac
