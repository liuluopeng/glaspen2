#!/bin/bash
set -e

APP_NAME="glaspen2"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version *= *"\(.*\)"/\1/')
ARCH=$(uname -m)
DMG_NAME="${APP_NAME}-${VERSION}-${ARCH}.dmg"
DMG_PATH="release_history/${DMG_NAME}"
VOLUME_NAME="Glaspen2"
BUILD_DIR="target/release"
APP_DIR="/tmp/${APP_NAME}-dmg"
APP_BUNDLE="${APP_DIR}/${APP_NAME}.app"
FW_DIR="${APP_BUNDLE}/Contents/Frameworks"
BIN="${APP_BUNDLE}/Contents/MacOS/${APP_NAME}"

echo "=== Building Flutter frameworks ==="
cd flutter_settings && fvm flutter build macos-framework --release && cd ..

echo "=== Building release ==="
cargo build --release

echo "=== Creating app bundle ==="
rm -rf "${APP_DIR}"
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"
mkdir -p "${FW_DIR}"

cp "${BUILD_DIR}/${APP_NAME}" "${BIN}"
cp "glaspen2.icns" "${APP_BUNDLE}/Contents/Resources/"

# --- Copy Flutter frameworks ---
FLUTTER_FW="flutter_settings/build/macos/framework/Release"
cp -R "${FLUTTER_FW}/FlutterMacOS.xcframework/macos-arm64_x86_64/FlutterMacOS.framework" "${FW_DIR}/"
cp -R "${FLUTTER_FW}/App.xcframework/macos-arm64_x86_64/App.framework" "${FW_DIR}/"

# Fix Flutter rpath
install_name_tool -delete_rpath "${PWD}/${FLUTTER_FW}/FlutterMacOS.xcframework/macos-arm64_x86_64" "${BIN}" 2>/dev/null || true
install_name_tool -delete_rpath "${PWD}/${FLUTTER_FW}/App.xcframework/macos-arm64_x86_64" "${BIN}" 2>/dev/null || true
install_name_tool -add_rpath "@executable_path/../Frameworks" "${BIN}" 2>/dev/null || true

# --- Bundle Homebrew dylibs ---
echo "=== Collecting Homebrew dylib dependencies ==="

get_homebrew_deps() {
    otool -L "$1" 2>/dev/null | tail -n +2 | awk '{print $1}' | grep -E "^/opt/homebrew|^/usr/local" | sort -u
}

# BFS: collect all deps transitively
TMPDIR=$(mktemp -d)
ALL_DEPS="${TMPDIR}/all.txt"
QUEUE="${TMPDIR}/queue.txt"
VISITED="${TMPDIR}/visited.txt"

get_homebrew_deps "$BIN" > "$QUEUE"

while [ -s "$QUEUE" ]; do
    # Process current queue
    while IFS= read -r lib; do
        grep -qxF "$lib" "$VISITED" 2>/dev/null && continue
        echo "$lib" >> "$VISITED"
        echo "$lib" >> "$ALL_DEPS"
        get_homebrew_deps "$lib" >> "${TMPDIR}/new.txt"
    done < "$QUEUE"
    # Prepare next queue
    if [ -f "${TMPDIR}/new.txt" ]; then
        cat "${TMPDIR}/new.txt" | sort -u > "$QUEUE"
        rm -f "${TMPDIR}/new.txt"
    else
        break
    fi
done

DEPS=$(cat "$ALL_DEPS" | sort -u)
rm -rf "$TMPDIR"

if [ -z "$DEPS" ]; then
    echo "No Homebrew dependencies found."
else
    echo "Found dependencies:"
    echo "$DEPS"

    # Copy all dylibs to Frameworks
    echo "=== Copying dylibs ==="
    while IFS= read -r lib; do
        [ -z "$lib" ] && continue
        base=$(basename "$lib")
        if [ ! -f "${FW_DIR}/${base}" ]; then
            echo "  ${base}"
            cp "$lib" "${FW_DIR}/${base}"
            chmod 644 "${FW_DIR}/${base}"
        fi
    done <<< "$DEPS"

    # Fix install names in main binary
    echo "=== Fixing main binary references ==="
    while IFS= read -r lib; do
        [ -z "$lib" ] && continue
        base=$(basename "$lib")
        install_name_tool -change "$lib" "@executable_path/../Frameworks/${base}" "${BIN}" 2>/dev/null || true
    done <<< "$DEPS"

    # Fix self-references and cross-references in all bundled dylibs
    echo "=== Fixing dylib references ==="

    for dylib in "${FW_DIR}"/*.dylib; do
        [ ! -f "$dylib" ] && continue
        # Fix self-reference (id)
        install_name_tool -id "@executable_path/../Frameworks/$(basename "$dylib")" "$dylib" 2>/dev/null || true
        # Fix references to other bundled dylibs
        while IFS= read -r lib; do
            [ -z "$lib" ] && continue
            base=$(basename "$lib")
            if [ -f "${FW_DIR}/${base}" ]; then
                install_name_tool -change "$lib" "@executable_path/../Frameworks/${base}" "$dylib" 2>/dev/null || true
            fi
        done <<< $(otool -L "$dylib" 2>/dev/null | tail -n +2 | awk '{print $1}' | grep -E "^/opt/homebrew|^/usr/local")
    done
fi

# --- Re-sign everything (install_name_tool invalidates signatures) ---
echo "=== Code signing ==="
# Sign all dylibs first
for f in "${FW_DIR}"/*.dylib; do
    [ -f "$f" ] && codesign --force --sign - "$f" 2>/dev/null
done
# Sign frameworks
codesign --force --sign - "${FW_DIR}/FlutterMacOS.framework" 2>/dev/null
codesign --force --sign - "${FW_DIR}/App.framework" 2>/dev/null
# Sign main binary last
codesign --force --sign - "${BIN}"

# --- Info.plist ---
cat > "${APP_BUNDLE}/Contents/Info.plist" << EOF
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

ln -s /Applications "${APP_DIR}/Applications"

echo "=== Creating DMG ==="
mkdir -p release_history
rm -f "${DMG_PATH}"
hdiutil create -volname "${VOLUME_NAME}" \
    -srcfolder "${APP_DIR}" \
    -ov -format UDZO \
    "${DMG_PATH}"

echo ""
echo "Done: ${DMG_PATH}"
echo ""
echo "Bundled frameworks:"
ls "${FW_DIR}/"
echo ""
echo "DMG size: $(du -h "${DMG_PATH}" | cut -f1)"
