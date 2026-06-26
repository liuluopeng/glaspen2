#!/bin/bash
set -e

APP_NAME="glaspen2"
DMG_NAME="${APP_NAME}.dmg"
VOLUME_NAME="Glaspen2"
BUILD_DIR="target/release"
APP_DIR="/tmp/${APP_NAME}-dmg"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version *= *"\(.*\)"/\1/')

echo "Building release..."
cargo build --release

echo "Creating app structure..."
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/${APP_NAME}.app/Contents/MacOS"
mkdir -p "${APP_DIR}/${APP_NAME}.app/Contents/Resources"

cp "${BUILD_DIR}/${APP_NAME}" "${APP_DIR}/${APP_NAME}.app/Contents/MacOS/"
cp "glaspen2.icns" "${APP_DIR}/${APP_NAME}.app/Contents/Resources/"

# Create Info.plist
cat > "${APP_DIR}/${APP_NAME}.app/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>glaspen2</string>
    <key>CFBundleIdentifier</key>
    <string>com.glaspen2.app</string>
    <key>CFBundleName</key>
    <string>Glaspen2</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleIconFile</key>
    <string>glaspen2</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSUIElement</key>
    <false/>
</dict>
</plist>
EOF

# Add Applications symlink
ln -s /Applications "${APP_DIR}/Applications"

echo "Creating DMG..."
rm -f "${DMG_NAME}"
hdiutil create -volname "${VOLUME_NAME}" \
    -srcfolder "${APP_DIR}" \
    -ov -format UDZO \
    "${DMG_NAME}"

echo "Done: ${DMG_NAME}"
