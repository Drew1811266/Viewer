#!/bin/sh
set -eu

usage() {
  echo "usage: $0 --mode development|signed <Viewer.app>" >&2
  exit 64
}

[ "$#" -eq 3 ] && [ "$1" = "--mode" ] || usage
mode=$2
app_input=$3
case "$mode" in development|signed) ;; *) usage ;; esac

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
app=$(CDPATH= cd -- "$(dirname -- "$app_input")" && pwd)/$(basename -- "$app_input")
executable=$app/Contents/MacOS/viewer-desktop
runtime=$app/Contents/Resources/ViewerVideoRuntime

[ "$(uname -s)" = Darwin ] || { echo "macOS is required" >&2; exit 1; }
[ -d "$app" ] || { echo "Viewer app is missing: $app" >&2; exit 1; }
[ -x "$executable" ] || { echo "Viewer executable is missing: $executable" >&2; exit 1; }
for relative in bin/ffmpeg bin/ffprobe lib/libmpv.2.dylib runtime.lock.json runtime.inventory.sha256; do
  [ -f "$runtime/$relative" ] || { echo "bundled runtime file is missing: $relative" >&2; exit 1; }
done

if [ "$mode" = signed ]; then
  /bin/bash "$script_dir/verify-app-bundle.sh" --mode signed "$app"
  echo "PASS signed clean-machine prerequisites"
  exit 0
fi

[ "$app" = "$repo_root/target/debug/bundle/macos/Viewer.app" ] || {
  echo "development mode currently requires the debug acceptance app path" >&2
  exit 1
}
[ -x /usr/bin/sandbox-exec ] || { echo "network sandbox is unavailable" >&2; exit 1; }

VIEWER_VIDEO_ACCEPTANCE_APP="$app" \
VIEWER_VIDEO_ACCEPTANCE_NETWORK_DISABLED=1 \
VIEWER_VIDEO_ACCEPTANCE_SKIP_BUILD="${VIEWER_VIDEO_ACCEPTANCE_SKIP_BUILD:-0}" \
/usr/bin/env node "$script_dir/render-feasibility.mjs"

matrix="$repo_root/target/video-render-feasibility/matrix-result.json"
diagnostic="$repo_root/target/video-render-feasibility/task14-generation-diagnostic.json"
audit="$repo_root/target/video-render-feasibility/bundle-audit.json"
launch="$repo_root/target/video-render-feasibility/network-launch.json"
smoke="$repo_root/target/video-acceptance/development-smoke.json"
/usr/bin/env node "$script_dir/emit-development-smoke.mjs" \
  "$app" "$matrix" "$diagnostic" "$audit" "$launch" "$smoke"
/usr/bin/env node "$script_dir/collect-development-evidence.mjs" \
  "$matrix" "$diagnostic" "$smoke" \
  "$repo_root/target/video-acceptance/development-evidence.json" "$repo_root"
echo "PASS offline developer smoke: network denied, bundled runtime only, no host media dependency"
