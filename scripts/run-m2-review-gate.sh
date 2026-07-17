#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

printf 'M2: verifying inherited M1, UI, workspace, dependency, security, and scope contracts\n'
pnpm gate:m1

printf 'M2: validating portable metadata policy and Apache-2.0 metadata\n'
node --test scripts/m2-portable-metadata.test.mjs
node scripts/validate-m2-portable-metadata.mjs --policy

printf 'M2: running portable metadata, projection, indexing, search, and browse contracts\n'
cargo test --locked --test m2_portable_metadata
cargo test --locked --test m2_session_projection
cargo test --locked --test m2_derived_indexing
cargo test --locked --test m2_search_queries
cargo test --locked --test m2_browse_projections

printf 'M2: running desktop integration and security boundaries\n'
cargo test --locked -p viewer-desktop --test m2_desktop_runtime
cargo test --locked -p viewer-desktop --test security_boundaries

printf 'M2: rerunning the G3 indexed-search performance and dependency gate\n'
./scripts/run-g3-scan-search-gate.sh

printf 'M2 review-efficiency gate passed\n'
