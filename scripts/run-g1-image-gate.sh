#!/usr/bin/env bash

set -euo pipefail

ROOT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_DIRECTORY="$ROOT_DIRECTORY/target/g1-image-gate"
cd "$ROOT_DIRECTORY"

printf 'G1: regenerating deterministic image fixtures\n'
swift scripts/generate-g1-image-fixtures.swift
git diff --exit-code -- tests/fixtures/images

printf 'G1: running image pipeline and restricted protocol tests\n'
cargo test --locked -p viewer-platform-macos image -- --nocapture
cargo test --locked -p viewer-desktop image_protocol -- --nocapture

printf 'G1: running optimized image benchmark\n'
cargo bench --locked -p viewer-platform-macos --bench image_pipeline

printf 'G1: comparing Quick Look and Image I/O pixels\n'
swift scripts/check-g1-image-consistency.swift \
  "$REPORT_DIRECTORY/consistency" \
  "$REPORT_DIRECTORY/consistency-report.json"

printf 'G1: auditing locked dependency graph\n'
cargo deny --offline --locked check

printf 'G1 image gate passed; reports are in %s\n' "$REPORT_DIRECTORY"
