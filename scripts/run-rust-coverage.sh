#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
mkdir -p target/coverage
cargo llvm-cov --locked --workspace --all-targets --json --output-path target/coverage/rust.json
