#!/usr/bin/env bash
# Build and verify the checked-in Well sources without generating/replacing them.
set -euo pipefail

well_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
well_target=${WELL_BUILD_TARGET_DIR:-"${TMPDIR:-/tmp}/well-release-build"}
well_profile=release
well_verify=1
well_package=0
well_destination="$well_root/dist"

usage() {
    cat <<'HELP'
Usage: ./well-build-harness.sh [--debug] [--skip-tests] [--package] [--output DIR]

Builds the checked-in Cargo workspace. By default runs workspace checks and
all tests, then builds the optimized Well binary. --package creates a macOS
.app/zip or an Arch Linux x86_64 package in DIR (default: ./dist).

WELL_BUILD_TARGET_DIR selects the build cache outside the checkout.
No source files, manifests, or configuration files are generated or overwritten.
HELP
}
while (($#)); do
    case "$1" in
        --debug) well_profile=debug ;;
        --skip-tests) well_verify=0 ;;
        --package) well_package=1 ;;
        --output) shift; if (($# == 0)); then printf '%s\n' '--output needs a directory' >&2; exit 2; fi; well_destination=$1 ;;
        --help|-h) usage; exit 0 ;;
        *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done
command -v cargo >/dev/null || { printf '%s\n' 'Cargo is required.' >&2; exit 1; }
cd -- "$well_root"
if ((well_verify)); then
    cargo check --workspace --locked --target-dir "$well_target"
    cargo test --workspace --locked --target-dir "$well_target"
fi
if [[ $well_profile == release ]]; then
    cargo build --bin well --release --locked --target-dir "$well_target"
else
    cargo build --bin well --locked --target-dir "$well_target"
fi
printf 'Built %s/%s/well\n' "$well_target" "$well_profile"
if ((well_package)); then
    well_package_id=$(cargo pkgid --manifest-path "$well_root/Cargo.toml" --locked)
    well_version=${well_package_id##*#}
    well_version=${well_version##*@}
    mkdir -p -- "$well_destination"
    well_destination=$(cd -- "$well_destination" && pwd)
    # Every package is assembled in a new staging directory; existing builds survive.
    well_stage=$(mktemp -d "$well_destination/.well-package.XXXXXX")
    if [[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]; then
        command -v makepkg >/dev/null || { printf '%s\n' 'Arch Linux packaging requires makepkg (pacman -S base-devel).' >&2; exit 1; }
        if [[ $(id -u) == 0 ]]; then printf '%s\n' 'Run Arch packaging as an unprivileged user, as required by makepkg.' >&2; exit 1; fi
        cp "$well_target/$well_profile/well" "$well_stage/well"
        cp "$well_root/packaging/org.well.Terminal.desktop" "$well_stage/"
        cp "$well_root/README.md" "$well_stage/"
        cp "$well_root/docs/well-terminal.md" "$well_stage/"
        cp "$well_root/docs/release-readiness.md" "$well_stage/"
        cp "$well_root/docs/automation.md" "$well_stage/"
        # Source checksums describe exactly the files placed in this package.
        well_arch_version=$(printf '%s' "$well_version" | sed 's/[^[:alnum:].]/./g')
        well_archive="$well_destination/well-$well_arch_version-1-x86_64.pkg.tar.zst"
        if [[ -e $well_archive || -e $well_archive.sha256 ]]; then
            printf 'Refusing to replace an existing package: %s\n' "$well_archive" >&2; exit 1
        fi
        printf 'pkgver=%q\n' "$well_arch_version" > "$well_stage/PKGBUILD"
        printf 'sha256sums=(' >> "$well_stage/PKGBUILD"
        for well_source in well org.well.Terminal.desktop README.md well-terminal.md release-readiness.md automation.md; do
            well_checksum=$(sha256sum "$well_stage/$well_source")
            printf "'%s' " "${well_checksum%% *}" >> "$well_stage/PKGBUILD"
        done
        printf ')\n' >> "$well_stage/PKGBUILD"
        cat "$well_root/packaging/arch/package.bash" >> "$well_stage/PKGBUILD"
        if [[ -f "$well_root/LICENSE" ]]; then
            cp "$well_root/LICENSE" "$well_stage/"
            well_checksum=$(sha256sum "$well_stage/LICENSE")
            printf "source+=('LICENSE')\nsha256sums+=('%s')\n" "${well_checksum%% *}" >> "$well_stage/PKGBUILD"
        fi
        (cd -- "$well_stage" && PKGDEST="$well_destination" makepkg --cleanbuild --noconfirm)
        sha256sum "$well_archive" > "$well_archive.sha256"
        printf 'Packaged %s\n' "$well_archive"
        exit 0
    elif [[ $(uname -s) != Darwin ]]; then
        printf '%s\n' 'Packaging supports macOS and Arch Linux x86_64 only.' >&2; exit 1
    fi
    command -v codesign >/dev/null || { printf '%s\n' 'codesign is required for the app bundle.' >&2; exit 1; }
    well_bundle="$well_stage/Well.app"
    mkdir -p "$well_bundle/Contents/MacOS" "$well_bundle/Contents/Resources/docs"
    cp "$well_target/$well_profile/well" "$well_bundle/Contents/MacOS/well"
    cp "$well_root/README.md" "$well_bundle/Contents/Resources/"
    cp "$well_root/docs/well-terminal.md" "$well_bundle/Contents/Resources/docs/"
    cp "$well_root/docs/release-readiness.md" "$well_bundle/Contents/Resources/docs/"
    cp "$well_root/docs/automation.md" "$well_bundle/Contents/Resources/docs/"
    if [[ -f "$well_root/LICENSE" ]]; then cp "$well_root/LICENSE" "$well_bundle/Contents/Resources/"; fi
    cat > "$well_bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.well.terminal</string>
<key>CFBundleName</key><string>Well</string>
<key>CFBundleDisplayName</key><string>Well</string>
<key>CFBundleExecutable</key><string>well</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$well_version</string>
<key>CFBundleVersion</key><string>$well_version</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PLIST
    plutil -lint "$well_bundle/Contents/Info.plist"
    codesign --force --sign "${WELL_SIGNING_IDENTITY:--}" "$well_bundle"
    codesign --verify --strict "$well_bundle"
    well_arch=$(uname -m)
    well_archive="$well_destination/Well-$well_version-$well_arch-$(date -u +%Y%m%dT%H%M%SZ).zip"
    if [[ -e $well_archive || -e $well_archive.sha256 ]]; then
        printf 'Refusing to replace an existing package: %s\n' "$well_archive" >&2
        exit 1
    fi
    ditto -c -k --keepParent "$well_bundle" "$well_archive"
    shasum -a 256 "$well_archive" > "$well_archive.sha256"
    printf 'Packaged %s\nApp bundle: %s\n' "$well_archive" "$well_bundle"
fi
