#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pnpm verify
./scripts/check-locked-dependencies.sh
./scripts/check-tauri-security.sh
cargo test --locked --test m1_browse_queries
cargo test --locked --test m1_text_preview
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
node scripts/check-scope-coverage.mjs
node scripts/check-m1-ipc-fixtures.mjs
