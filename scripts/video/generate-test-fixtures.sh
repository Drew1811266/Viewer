#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd -P)
repo_root=$(cd "$script_dir/../.." && pwd -P)
fixture_dir="$repo_root/tests/fixtures/videos"
manifest="$fixture_dir/manifest.json"
runtime_input=${VIEWER_VIDEO_RUNTIME_DIR:-${1:-}}

if [[ -z "$runtime_input" ]]; then
  printf 'set VIEWER_VIDEO_RUNTIME_DIR to the reviewed runtime parent or ViewerVideoRuntime directory\n' >&2
  exit 1
fi
if [[ -d "$runtime_input/ViewerVideoRuntime" ]]; then
  runtime="$runtime_input/ViewerVideoRuntime"
else
  runtime="$runtime_input"
fi
runtime=$(cd "$runtime" 2>/dev/null && pwd -P) || {
  printf 'reviewed video runtime does not exist: %s\n' "$runtime_input" >&2
  exit 1
}
ffmpeg="$runtime/bin/ffmpeg"
ffprobe="$runtime/bin/ffprobe"
if [[ ! -x "$ffmpeg" || ! -x "$ffprobe" || ! -f "$runtime/lib/libmpv.2.dylib" ]]; then
  printf 'fixture generation requires an intact ViewerVideoRuntime\n' >&2
  exit 1
fi

verify_only=false
if [[ ${2:-${1:-}} == --verify ]]; then
  verify_only=true
fi

verify_manifest() {
  node --input-type=module - "$manifest" "$fixture_dir" "$ffprobe" <<'NODE'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import path from 'node:path'

const [manifestPath, fixtureDirectory, ffprobe] = process.argv.slice(2)
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
if (manifest.schemaVersion !== 2 || manifest.fixtures.length !== 9) {
  throw new Error('fixture manifest must contain the Task 14 schema and nine entries')
}
for (const fixture of manifest.fixtures) {
  const filePath = path.join(fixtureDirectory, fixture.file)
  const digest = createHash('sha256').update(readFileSync(filePath)).digest('hex')
  if (digest !== fixture.sha256) throw new Error(`fixture digest mismatch: ${fixture.id}`)
  if (fixture.redistributable !== true) throw new Error(`fixture is not redistribution-safe: ${fixture.id}`)
  const probe = spawnSync(ffprobe, [
    '-v', 'error', '-show_streams', '-show_format', '-of', 'json', filePath,
  ], { encoding: 'utf8' })
  if (fixture.expectedFailure === 'damaged') {
    if (probe.status === 0) throw new Error(`damaged fixture unexpectedly probes: ${fixture.id}`)
    continue
  }
  if (probe.status !== 0) throw new Error(`fixture did not probe: ${fixture.id}`)
  const data = JSON.parse(probe.stdout)
  const video = data.streams.find(({ codec_type: type }) => type === 'video')
  const audio = data.streams.find(({ codec_type: type }) => type === 'audio')
  if (fixture.expectedFailure === 'unsupported') {
    if (video?.codec_name) throw new Error(`unsupported fixture resolved a decoder: ${fixture.id}`)
    continue
  }
  if (!video) throw new Error(`fixture has no video stream: ${fixture.id}`)
  if (fixture.videoCodec && video.codec_name !== fixture.videoCodec) {
    throw new Error(`unexpected video codec: ${fixture.id}`)
  }
  if (Object.hasOwn(fixture, 'audioCodec') && (audio?.codec_name ?? null) !== fixture.audioCodec) {
    throw new Error(`unexpected audio codec: ${fixture.id}`)
  }
  if (fixture.rotation && !video.side_data_list?.some(({ rotation }) => rotation === fixture.rotation)) {
    throw new Error(`fixture rotation is absent: ${fixture.id}`)
  }
  if (fixture.variableFrameRate && video.avg_frame_rate === video.r_frame_rate) {
    throw new Error(`fixture is not variable frame rate: ${fixture.id}`)
  }
}
NODE
}

if [[ "$verify_only" == true ]]; then
  verify_manifest
  printf 'Viewer video fixture hashes verified with bundled tools at %s\n' "$runtime"
  exit 0
fi

mkdir -p "$fixture_dir"
common=(-hide_banner -loglevel error -y -fflags +bitexact)

# The three Task 3 sources are already reviewed, synthetic color/motion samples.
# Task 14 remuxes them with the bundled FFmpeg so the matrix stays independent
# of Homebrew, PATH, VLC, system codec packs, and network access.
"$ffmpeg" "${common[@]}" \
  -i "$fixture_dir/h264-1080p.mp4" \
  -f lavfi -i 'sine=frequency=440:sample_rate=48000:duration=2' \
  -map 0:v:0 -map 1:a:0 -map_metadata -1 -c:v copy -c:a aac -shortest \
  "$fixture_dir/h264-aac.mp4"

"$ffmpeg" "${common[@]}" \
  -display_rotation:v:0 90 -i "$fixture_dir/hevc-portrait.mp4" \
  -map 0:v:0 -map_metadata -1 -c copy "$fixture_dir/hevc-portrait.mov"

"$ffmpeg" "${common[@]}" \
  -f lavfi -i 'testsrc2=size=320x180:rate=24:duration=2' \
  -map_metadata -1 -c:v prores_ks -profile:v 0 -pix_fmt yuv422p10le -an \
  "$fixture_dir/prores.mov"

# The reviewed runtime intentionally has VP9 and AV1 decoders but no matching
# video encoders. These two tiny synthetic, redistributable seeds are embedded
# as base64 so regeneration is byte-for-byte offline and never falls back to a
# production host executable. FFprobe below proves their reviewed codec shape.
base64 -D -i "$script_dir/fixture-seeds/vp9-opus.webm.b64" \
  -o "$fixture_dir/vp9-opus.webm"
base64 -D -i "$script_dir/fixture-seeds/av1.mkv.b64" \
  -o "$fixture_dir/av1.mkv"

"$ffmpeg" "${common[@]}" -i "$fixture_dir/vfr-step.mp4" -map 0:v:0 -map_metadata -1 -c copy \
  "$fixture_dir/vfr.mp4"
"$ffmpeg" "${common[@]}" -i "$fixture_dir/h264-1080p.mp4" -map 0:v:0 -map_metadata -1 -c copy -an \
  "$fixture_dir/silent.mp4"

cp "$fixture_dir/h264-aac.mp4" "$fixture_dir/truncated.mp4"
truncate -s 4096 "$fixture_dir/truncated.mp4"

cp "$fixture_dir/av1.mkv" "$fixture_dir/unsupported-codec.mkv"
node --input-type=module - "$fixture_dir/unsupported-codec.mkv" <<'NODE'
import { readFileSync, writeFileSync } from 'node:fs'
const [file] = process.argv.slice(2)
const bytes = readFileSync(file)
const before = Buffer.from('V_AV1')
const after = Buffer.from('V_NO1')
const offset = bytes.indexOf(before)
if (offset < 0 || bytes.indexOf(before, offset + 1) >= 0) {
  throw new Error('expected exactly one Matroska AV1 codec identifier')
}
after.copy(bytes, offset)
writeFileSync(file, bytes)
NODE

verify_manifest
printf 'Generated and verified Task 14 video fixtures with bundled tools at %s\n' "$runtime"
