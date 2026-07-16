#!/usr/bin/env bash
set -euo pipefail

required=(rustc cargo node pnpm clang xcrun)
for command_name in "${required[@]}"; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "[MISSING] $command_name"
    exit 1
  fi
  echo "[OK] $command_name"
done

xcrun --show-sdk-path >/dev/null
rustup target list --installed | grep -qx aarch64-apple-darwin
echo "[OK] macOS SDK"
echo "[OK] aarch64-apple-darwin target"
