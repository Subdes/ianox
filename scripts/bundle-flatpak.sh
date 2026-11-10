#!/usr/bin/env bash
# scripts/bundle-flatpak.sh — Empaqueta el bundle .flatpak
set -e

VERSION=$(node -p "require('./package.json').version")
MANIFEST="packaging/flatpak/io.github.subdes.ianox.yml"
REPO_DIR="src-tauri/target/flatpak-repo"
BUILD_DIR="src-tauri/target/flatpak-build"
OUT_BUNDLE="dist-packages/ianox_${VERSION}_x86_64.flatpak"

mkdir -p dist-packages

echo "==> Compilando Flatpak con flatpak-builder..."
flatpak-builder --user --force-clean --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"

echo "==> Generando bundle único .flatpak..."
flatpak build-bundle "$REPO_DIR" "$OUT_BUNDLE" io.github.subdes.ianox

echo "==> Creado exitosamente: $OUT_BUNDLE"

