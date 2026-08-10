#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd -P)
repo_root=$(cd "$script_dir/../.." && pwd -P)
lock_file="$script_dir/runtime.lock.json"
: "${VIEWER_VIDEO_STAGE_DIR:?set VIEWER_VIDEO_STAGE_DIR to the staged ViewerVideoRuntime directory}"

if [[ ! -d "$VIEWER_VIDEO_STAGE_DIR" ]]; then
  printf 'video runtime stage does not exist\n' >&2
  exit 1
fi
stage_dir=$(cd "$VIEWER_VIDEO_STAGE_DIR" && pwd -P)

node "$script_dir/runtime-lock.mjs" validate "$lock_file"

require_file() {
  local path=$1
  local label=$2
  if [[ ! -f "$path" ]]; then
    printf 'missing %s\n' "$label" >&2
    exit 1
  fi
}

require_executable() {
  local path=$1
  local label=$2
  require_file "$path" "$label"
  if [[ ! -x "$path" ]]; then
    printf '%s is not executable\n' "$label" >&2
    exit 1
  fi
}

require_file "$stage_dir/lib/libmpv.2.dylib" 'libmpv.2.dylib'
require_executable "$stage_dir/bin/ffmpeg" 'ffmpeg'
require_executable "$stage_dir/bin/ffprobe" 'ffprobe'
require_file "$stage_dir/runtime.lock.json" 'runtime.lock.json'
cmp -s "$lock_file" "$stage_dir/runtime.lock.json" || {
  printf 'staged runtime lock differs from the reviewed lock\n' >&2
  exit 1
}

while IFS=$'\t' read -r name _version _url _sha256; do
  require_file "$stage_dir/licenses/$name.txt" "$name license"
done < <(node "$script_dir/runtime-lock.mjs" list "$lock_file")

build_configuration=$($stage_dir/bin/ffmpeg -hide_banner -buildconf 2>&1)
for option in --disable-gpl --disable-nonfree --disable-network --disable-ffplay; do
  if [[ "$build_configuration" != *"$option"* ]]; then
    printf 'ffmpeg build is missing %s\n' "$option" >&2
    exit 1
  fi
done
for option in --enable-gpl --enable-nonfree --enable-network; do
  if [[ "$build_configuration" == *"$option"* ]]; then
    printf 'ffmpeg build contains prohibited option %s\n' "$option" >&2
    exit 1
  fi
done

linkage_file=$(mktemp)
for binary in "$stage_dir/lib/libmpv.2.dylib" "$stage_dir/bin/ffmpeg" "$stage_dir/bin/ffprobe"; do
  otool -L "$binary" >> "$linkage_file"
done
node "$script_dir/validate-linkage.mjs" "$linkage_file"
if ! rg -q '^[[:space:]]+/usr/lib/libiconv\.2\.dylib ' "$linkage_file"; then
  printf 'libmpv is missing the approved macOS system iconv linkage\n' >&2
  exit 1
fi

rpaths_file=$(mktemp)
otool -l "$stage_dir/lib/libmpv.2.dylib" > "$rpaths_file"
node "$script_dir/validate-rpaths.mjs" "$rpaths_file"
if ! rg -q '^[[:space:]]+path /usr/lib/swift \(offset [0-9]+\)$' "$rpaths_file"; then
  printf 'libmpv is missing the approved macOS system Swift runtime path\n' >&2
  exit 1
fi

if [[ -f "$stage_dir/runtime.inventory.sha256" ]]; then
  (cd "$stage_dir" && shasum -a 256 -c runtime.inventory.sha256)
fi

printf 'Viewer video runtime verified: %s\n' "$stage_dir"
