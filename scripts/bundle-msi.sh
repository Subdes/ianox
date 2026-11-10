#!/usr/bin/env bash
# scripts/bundle-msi.sh — Compila ianox.wxs a .msi usando wixl (msitools)
set -e

WXS_SOURCE="packaging/windows/ianox.wxs"
OUT_MSI="dist-packages/ianox_0.0.3_x64.msi"
EXE_BIN="src-tauri/target/x86_64-pc-windows-gnu/release/ianox.exe"

if [ ! -f "$EXE_BIN" ]; then
    echo "==> Error: No se encuentra $EXE_BIN. Compila Windows primero."
    exit 1
fi

mkdir -p dist-packages

echo "==> Generando instalador .msi con wixl..."
wixl -v -a x64 -o "$OUT_MSI" "$WXS_SOURCE"

echo "==> Creado exitosamente: $OUT_MSI"

