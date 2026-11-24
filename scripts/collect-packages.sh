#!/usr/bin/env bash
# scripts/collect-packages.sh — Reúne todos los instaladores generados en dist-packages/
set -e

DEST_DIR="$(pwd)/dist-packages"
VERSION=$(node -p "require('./package.json').version")
mkdir -p "$DEST_DIR"

echo "==> Limpiando versiones viejas en $DEST_DIR (conservando solo $VERSION)..."
find "$DEST_DIR" -type f ! -name "*${VERSION}*" -delete 2>/dev/null || true

echo "==> Copiando paquetes generados para versión $VERSION a $DEST_DIR..."

# 1. Linux
find src-tauri/target/release/bundle/deb/ -name "*${VERSION}*.deb" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/release/bundle/appimage/ -name "*${VERSION}*.AppImage" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/release/bundle/rpm/ -name "*${VERSION}*.rpm" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true

# 2. Windows (.exe / .msi)
find src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/ -name "*${VERSION}*.exe" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/x86_64-pc-windows-gnu/release/bundle/msi/ -name "*${VERSION}*.msi" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true

# 3. macOS (.dmg / .app)
find src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/ -name "*${VERSION}*.dmg" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/ -name "*${VERSION}*.dmg" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true

echo "==> Contenido final de $DEST_DIR (versión $VERSION):"
ls -lh "$DEST_DIR"

