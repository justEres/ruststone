#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"
PACKAGE="rs-client"
BIN_NAME="rs-client"
OUT_EXE="rustone.exe"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found in PATH" >&2
  exit 1
fi

if command -v rustup >/dev/null 2>&1; then
  if ! rustup target list --installed | grep -qx "$TARGET"; then
    echo "Installing Rust target: $TARGET"
    rustup target add "$TARGET"
  fi
else
  echo "warning: rustup not found; assuming target $TARGET is already installed"
fi

echo "Building $PACKAGE for $TARGET (release, feature: bundle_assets)..."
cargo build --release --target "$TARGET" -p "$PACKAGE" --features bundle_assets

SRC_EXE="target/$TARGET/release/$BIN_NAME.exe"
if [[ ! -f "$SRC_EXE" ]]; then
  echo "error: expected output not found: $SRC_EXE" >&2
  exit 1
fi

cp -f "$SRC_EXE" "$OUT_EXE"
echo "Wrote $(pwd)/$OUT_EXE"
