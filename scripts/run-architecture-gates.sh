#!/usr/bin/env bash

set -euo pipefail

ROOT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIRECTORY"

required_evidence=(
  docs/adr/0001-macos-image-pipeline.md
  docs/adr/0002-file-transaction-protocol.md
  docs/adr/0003-scan-search-and-generation.md
)

for path in "${required_evidence[@]}"; do
  if [[ ! -s "$path" ]]; then
    printf 'missing gate evidence: %s\n' "$path" >&2
    exit 1
  fi
done

./scripts/run-g1-image-gate.sh
./scripts/run-g2-file-transaction-gate.sh
./scripts/run-g3-scan-search-gate.sh
./scripts/check-tauri-security.sh
node scripts/check-scope-coverage.mjs
pnpm verify
cargo deny --offline --locked check

printf 'Viewer architecture gates passed\n'
