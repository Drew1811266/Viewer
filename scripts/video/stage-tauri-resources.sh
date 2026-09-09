#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd -P)
repo_root=$(cd "$script_dir/../.." && pwd -P)

case "$(uname -m)" in
  arm64) host_target=aarch64-apple-darwin ;;
  x86_64) host_target=x86_64-apple-darwin ;;
  *) printf 'unsupported macOS architecture\n' >&2; exit 1 ;;
esac

if [[ -n "${1:-}" ]]; then
  source_dir=$1
elif [[ -n "${VIEWER_VIDEO_SOURCE_DIR:-}" ]]; then
  source_dir=$VIEWER_VIDEO_SOURCE_DIR
elif [[ -d "$repo_root/target/viewer-video-runtime/$host_target/ViewerVideoRuntime" ]]; then
  source_dir="$repo_root/target/viewer-video-runtime/$host_target/ViewerVideoRuntime"
else
  # Local development may only have the reviewed universal runtime. Release
  # CI still supplies the architecture-specific source explicitly.
  source_dir="$repo_root/target/viewer-video-runtime/universal-apple-darwin/ViewerVideoRuntime"
fi
target_dir=${2:-${VIEWER_VIDEO_TAURI_RESOURCE_DIR:-$repo_root/target/viewer-video-runtime/universal-apple-darwin/ViewerVideoRuntime}}
runtime_verifier=${VIEWER_VIDEO_RUNTIME_VERIFIER:-$script_dir/verify-runtime.sh}

if [[ ! -d "$source_dir" ]]; then
  printf 'video runtime source does not exist: %s\n' "$source_dir" >&2
  exit 1
fi
source_dir=$(cd "$source_dir" && pwd -P)

# Verification intentionally precedes every target inspection or mutation.
VIEWER_VIDEO_STAGE_DIR="$source_dir" "$runtime_verifier"

expected_files=$(mktemp)
trap 'rm -f "$expected_files"' EXIT
{
  printf '%s\n' runtime.inventory.sha256 runtime.lock.json
  sed -E 's/^[[:xdigit:]]{64}  //' "$source_dir/runtime.inventory.sha256"
} | LC_ALL=C sort -u > "$expected_files"

validate_exact_files() {
  local directory=$1 label=$2
  while IFS= read -r entry; do
    relative=${entry#"$directory"/}
    if [[ -L "$entry" || ! -f "$entry" ]]; then
      printf 'unexpected %s entry: %s\n' "$label" "$relative" >&2
      exit 1
    fi
    if ! grep -Fqx "$relative" "$expected_files"; then
      printf 'unexpected %s file: %s\n' "$label" "$relative" >&2
      exit 1
    fi
  done < <(find "$directory" -mindepth 1 ! -type d -print | LC_ALL=C sort)

  while IFS= read -r relative; do
    if [[ ! -f "$directory/$relative" || -L "$directory/$relative" ]]; then
      printf 'missing %s file: %s\n' "$label" "$relative" >&2
      exit 1
    fi
  done < "$expected_files"
}

validate_exact_files "$source_dir" source

mkdir -p "$target_dir"
target_dir=$(cd "$target_dir" && pwd -P)
if [[ "$source_dir" == "$target_dir" ]]; then
  printf 'Viewer video runtime already staged: %s\n' "$target_dir"
  exit 0
fi

while IFS= read -r target_entry; do
  relative=${target_entry#"$target_dir"/}
  if [[ -L "$target_entry" ]]; then
    printf 'target symlink is prohibited: %s\n' "$relative" >&2
    exit 1
  fi
  if ! grep -Fqx "$relative" "$expected_files"; then
    printf 'unexpected target entry: %s\n' "$relative" >&2
    exit 1
  fi
done < <(find "$target_dir" -mindepth 1 ! -type d -print | LC_ALL=C sort)

while IFS= read -r target_directory; do
  relative=${target_directory#"$target_dir"/}
  if ! grep -Fq "$relative/" "$expected_files"; then
    printf 'unexpected target directory: %s\n' "$relative" >&2
    exit 1
  fi
done < <(find "$target_dir" -mindepth 1 -type d -print | LC_ALL=C sort)

ditto "$source_dir" "$target_dir"
validate_exact_files "$target_dir" target

while IFS= read -r executable; do
  relative=${executable#"$source_dir"/}
  if [[ ! -x "$target_dir/$relative" ]]; then
    printf 'staging did not preserve executable bit: %s\n' "$relative" >&2
    exit 1
  fi
done < <(find "$source_dir" -type f -perm -111 -print)

signing_identity=${VIEWER_VIDEO_SIGNING_IDENTITY:-}
if [[ -n "$signing_identity" ]]; then
  codesign_bin=${VIEWER_VIDEO_CODESIGN_BIN:-codesign}
  if ! command -v "$codesign_bin" >/dev/null 2>&1 && [[ ! -x "$codesign_bin" ]]; then
    printf 'configured video runtime signing tool does not exist: %s\n' "$codesign_bin" >&2
    exit 1
  fi
  if [[ "$signing_identity" != Developer\ ID\ Application:* ]]; then
    printf 'VIEWER_VIDEO_SIGNING_IDENTITY must be a Developer ID Application identity\n' >&2
    exit 1
  fi
  while IFS= read -r -d '' nested; do
    "$codesign_bin" --force --options runtime --timestamp --sign "$signing_identity" "$nested"
  done < <(find "$target_dir" -type f \( -name '*.dylib' -o -perm -111 \) -print0)
else
  printf 'Viewer video runtime staged without release signing (development mode)\n'
fi

(
  cd "$target_dir"
  find bin lib licenses -type f -print | LC_ALL=C sort | while IFS= read -r path; do
    shasum -a 256 "$path"
  done > runtime.inventory.sha256
)

VIEWER_VIDEO_STAGE_DIR="$target_dir" "$runtime_verifier"
printf 'Viewer video runtime staged for Tauri: %s\n' "$target_dir"
