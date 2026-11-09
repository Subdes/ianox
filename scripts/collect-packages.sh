#!/usr/bin/env bash
# scripts/collect-packages.sh — Reúne todos los instaladores generados en dist-packages/
set -e

DEST_DIR="$(pwd)/dist-packages"
mkdir -p "$DEST_DIR"

echo "==> Copiando paquetes generados a $DEST_DIR..."

# 1. Linux
find src-tauri/target/release/bundle/deb/ -name "*.deb" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/release/bundle/appimage/ -name "*.AppImage" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/release/bundle/rpm/ -name "*.rpm" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true

# 2. Windows (.exe / .msi)
find src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/ -name "*.exe" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/x86_64-pc-windows-gnu/release/bundle/msi/ -name "*.msi" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true

# 3. macOS (.dmg / .app)
find src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/ -name "*.dmg" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true
find src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/ -name "*.dmg" -exec cp -f {} "$DEST_DIR/" \; 2>/dev/null || true

echo "==> Contenido final de $DEST_DIR:"
ls -lh "$DEST_DIR"

