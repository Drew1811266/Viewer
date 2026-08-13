#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s [--mode development|signed] [--attestation file --artifact file] Viewer.app\n' "$0" >&2
  exit 64
}

mode=signed
attestation=
artifact=
while [[ ${1:-} == --* ]]; do
  case $1 in
    --mode) [[ $# -ge 2 ]] || usage; mode=$2; shift 2 ;;
    --attestation) [[ $# -ge 2 ]] || usage; attestation=$2; shift 2 ;;
    --artifact) [[ $# -ge 2 ]] || usage; artifact=$2; shift 2 ;;
    *) usage ;;
  esac
done
[[ $# == 1 ]] || usage
app=$1
[[ $mode == development || $mode == signed ]] || usage
[[ -z $attestation && -z $artifact || -n $attestation && -n $artifact ]] || usage

script_dir=$(cd "$(dirname "$0")" && pwd -P)
if [[ ! -d "$app" ]]; then
  printf 'application bundle does not exist: %s\n' "$app" >&2
  exit 1
fi
app=$(cd "$app" && pwd -P)
runtime="$app/Contents/Resources/ViewerVideoRuntime"
[[ -d $runtime ]] || { printf 'missing bundled video runtime\n' >&2; exit 1; }
app_executable="$app/Contents/MacOS/viewer-desktop"
[[ -x $app_executable && ! -L $app_executable ]] || {
  printf 'missing Viewer application executable\n' >&2
  exit 1
}

runtime_verifier=${VIEWER_VIDEO_RUNTIME_VERIFIER:-$script_dir/verify-runtime.sh}
VIEWER_VIDEO_STAGE_DIR="$runtime" "$runtime_verifier"
inventory="$runtime/runtime.inventory.sha256"
[[ -f $inventory && ! -L $inventory ]] || { printf 'missing runtime inventory\n' >&2; exit 1; }
[[ -f $runtime/runtime.lock.json && ! -L $runtime/runtime.lock.json ]] || {
  printf 'missing runtime lock\n' >&2
  exit 1
}
if find "$runtime" -mindepth 1 -type l -print -quit | grep -q .; then
  printf 'runtime symlink is prohibited\n' >&2
  exit 1
fi

expected_files=$(mktemp)
trap 'rm -f "$expected_files"' EXIT
{
  printf '%s\n' runtime.inventory.sha256 runtime.lock.json
  while IFS= read -r line; do
    [[ $line =~ ^[[:xdigit:]]{64}\ \ [^/].*$ ]] || {
      printf 'invalid runtime inventory entry\n' >&2
      exit 1
    }
    relative=${line:66}
    [[ $relative != *'/../'* && $relative != ../* && $relative != */.. ]] || {
      printf 'runtime inventory path escapes its root: %s\n' "$relative" >&2
      exit 1
    }
    printf '%s\n' "$relative"
  done < "$inventory"
} | LC_ALL=C sort -u > "$expected_files"
while IFS= read -r entry; do
  relative=${entry#"$runtime"/}
  [[ -f $entry && ! -L $entry ]] || { printf 'unexpected runtime entry: %s\n' "$relative" >&2; exit 1; }
  grep -Fqx "$relative" "$expected_files" || { printf 'unexpected runtime file: %s\n' "$relative" >&2; exit 1; }
done < <(find "$runtime" -mindepth 1 ! -type d -print | LC_ALL=C sort)
while IFS= read -r relative; do
  [[ -f $runtime/$relative && ! -L $runtime/$relative ]] || {
    printf 'missing inventoried runtime file: %s\n' "$relative" >&2
    exit 1
  }
done < "$expected_files"
(cd "$runtime" && shasum -a 256 -c runtime.inventory.sha256)

expected_archs=$(lipo -archs "$app_executable" | tr ' ' '\n' | LC_ALL=C sort | tr '\n' ' ')
is_apple_library() { [[ $1 == /System/Library/* || $1 == /usr/lib/* ]]; }
contained_file() {
  local candidate=$1 binary=$2 dependency=$3 resolved
  [[ -f $candidate && ! -L $candidate ]] || return 1
  resolved=$(cd "$(dirname "$candidate")" && pwd -P)/$(basename "$candidate")
  if [[ $resolved != "$app/"* ]]; then
    printf 'prohibited rpath/loader resolution: %s -> %s (%s)\n' "$binary" "$resolved" "$dependency" >&2
    return 1
  fi
}
resolve_rpath() {
  local binary=$1 dependency=$2 suffix=${dependency#@rpath/} install_id rpath candidate
  install_id=$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -1 || true)
  [[ $install_id == "$dependency" ]] && return 0
  while IFS= read -r rpath; do
    rpath=${rpath//@loader_path/$(dirname "$binary")}
    rpath=${rpath//@executable_path/$(dirname "$app_executable")}
    candidate="$rpath/$suffix"
    if [[ -e $candidate ]]; then
      contained_file "$candidate" "$binary" "$dependency" && return 0
      return 1
    fi
  done < <(otool -l "$binary" | awk '/path .* \(offset [0-9]+\)$/ { print $2 }')
  return 1
}

while IFS= read -r -d '' binary; do
  actual_archs=$(lipo -archs "$binary" | tr ' ' '\n' | LC_ALL=C sort | tr '\n' ' ')
  [[ $actual_archs == "$expected_archs" ]] || {
    printf 'architecture mismatch: %s (%s != %s)\n' "$binary" "$actual_archs" "$expected_archs" >&2
    exit 1
  }
  while IFS= read -r dependency; do
    case "$dependency" in
      @loader_path/*)
        contained_file "$(dirname "$binary")/${dependency#@loader_path/}" "$binary" "$dependency" || {
          printf 'unresolved loader dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1;
        } ;;
      @rpath/*)
        resolve_rpath "$binary" "$dependency" || {
          printf 'unresolved rpath dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1;
        } ;;
      @executable_path/*)
        contained_file "$(dirname "$app_executable")/${dependency#@executable_path/}" "$binary" "$dependency" || {
          printf 'unresolved executable dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1;
        } ;;
      /*)
        is_apple_library "$dependency" || {
          printf 'host-path dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1;
        } ;;
      *) printf 'unsupported loader dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1 ;;
    esac
  done < <(otool -L "$binary" | tail -n +2 | sed -E 's/^[[:space:]]*([^[:space:]]+).*/\1/')
  [[ $mode == development ]] || codesign --verify --strict --verbose=2 "$binary"
done < <({ printf '%s\0' "$app_executable"; find "$runtime" -type f \( -name '*.dylib' -o -perm -111 \) -print0; })

if [[ $mode == signed ]]; then
  codesign --verify --deep --strict --verbose=2 "$app"
  signature_details=$(codesign -dv --verbose=4 "$app" 2>&1)
  [[ $signature_details == *'Authority=Developer ID Application:'* && $signature_details == *'Timestamp='* ]] || {
    printf 'application bundle is not Developer ID signed with a secure timestamp\n' >&2
    exit 1
  }
  xcrun stapler validate "$app"
  spctl --assess --type execute --verbose=2 "$app"
fi

fixture="$script_dir/../../tests/fixtures/videos/h264-1080p.mp4"
sandbox-exec -p '(version 1) (allow default) (deny network*)' \
  "$runtime/bin/ffprobe" -v error -protocol_whitelist file \
  -show_entries format=format_name -of default=nw=1:nk=1 "$fixture" >/dev/null
if [[ -n $artifact ]]; then
  /usr/bin/env node "$script_dir/emit-bundle-audit.mjs" \
    "$mode" "$app" "$attestation" "$artifact"
fi
printf 'Viewer application bundle audited (%s): %s\n' "$mode" "$app"
