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

# macOS 27 的 lipo 拒绝 flutter 3.41 使用的双架构 -verify_arch 调用形式
# ("requires exactly one input file"),shim 把它拆成逐架构校验
# (见 scripts/lipo-shim/lipo)。必须先入 PATH,后续所有 lipo 调用都走 shim。
export PATH="$PWD/scripts/lipo-shim:$PATH"

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

# --- Verify flutter_rust_bridge entry points ---
# 设置面板靠 DynamicLibrary.process()(dlopen(NULL) + dlsym)在运行时解析这些
# 符号;Rust 以 rlib 静态链入,靠 build.rs 的 `-Wl,-u,_<symbol>` 逐个保留。
# 一旦丢失,构建与签名全都正常,只有用户打开设置面板时才会失败 —— 所以这里
# 直接断言最终二进制,让问题在打包阶段暴露。
#
# 基准取同一次构建产出的 cdylib:它是这个 crate 的完整符号集,而 Dart 能
# lookup 的 frb_* 名字与它一一对应。升级 FRB 后新增入口也能自动覆盖,
# 不用在这里再维护一份列表。
echo "=== Verifying flutter_rust_bridge entry points ==="
if command -v dyld_info >/dev/null 2>&1; then
    SYMBOLS=$(dyld_info -exports "${BIN}")
    DYLIB="${BUILD_DIR}/lib${APP_NAME}.dylib"
    if [ -f "${DYLIB}" ]; then
        REQUIRED=$(dyld_info -exports "${DYLIB}" | awk '{print $2}' | grep '^_frb' | sort -u)
    else
        echo "  WARNING: 找不到 ${DYLIB},退化为只检查 init 必需项" >&2
        REQUIRED=$(printf '_frb_pde_ffi_dispatcher_primary\n_frb_pde_ffi_dispatcher_sync\n_frb_get_rust_content_hash\n_frb_init_frb_dart_api_dl\n')
    fi
    missing=""
    for sym in ${REQUIRED}; do
        echo "${SYMBOLS}" | grep -qE "[[:space:]]${sym}\$" || missing="${missing} ${sym}"
    done
    if [ -n "${missing}" ]; then
        echo "ERROR: 这些 flutter_rust_bridge 符号不在 ${BIN} 的导出表里:" >&2
        for sym in ${missing}; do
            echo "         ${sym}  → 检查 build.rs 的 FRB_SYMBOLS / -Wl,-u,${sym}" >&2
        done
        echo "       Dart 侧会解析不到符号,设置面板将无法与 Rust 通信。" >&2
        exit 1
    fi
    echo "  ok ($(echo "${REQUIRED}" | wc -l | tr -d ' ') 个符号)"
else
    echo "  WARNING: 没有 dyld_info,跳过符号校验" >&2
fi

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

# --- Re-sign everything (install_name_tool invalidates signatures) ---
# Sign with the stable self-signed certificate so the macOS Accessibility
# permission (TCC) survives app updates. Ad-hoc signatures are keyed on the
# build's cdhash and force the user to re-grant the permission every time.
echo "=== Code signing ==="
SIGN_IDENTITY="Glaspen2 Development"
# Unlock the dedicated signing keychain (known password) so codesign never
# prompts; its partition list already allows codesign access.
SIGN_KEYCHAIN="${HOME}/Library/Keychains/glaspen2-signing.keychain-db"
if [ -f "${SIGN_KEYCHAIN}" ]; then
    security unlock-keychain -p glaspen2 "${SIGN_KEYCHAIN}" 2>/dev/null || true
fi
if security find-identity -p codesigning 2>/dev/null | grep -q "${SIGN_IDENTITY}"; then
    echo "  using identity: ${SIGN_IDENTITY}"
    IDENTITY_ARGS=(--sign "${SIGN_IDENTITY}")
else
    echo "  WARNING: '${SIGN_IDENTITY}' not found in keychain — falling back to ad-hoc."
    echo "  Run scripts/create-signing-cert.sh once; ad-hoc signatures require"
    echo "  re-adding the Accessibility permission after every update."
    IDENTITY_ARGS=(--sign -)
fi
# Sign all dylibs first
for f in "${FW_DIR}"/*.dylib; do
    [ -f "$f" ] && codesign --force "${IDENTITY_ARGS[@]}" "$f" 2>/dev/null
done
# Sign frameworks
codesign --force "${IDENTITY_ARGS[@]}" "${FW_DIR}/FlutterMacOS.framework" 2>/dev/null
codesign --force "${IDENTITY_ARGS[@]}" "${FW_DIR}/App.framework" 2>/dev/null
# Sign the whole bundle last (seals executable + Info.plist together)
codesign --force "${IDENTITY_ARGS[@]}" "${APP_BUNDLE}"

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
