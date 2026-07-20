#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

live_parent="$(mktemp -d "${TMPDIR:-/tmp}/viewer-m3-live.XXXXXX")"
live_project="$live_parent/project"
cleanup() {
  rm -rf "$live_parent"
}
trap cleanup EXIT
mkdir "$live_project"

printf 'M3: verifying the complete inherited M2 contract\n'
pnpm gate:m2

printf 'M3: verifying UI tests, production build, formatting, lint, and workspace tests\n'
pnpm --dir ui test
pnpm --dir ui build
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace

printf 'M3: verifying every organization, drag, compare, watcher, and lifecycle suite\n'
cargo test --locked -p viewer-application --test m3_command_service
cargo test --locked -p viewer-infrastructure --test m3_file_commands
cargo test --locked -p viewer-infrastructure --test m3_operation_journal
cargo test --locked -p viewer-infrastructure --test m3_operation_projections
cargo test --locked -p viewer-infrastructure --test m3_rename_preflight
cargo test --locked -p viewer-infrastructure --test m3_undo
cargo test --locked -p viewer-infrastructure --test m3_watcher_runtime
cargo test --locked -p viewer-desktop --test m3_desktop_runtime
cargo test --locked -p viewer-desktop --test m3_drag_export
cargo test --locked -p viewer-desktop --test m3_compare_budget
cargo test --locked -p viewer-desktop --test m3_readonly_lifecycle

printf 'M3: rerunning G2 crash recovery and G3 watcher/search performance gates\n'
./scripts/run-g2-file-transaction-gate.sh
./scripts/run-g3-scan-search-gate.sh

printf 'M3: enforcing repository, dependency, license, audit, security, and scope policy\n'
node --test scripts/repository-policy.test.mjs
./scripts/check-locked-dependencies.sh
./scripts/check-tauri-security.sh
node scripts/check-scope-coverage.mjs

printf 'M3: generating and validating a fresh schema-v3 portable project\n'
VIEWER_M3_LIVE_FIXTURE="$live_project" node --test scripts/m3-portable-metadata.test.mjs
node scripts/validate-m2-portable-metadata.mjs "$live_project"

printf 'M3 organization and comparison gate passed\n'
