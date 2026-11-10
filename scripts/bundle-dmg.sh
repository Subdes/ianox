#!/usr/bin/env bash
# scripts/bundle-dmg.sh — Empaqueta ianox.app en .dmg y .app.tar.gz para macOS desde Linux
set -e

APP_NAME="ianox"
VERSION=$(node -p "require('./package.json').version")
mkdir -p dist-packages

bundle_arch() {
    local TARGET_TRIPLE="$1"
    local ARCH_NAME="$2"
    local BIN_SOURCE="src-tauri/target/${TARGET_TRIPLE}/release/ianox"
    local STAGE_DIR="src-tauri/target/${TARGET_TRIPLE}/release/bundle/osx"
    local APP_DIR="$STAGE_DIR/${APP_NAME}.app"
    local DMG_OUT="dist-packages/${APP_NAME}_${VERSION}_${ARCH_NAME}.dmg"
    local TAR_OUT="dist-packages/${APP_NAME}_${VERSION}_${ARCH_NAME}.app.tar.gz"

    if [ ! -f "$BIN_SOURCE" ]; then
        return 0
    fi

    echo "==> Empaquetando macOS ($ARCH_NAME)..."
    rm -rf "$STAGE_DIR"
    mkdir -p "$APP_DIR/Contents/MacOS"
    mkdir -p "$APP_DIR/Contents/Resources"

    # Copiar binario nativo e icono
    cp -f "$BIN_SOURCE" "$APP_DIR/Contents/MacOS/${APP_NAME}"
    chmod +x "$APP_DIR/Contents/MacOS/${APP_NAME}"
    cp -f "src-tauri/icons/icon.icns" "$APP_DIR/Contents/Resources/icon.icns"

    # Generar Info.plist
    cat << EOF > "$APP_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.subdes.ianox</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

    # Método 1: Bundle portable .app.tar.gz
    echo "==> Método 1: Generando bundle portable $TAR_OUT..."
    tar -czf "$TAR_OUT" -C "$STAGE_DIR" "${APP_NAME}.app"

    # Método 2: Imagen .dmg con branding
    echo "==> Método 2: Generando imagen .dmg con genisoimage ($DMG_OUT)..."
    mkdir -p "$STAGE_DIR/dmg-root"
    cp -rf "$APP_DIR" "$STAGE_DIR/dmg-root/"
    ln -sf /Applications "$STAGE_DIR/dmg-root/Applications"
    mkdir -p "$STAGE_DIR/dmg-root/.background"
    cp -f "packaging/macos/assets/dmg-background.png" "$STAGE_DIR/dmg-root/.background/background.png"

    genisoimage -V "${APP_NAME}" \
                -D -R -apple \
                -no-pad \
                -o "$DMG_OUT" \
                "$STAGE_DIR/dmg-root" 2>/dev/null || \
    genisoimage -V "${APP_NAME}" \
                -r -J \
                -o "$DMG_OUT" \
                "$STAGE_DIR/dmg-root"

    echo "==> macOS ($ARCH_NAME) completado exitosamente: $DMG_OUT y $TAR_OUT"
}

# 1. Intel x86_64
bundle_arch "x86_64-apple-darwin" "x64"

# 2. Apple Silicon aarch64
bundle_arch "aarch64-apple-darwin" "aarch64"
