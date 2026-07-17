#!/usr/bin/env bash

set -euo pipefail

ROOT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_TEMP_ROOT="${TMPDIR:-/tmp}"
GATE_TEMP_DIRECTORY="$(mktemp -d "${LOCAL_TEMP_ROOT%/}/viewer-g2-gate.XXXXXX")"

cleanup() {
  rm -rf -- "$GATE_TEMP_DIRECTORY"
}
trap cleanup EXIT

cd "$ROOT_DIRECTORY"
export TMPDIR="$GATE_TEMP_DIRECTORY"

printf 'G2: running file-operation unit tests in %s\n' "$GATE_TEMP_DIRECTORY"
cargo test --locked --workspace --lib -- --skip real_trash_smoke_test_requires_explicit_opt_in

printf 'G2: running real-filesystem transaction and fake-Trash contracts\n'
cargo test --locked -p viewer-infrastructure --test file_transactions -- --nocapture

printf 'G2: rerunning the deterministic recovery matrix and unsafe-evidence cases\n'
cargo test --locked -p viewer-infrastructure --test file_transactions recovery_ -- --nocapture

if [[ "${VIEWER_ALLOW_REAL_TRASH_TEST:-0}" == "1" ]]; then
  printf 'G2: running explicitly authorized real macOS Trash smoke test\n'
  cargo test --locked -p viewer-platform-macos \
    files::trash::tests::real_trash_smoke_test_requires_explicit_opt_in -- --exact --nocapture
else
  printf 'G2: skipping real macOS Trash smoke test (set VIEWER_ALLOW_REAL_TRASH_TEST=1 manually)\n'
fi

printf 'G2: checking formatting, lints and locked dependencies\n'
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny --offline --locked check

LEFTOVER_TEMPORARY="$(find "$GATE_TEMP_DIRECTORY" -type f \
  \( -name '.viewer-copy-*.part' -o -name '.viewer-rename-*.part' -o -name '.viewer-replace-*.part' \) \
  -print -quit)"
if [[ -n "$LEFTOVER_TEMPORARY" ]]; then
  printf 'G2 gate failed: unregistered temporary remains at %s\n' "$LEFTOVER_TEMPORARY" >&2
  exit 1
fi

printf 'G2 file transaction gate passed\n'
