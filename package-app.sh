#!/usr/bin/env bash
# package-app.sh
#
# Packages the compiled release binary of Well into a standalone macOS Application Bundle (Well.app).

set -euo pipefail

INFO='\033[0;34m[INFO]\033[0m'
SUCCESS='\033[0;32m[SUCCESS]\033[0m'
ERROR='\033[0;31m[ERROR]\033[0m'

APP_NAME="Well"
DIST_DIR="dist"
APP_BUNDLE="${DIST_DIR}/${APP_NAME}.app"
CONTENTS="${APP_BUNDLE}/Contents"
MACOS_DIR="${CONTENTS}/MacOS"
RESOURCES_DIR="${CONTENTS}/Resources"

echo -e "${INFO} Packaging ${APP_NAME}.app bundle..."

# 1. Ensure release binary exists
if [ ! -f "target/release/well" ]; then
    echo -e "${INFO} Compiling release binary with cargo..."
    cargo build --release
fi

# 2. Re-create bundle directory structure
rm -rf "${APP_BUNDLE}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

# 3. Copy executable
cp "target/release/well" "${MACOS_DIR}/${APP_NAME}"
chmod +x "${MACOS_DIR}/${APP_NAME}"

# 4. Create Info.plist
cat << 'EOF' > "${CONTENTS}/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Well</string>
    <key>CFBundleIdentifier</key>
    <string>org.well.terminal</string>
    <key>CFBundleName</key>
    <string>Well</string>
    <key>CFBundleDisplayName</key>
    <string>Well Terminal</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
</dict>
</plist>
EOF

# 5. Copy font assets if present
if [ -d "assets/fonts" ]; then
    mkdir -p "${RESOURCES_DIR}/fonts"
    cp -R assets/fonts/* "${RESOURCES_DIR}/fonts/" 2>/dev/null || true
fi

echo -e "${SUCCESS} Successfully generated ${APP_BUNDLE}!"
echo -e "You can test-launch it via: open ${APP_BUNDLE}"
echo -e "Or install to Applications: cp -R ${APP_BUNDLE} /Applications/"
