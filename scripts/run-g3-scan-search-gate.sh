#!/usr/bin/env bash

set -euo pipefail

ROOT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_TEMP_ROOT="${TMPDIR:-/tmp}"
GATE_TEMP_DIRECTORY="$(mktemp -d "${LOCAL_TEMP_ROOT%/}/viewer-g3-gate.XXXXXX")"
CORPUS_DIRECTORY="$GATE_TEMP_DIRECTORY/viewer-g3-corpus"
REPORT_DIRECTORY="$ROOT_DIRECTORY/target/g3-scan-search-gate"

cleanup() {
  rm -rf -- "$GATE_TEMP_DIRECTORY"
}
trap cleanup EXIT

cd "$ROOT_DIRECTORY"
printf 'G3: generating deterministic scan/search corpus\n'
./scripts/generate-g3-project.sh "$CORPUS_DIRECTORY"
PORTABLE_BEFORE="$(shasum -a 256 "$CORPUS_DIRECTORY/.viewer/metadata.sqlite")"

printf 'G3: running scan, search, generation and reconciliation contracts\n'
cargo test --locked -p viewer-application scheduler -- --nocapture
cargo test --locked -p viewer-infrastructure --test progressive_scan -- --nocapture
cargo test --locked -p viewer-infrastructure --test search -- --nocapture
cargo test --locked -p viewer-platform-macos --test watcher_reconcile -- --nocapture

printf 'G3: running 20 fresh-session optimized measurements\n'
rm -rf -- "$REPORT_DIRECTORY"
cargo run --locked --release -p viewer-infrastructure --example g3_gate -- \
  "$CORPUS_DIRECTORY" "$REPORT_DIRECTORY/benchmark-report.json" 20

PORTABLE_AFTER="$(shasum -a 256 "$CORPUS_DIRECTORY/.viewer/metadata.sqlite")"
if [[ "$PORTABLE_BEFORE" != "$PORTABLE_AFTER" ]]; then
  printf 'G3 gate failed: portable metadata changed during session-index rebuilds\n' >&2
  exit 1
fi
node -e 'const report=require(process.argv[1]); if (!report.passed) process.exit(1)' \
  "$REPORT_DIRECTORY/benchmark-report.json"

printf 'G3: checking formatting, lints and dependency policy\n'
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny --offline --locked check

printf 'G3 scan/search gate passed; report is in %s\n' "$REPORT_DIRECTORY"
