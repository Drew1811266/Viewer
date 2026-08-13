#!/bin/sh
set -eu

usage() {
  echo "usage: $0 [--verify] <ViewerVideoRuntime-or-parent> [output-directory]" >&2
  exit 64
}

verify_only=false
if [ "${1:-}" = "--verify" ]; then
  verify_only=true
  shift
fi
[ "$#" -ge 1 ] && [ "$#" -le 2 ] || usage

runtime_input=$1
output_dir=${2:-target/video-performance}
case "$runtime_input" in
  */ViewerVideoRuntime) runtime_dir=$runtime_input ;;
  *) runtime_dir=$runtime_input/ViewerVideoRuntime ;;
esac

ffmpeg=$runtime_dir/bin/ffmpeg
ffprobe=$runtime_dir/bin/ffprobe
[ -x "$ffmpeg" ] || { echo "reviewed ffmpeg missing: $ffmpeg" >&2; exit 1; }
[ -x "$ffprobe" ] || { echo "reviewed ffprobe missing: $ffprobe" >&2; exit 1; }
[ -f "$runtime_dir/lib/libmpv.2.dylib" ] || {
  echo "reviewed libmpv missing: $runtime_dir/lib/libmpv.2.dylib" >&2
  exit 1
}

mkdir -p "$output_dir"
h264=$output_dir/h264-1080p60.mp4
hevc=$output_dir/hevc-4k30.mov

if [ "$verify_only" = false ]; then
  "$ffmpeg" -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=1920x1080:rate=60:duration=5" \
    -an -pix_fmt nv12 -c:v h264_videotoolbox -allow_sw 0 -b:v 12M \
    -movflags +faststart "$h264"
  "$ffmpeg" -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=3840x2160:rate=30:duration=5" \
    -an -pix_fmt nv12 -c:v hevc_videotoolbox -allow_sw 0 -b:v 24M -tag:v hvc1 \
    "$hevc"
fi

verify_stream() {
  file=$1
  codec=$2
  width=$3
  height=$4
  rate=$5
  actual=$(
    "$ffprobe" -v error -select_streams v:0 \
      -show_entries stream=codec_name,width,height,avg_frame_rate,pix_fmt \
      -of default=noprint_wrappers=1:nokey=1 "$file" | tr '\n' '|'
  )
  expected="$codec|$width|$height|yuv420p|$rate|"
  [ "$actual" = "$expected" ] || {
    echo "performance sample mismatch: $file expected=$expected actual=$actual" >&2
    exit 1
  }
}

verify_stream "$h264" h264 1920 1080 60/1
verify_stream "$hevc" hevc 3840 2160 30/1
echo "PASS performance samples: $h264 $hevc"
