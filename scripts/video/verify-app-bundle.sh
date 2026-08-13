#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd -P)
app=${1:?usage: verify-app-bundle.sh Viewer.app}
if [[ ! -d "$app" ]]; then
  printf 'application bundle does not exist: %s\n' "$app" >&2
  exit 1
fi
app=$(cd "$app" && pwd -P)
runtime="$app/Contents/Resources/ViewerVideoRuntime"
if [[ ! -d "$runtime" ]]; then
  printf 'missing bundled video runtime\n' >&2
  exit 1
fi

runtime_verifier=${VIEWER_VIDEO_RUNTIME_VERIFIER:-$script_dir/verify-runtime.sh}
VIEWER_VIDEO_STAGE_DIR="$runtime" "$runtime_verifier"

app_executable="$app/Contents/MacOS/Viewer"
if [[ ! -x "$app_executable" ]]; then
  printf 'missing Viewer application executable\n' >&2
  exit 1
fi
expected_archs=$(lipo -archs "$app_executable" | tr ' ' '\n' | LC_ALL=C sort | tr '\n' ' ')

is_apple_library() {
  [[ $1 == /System/Library/* || $1 == /usr/lib/* ]]
}

resolve_rpath() {
  local binary=$1 dependency=$2 suffix=${dependency#@rpath/}
  install_id=$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -1 || true)
  if [[ "$install_id" == "$dependency" ]]; then
    return 0
  fi
  while IFS= read -r rpath; do
    rpath=${rpath//@loader_path/$(dirname "$binary")}
    rpath=${rpath//@executable_path/$(dirname "$app_executable")}
    if [[ -f "$rpath/$suffix" ]]; then
      resolved=$(cd "$(dirname "$rpath/$suffix")" && pwd -P)/$(basename "$rpath/$suffix")
      if [[ "$resolved" != "$app/"* ]]; then
        printf 'prohibited rpath resolution: %s -> %s\n' "$binary" "$resolved" >&2
        return 1
      fi
      return 0
    fi
  done < <(otool -l "$binary" | awk '/path .* \(offset [0-9]+\)$/ { print $2 }')
  return 1
}

while IFS= read -r -d '' binary; do
  actual_archs=$(lipo -archs "$binary" | tr ' ' '\n' | LC_ALL=C sort | tr '\n' ' ')
  if [[ "$actual_archs" != "$expected_archs" ]]; then
    printf 'architecture mismatch: %s (%s != %s)\n' "$binary" "$actual_archs" "$expected_archs" >&2
    exit 1
  fi

  while IFS= read -r dependency; do
    case "$dependency" in
      @loader_path/*)
        resolved="$(dirname "$binary")/${dependency#@loader_path/}"
        [[ -f "$resolved" ]] || { printf 'unresolved loader dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1; }
        ;;
      @rpath/*)
        resolve_rpath "$binary" "$dependency" || { printf 'unresolved rpath dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1; }
        ;;
      @executable_path/*)
        resolved="$(dirname "$app_executable")/${dependency#@executable_path/}"
        [[ -f "$resolved" ]] || { printf 'unresolved executable dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1; }
        ;;
      /*)
        is_apple_library "$dependency" || { printf 'host-path dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1; }
        ;;
      *) printf 'unsupported loader dependency: %s -> %s\n' "$binary" "$dependency" >&2; exit 1 ;;
    esac
  done < <(otool -L "$binary" | tail -n +2 | sed -E 's/^[[:space:]]*([^[:space:]]+).*/\1/')

  codesign --verify --strict --verbose=2 "$binary"
done < <(find "$runtime" -type f \( -name '*.dylib' -o -perm -111 \) -print0)

codesign --verify --deep --strict --verbose=2 "$app"
signature_details=$(codesign -dv --verbose=4 "$app" 2>&1)
if [[ "$signature_details" != *"Authority=Developer ID Application:"* || "$signature_details" != *"Timestamp="* ]]; then
  printf 'application bundle is not Developer ID signed with a secure timestamp\n' >&2
  exit 1
fi
spctl --assess --type execute --verbose=2 "$app"

fixture="$script_dir/../../tests/fixtures/videos/h264-1080p.mp4"
sandbox-exec -p '(version 1) (allow default) (deny network*)' \
  "$runtime/bin/ffprobe" -v error -protocol_whitelist file \
  -show_entries format=format_name -of default=nw=1:nk=1 "$fixture" >/dev/null

printf 'Viewer application bundle verified: %s\n' "$app"
