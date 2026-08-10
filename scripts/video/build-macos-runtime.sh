#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd -P)
repo_root=$(cd "$script_dir/../.." && pwd -P)
lock_file="$script_dir/runtime.lock.json"
: "${VIEWER_VIDEO_WORK_DIR:?set VIEWER_VIDEO_WORK_DIR to an empty build directory}"
: "${VIEWER_VIDEO_STAGE_DIR:?set VIEWER_VIDEO_STAGE_DIR to an empty output directory}"

if [[ -e "$VIEWER_VIDEO_WORK_DIR" ]] && [[ -n "$(find "$VIEWER_VIDEO_WORK_DIR" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  printf 'VIEWER_VIDEO_WORK_DIR must be empty\n' >&2
  exit 1
fi
if [[ -e "$VIEWER_VIDEO_STAGE_DIR" ]] && [[ -n "$(find "$VIEWER_VIDEO_STAGE_DIR" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  printf 'VIEWER_VIDEO_STAGE_DIR must be empty\n' >&2
  exit 1
fi

mkdir -p "$VIEWER_VIDEO_WORK_DIR" "$VIEWER_VIDEO_STAGE_DIR"
work_dir=$(cd "$VIEWER_VIDEO_WORK_DIR" && pwd -P)
stage_dir=$(cd "$VIEWER_VIDEO_STAGE_DIR" && pwd -P)
source_dir="$work_dir/sources"
download_dir="$work_dir/downloads"
build_dir="$work_dir/build"
prefix_dir="$work_dir/prefix"
tool_dir="$work_dir/tools"
mkdir -p "$source_dir" "$download_dir" "$build_dir" "$prefix_dir" "$stage_dir/bin" \
  "$stage_dir/lib" "$stage_dir/licenses"

node "$script_dir/runtime-lock.mjs" validate "$lock_file"

python3 -m venv "$tool_dir"
"$tool_dir/bin/python" -m pip install --disable-pip-version-check \
  -r "$script_dir/build-tools.txt"
"$tool_dir/bin/python" -c 'import jinja2, markupsafe'
meson="$tool_dir/bin/meson"
ninja="$tool_dir/bin/ninja"

while IFS=$'\t' read -r name version url expected_sha256; do
  archive="$download_dir/$name-$version.tar.gz"
  curl -L --fail --show-error --silent "$url" -o "$archive"
  actual_sha256=$(shasum -a 256 "$archive" | awk '{print $1}')
  if [[ "$actual_sha256" != "$expected_sha256" ]]; then
    printf '%s source digest mismatch\n' "$name" >&2
    exit 1
  fi
  mkdir "$source_dir/$name"
  tar -xzf "$archive" -C "$source_dir/$name" --strip-components=1
done < <(node "$script_dir/runtime-lock.mjs" list "$lock_file")

export MACOSX_DEPLOYMENT_TARGET=13.0
export PATH="$prefix_dir/bin:$tool_dir/bin:$PATH"
export PKG_CONFIG="$prefix_dir/bin/pkgconf"
export PKG_CONFIG_PATH="$prefix_dir/lib/pkgconfig:$prefix_dir/share/pkgconfig"
export CFLAGS="-O2 -fPIC -mmacosx-version-min=13.0"
export CXXFLAGS="-O2 -fPIC -mmacosx-version-min=13.0"
export OBJCFLAGS="$CFLAGS"
export LDFLAGS="-mmacosx-version-min=13.0"

meson_static_install() {
  local name=$1
  shift
  "$meson" setup "$build_dir/$name" "$source_dir/$name" \
    --prefix "$prefix_dir" --libdir lib --buildtype release --default-library static "$@"
  "$meson" compile -C "$build_dir/$name"
  "$meson" install -C "$build_dir/$name"
}

meson_static_install pkgconf -Dtests=disabled \
  -Dwith-system-includedir=/usr/include -Dwith-system-libdir=/usr/lib
meson_static_install freetype \
  -Dbrotli=disabled -Dbzip2=disabled -Dharfbuzz=disabled -Dpng=disabled \
  -Dtests=disabled -Dzlib=system
meson_static_install fribidi -Ddeprecated=false -Ddocs=false -Dbin=false -Dtests=false
meson_static_install harfbuzz \
  -Dglib=disabled -Dgobject=disabled -Dcairo=disabled -Dchafa=disabled \
  -Dicu=disabled -Dgraphite2=disabled -Dfreetype=disabled -Dcoretext=disabled \
  -Dtests=disabled -Dintrospection=disabled -Ddocs=disabled -Dutilities=disabled
meson_static_install libass \
  -Dtest=disabled -Dcompare=disabled -Dprofile=disabled -Dfuzz=disabled \
  -Dcheckasm=disabled -Dfontconfig=disabled -Dcoretext=enabled -Dasm=disabled \
  -Dlibunibreak=disabled

# GitHub tag archives omit git submodules. Recreate the exact libplacebo
# submodule layout from the commit-pinned, digest-verified source archives.
mkdir -p "$source_dir/libplacebo/3rdparty"
ditto "$source_dir/fast_float" "$source_dir/libplacebo/3rdparty/fast_float"
ditto "$source_dir/vulkan-headers" "$source_dir/libplacebo/3rdparty/Vulkan-Headers"
meson_static_install libplacebo \
  -Dauto_features=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false \
  -Dvulkan=disabled -Dopengl=disabled -Dd3d11=disabled -Dlcms=disabled \
  -Ddovi=disabled -Dlibdovi=disabled -Dunwind=disabled -Dxxhash=disabled

ffmpeg_arch=${VIEWER_VIDEO_ARCH:-$(uname -m)}
case "$ffmpeg_arch" in
  arm64)
    ffmpeg_arch=arm64
    swift_target=arm64-apple-macos13.0
    ;;
  x86_64)
    ffmpeg_arch=x86_64
    swift_target=x86_64-apple-macos13.0
    ;;
  *) printf 'unsupported macOS architecture: %s\n' "$ffmpeg_arch" >&2; exit 1 ;;
esac

(
  cd "$source_dir/ffmpeg"
  ./configure \
    --prefix="$prefix_dir" \
    --arch="$ffmpeg_arch" \
    --target-os=darwin \
    --cc=clang \
    --cxx=clang++ \
    --disable-autodetect \
    --disable-debug \
    --disable-doc \
    --disable-ffplay \
    --disable-gpl \
    --disable-network \
    --disable-nonfree \
    --disable-shared \
    --enable-static \
    --enable-pic \
    --enable-audiotoolbox \
    --enable-videotoolbox \
    --enable-ffmpeg \
    --enable-ffprobe \
    --extra-cflags="$CFLAGS" \
    --extra-cxxflags="$CXXFLAGS" \
    --extra-ldflags="$LDFLAGS"
  make -j"$(sysctl -n hw.ncpu)"
  make install
)

"$meson" setup "$build_dir/mpv" "$source_dir/mpv" \
  --prefix "$prefix_dir" --libdir lib --buildtype release --default-library shared \
  --prefer-static \
  -Dauto_features=disabled \
  -Dgpl=false -Dcplayer=false -Dlibmpv=true -Dbuild-date=false \
  -Dtests=false -Dfuzzers=false -Djavascript=disabled -Dlua=disabled \
  -Dcplugins=disabled -Dlibarchive=disabled -Dlibbluray=disabled \
  -Ddvdnav=disabled -Drubberband=disabled -Dvapoursynth=disabled \
  -Dlibavdevice=disabled -Djpeg=disabled -Dlcms2=disabled -Duchardet=disabled \
  -Dzimg=disabled -Dcoreaudio=enabled -Diconv=enabled -Dzlib=enabled \
  -Dgl=enabled -Dplain-gl=enabled -Dvideotoolbox-gl=enabled \
  -Dcocoa=enabled -Dgl-cocoa=enabled -Dswift-build=enabled \
  "-Dswift-flags=-target $swift_target" \
  -Dmacos-cocoa-cb=disabled \
  -Dmacos-media-player=disabled -Dmacos-touchbar=disabled \
  -Dvulkan=disabled -Dvideotoolbox-pl=disabled \
  -Dhtml-build=disabled -Dmanpage-build=disabled -Dpdf-build=disabled \
  "-Dc_link_args=-lc++ -liconv"
"$meson" compile -C "$build_dir/mpv"
"$meson" install -C "$build_dir/mpv"

cp "$prefix_dir/lib/libmpv.2.dylib" "$stage_dir/lib/libmpv.2.dylib"
cp "$prefix_dir/bin/ffmpeg" "$stage_dir/bin/ffmpeg"
cp "$prefix_dir/bin/ffprobe" "$stage_dir/bin/ffprobe"
chmod 755 "$stage_dir/bin/ffmpeg" "$stage_dir/bin/ffprobe"
install_name_tool -id '@rpath/libmpv.2.dylib' "$stage_dir/lib/libmpv.2.dylib"
swiftc_path=$(xcrun -find swiftc)
swift_toolchain_runtime=$(
  "$tool_dir/bin/python" "$source_dir/mpv/TOOLS/macos-swift-lib-directory.py" "$swiftc_path"
)
if otool -l "$stage_dir/lib/libmpv.2.dylib" | rg -F -q "path $swift_toolchain_runtime "; then
  install_name_tool -delete_rpath "$swift_toolchain_runtime" "$stage_dir/lib/libmpv.2.dylib"
fi

cp "$source_dir/pkgconf/COPYING" "$stage_dir/licenses/pkgconf.txt"
cp "$source_dir/freetype/LICENSE.TXT" "$stage_dir/licenses/freetype.txt"
cp "$source_dir/fribidi/COPYING" "$stage_dir/licenses/fribidi.txt"
cp "$source_dir/harfbuzz/COPYING" "$stage_dir/licenses/harfbuzz.txt"
cp "$source_dir/libass/COPYING" "$stage_dir/licenses/libass.txt"
cp "$source_dir/ffmpeg/COPYING.LGPLv2.1" "$stage_dir/licenses/ffmpeg.txt"
cp "$source_dir/fast_float/LICENSE-APACHE" "$stage_dir/licenses/fast_float.txt"
cp "$source_dir/vulkan-headers/LICENSE.txt" "$stage_dir/licenses/vulkan-headers.txt"
cp "$source_dir/libplacebo/LICENSE" "$stage_dir/licenses/libplacebo.txt"
cp "$source_dir/mpv/LICENSE.LGPL" "$stage_dir/licenses/mpv.txt"
cp "$lock_file" "$stage_dir/runtime.lock.json"

(
  cd "$stage_dir"
  find bin lib licenses -type f -print | LC_ALL=C sort | while IFS= read -r path; do
    shasum -a 256 "$path"
  done > runtime.inventory.sha256
)

VIEWER_VIDEO_STAGE_DIR="$stage_dir" "$script_dir/verify-runtime.sh"
printf 'Viewer video runtime staged at %s\n' "$stage_dir"
