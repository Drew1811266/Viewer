#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

before_cargo=$(shasum -a 256 Cargo.lock)
before_pnpm=$(shasum -a 256 pnpm-lock.yaml)

cargo metadata --locked --format-version 1 >/dev/null
pnpm install --frozen-lockfile
cargo deny check advisories bans licenses sources
pnpm audit --audit-level high

after_cargo=$(shasum -a 256 Cargo.lock)
after_pnpm=$(shasum -a 256 pnpm-lock.yaml)
test "$before_cargo" = "$after_cargo" || {
  echo "Cargo.lock changed during locked dependency verification" >&2
  exit 1
}
test "$before_pnpm" = "$after_pnpm" || {
  echo "pnpm-lock.yaml changed during locked dependency verification" >&2
  exit 1
}

echo "locked dependencies verified"
